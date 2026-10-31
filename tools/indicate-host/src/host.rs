//! The host loop: admit, paint, route.

use indicate_alerts::{
    AlertContext, AlertEvent, AlertManager, AlertOutput, AlertProfile, FlightPhase,
};
use indicate_instrument_descriptor::{
    ConfigBlob, DesignFrame, DrawContext, PanelDescriptor, SceneTarget,
};
use indicate_instrument_feeder::clock::clock_ok;
use indicate_instrument_scene::{
    LayerReport, SceneWriter, validate_layers,
    volume::{Extent3D, VolumeBandReport, VolumeSceneWriter, validate_volume_bands},
};
use indicate_instrument_state::{AircraftState, FreshnessPolicy, NavSource, PanelData, resolve};
use indicate_intent::{EchoMonitor, EchoStatus, EchoSummary, Intent, IntentGate, Request};
use indicate_interaction::{
    InteractionFrame, InteractionOutcome, ViewLimits, ViewPose3D, apply_event,
};

use crate::error::{HostError, RequestRefused};
use crate::liveness::Liveness;

/// Bound on alert events queued between paints.
const MAX_QUEUED_EVENTS: usize = 32;

/// What the host is configured with.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HostConfig {
    /// The liveness deadline: a frame must be made this often.
    pub frame_deadline_ms: f64,
    /// How long a selection request may wait for its echo.
    pub confirm_within_ms: f64,
    /// The flight phase the alert manager is stepped in.
    pub phase: FlightPhase,
}

impl Default for HostConfig {
    fn default() -> Self {
        Self {
            frame_deadline_ms: 1000.0,
            confirm_within_ms: 2000.0,
            phase: FlightPhase::Climb,
        }
    }
}

/// Where an admitted request went.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Routed {
    /// A selection the shell forwards to its source; the display shows
    /// it only once the source echoes it.
    ToSource(Intent),
    /// An acknowledgement queued for the alert manager's next step.
    ToAlerts,
    /// A page or declutter request the shell applies to its display.
    ToDisplay(Intent),
}

/// One completed frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Painted {
    /// Encoded scene length in the caller's buffer.
    pub scene_len: usize,
    /// The snapshot generation the frame was resolved from.
    pub frame_generation: u32,
    /// The render generation after this frame; advances once per
    /// completed frame and never on a failed one.
    pub render_generation: u32,
    /// The alert snapshot fanned to the panel.
    pub alerts: AlertOutput,
    /// Where each outstanding selection request stands.
    pub echo: [EchoStatus; 4],
    /// The layer report of the validated scene.
    pub layers: LayerReport,
}

/// One completed 3D volumetric frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PaintedVolume {
    /// Encoded volume scene length in the caller's buffer.
    pub scene_len: usize,
    /// The snapshot generation the frame was resolved from.
    pub frame_generation: u32,
    /// The render generation after this frame.
    pub render_generation: u32,
    /// The alert snapshot fanned to the instrument.
    pub alerts: AlertOutput,
    /// Where each outstanding selection request stands.
    pub echo: [EchoStatus; 4],
    /// The active 3D view pose supplied to the instrument.
    pub view_pose: ViewPose3D,
    /// The band report of the validated volume scene.
    pub bands: VolumeBandReport,
}

/// The reference host.
#[derive(Debug)]
pub struct Host {
    config: HostConfig,
    profile: AlertProfile,
    policy: FreshnessPolicy,
    state: AircraftState,
    frame_generation: u32,
    render_generation: u32,
    alerts: AlertManager,
    liveness: Liveness,
    gate: IntentGate,
    echo: EchoMonitor,
    last_echo: [EchoStatus; 4],
    last_admitted_seq: Option<u32>,
    view_pose: ViewPose3D,
    queued: [Option<AlertEvent>; MAX_QUEUED_EVENTS],
    queued_len: usize,
    declutter: bool,
    observed_ms: Option<f64>,
}

impl Host {
    /// A host with no snapshot admitted and no frame made.
    pub fn new(config: HostConfig, profile: AlertProfile) -> Self {
        Self {
            config,
            profile,
            policy: FreshnessPolicy::default(),
            state: AircraftState::default(),
            frame_generation: 0,
            render_generation: 0,
            alerts: AlertManager::new(),
            liveness: Liveness::new(config.frame_deadline_ms),
            gate: IntentGate::new(),
            echo: EchoMonitor::new(config.confirm_within_ms),
            last_echo: [EchoStatus::Idle; 4],
            last_admitted_seq: None,
            view_pose: ViewPose3D::IDENTITY,
            queued: [None; MAX_QUEUED_EVENTS],
            queued_len: 0,
            declutter: false,
            observed_ms: None,
        }
    }

    /// Admits one snapshot and advances the snapshot generation once.
    /// The generation is the host's: a tracker's counter is never
    /// forwarded into it. Returns the generation the snapshot carries.
    pub fn admit_snapshot(
        &mut self,
        mut state: AircraftState,
        now_ms: f64,
    ) -> Result<u32, HostError> {
        if !clock_ok(now_ms, self.observed_ms) {
            return Err(HostError::InvalidClock);
        }
        self.observed_ms = Some(now_ms);
        self.frame_generation = self.frame_generation.wrapping_add(1);
        state.snapshot.generation = self.frame_generation;
        self.state = state;
        Ok(self.frame_generation)
    }

    /// The snapshot the next paint resolves from.
    pub const fn state(&self) -> &AircraftState {
        &self.state
    }

    /// Admits and routes one operator request.
    pub fn request(&mut self, request: &Request, now_ms: f64) -> Result<Routed, RequestRefused> {
        let admitted = self.gate.admit(request, now_ms)?;
        self.last_admitted_seq = Some(admitted.request.stamp.sequence);
        match admitted.request.intent {
            Intent::AcknowledgeAlert(id) => {
                self.enqueue(AlertEvent::Acknowledge(id))?;
                Ok(Routed::ToAlerts)
            }
            Intent::AcknowledgeAllAlerts => {
                self.enqueue(AlertEvent::AcknowledgeAll)?;
                Ok(Routed::ToAlerts)
            }
            Intent::Declutter(on) => {
                self.declutter = on;
                self.view_pose.decluttered = on;
                Ok(Routed::ToDisplay(admitted.request.intent))
            }
            Intent::SelectPage(_) | Intent::PressSoftkey { .. } => {
                Ok(Routed::ToDisplay(admitted.request.intent))
            }
            Intent::SelectBaro { .. }
            | Intent::SetHeadingBug { .. }
            | Intent::SelectAltitude { .. }
            | Intent::SelectNavSource(_)
            | Intent::Custom { .. } => {
                self.echo.request(&admitted);
                Ok(Routed::ToSource(admitted.request.intent))
            }
        }
    }

    /// Current bounded 3D view pose supplied to volumetric instruments.
    #[must_use]
    pub const fn view_pose(&self) -> ViewPose3D {
        self.view_pose
    }

    /// Current echo-tracking summary supplied in [`DrawContext`].
    #[must_use]
    pub const fn echo_summary(&self) -> EchoSummary {
        EchoSummary {
            statuses: self.last_echo,
            last_admitted_seq: self.last_admitted_seq,
        }
    }

    /// Applies one spatial [`InteractionFrame`] against `limits` to evolve [`ViewPose3D`].
    pub fn interact(
        &mut self,
        frame: &InteractionFrame,
        limits: &ViewLimits,
    ) -> Result<InteractionOutcome, HostError> {
        let outcome = apply_event(&mut self.view_pose, &frame.event, limits)?;
        self.declutter = self.view_pose.decluttered;
        Ok(outcome)
    }

    /// One paint-clock tick: resolve the admitted snapshot once, step
    /// the alert manager once with the queued and supplied events, draw
    /// `panel` into `scene_buf`, and validate the scene against the
    /// layer contract and the panel's required layers.
    ///
    /// A completed frame advances the render generation; a refused one
    /// does not, and the shell paints [`crate::failure_page`] instead.
    /// The tick runs whether or not data arrived: a frame made on time
    /// from stale data is a live frame that carries stale data.
    pub fn paint(
        &mut self,
        panel: &PanelDescriptor,
        config: &ConfigBlob<'_>,
        frame: DesignFrame,
        events: &[AlertEvent],
        scene_buf: &mut [u8],
        now_ms: f64,
    ) -> Result<Painted, HostError> {
        if !clock_ok(now_ms, self.observed_ms) {
            return Err(HostError::InvalidClock);
        }
        self.observed_ms = Some(now_ms);
        if let Some(event) = self.liveness.tick(now_ms) {
            self.enqueue_or_drop(event);
        }
        let alerts = self.step_alerts(events, now_ms);
        let echo = self.observe_echo(now_ms);
        let data: PanelData = resolve(&self.state, &self.policy);
        let scene_len = self.draw_planar(panel, &data, config, Some(&alerts), frame, scene_buf)?;
        let layers = validate_layers(&scene_buf[..scene_len]).map_err(HostError::Layers)?;
        let missing = panel.required_layers & !u16::from(layers.present);
        if missing != 0 {
            return Err(HostError::MissingLayers { missing });
        }
        self.render_generation = self.render_generation.wrapping_add(1);
        if let Some(event) = self.liveness.painted(now_ms) {
            self.enqueue_or_drop(event);
        }
        Ok(Painted {
            scene_len,
            frame_generation: self.frame_generation,
            render_generation: self.render_generation,
            alerts,
            echo,
            layers,
        })
    }

    /// One paint-clock tick for a 3D volumetric instrument (`RenderSpace::Volume3D`).
    pub fn paint_volume(
        &mut self,
        panel: &PanelDescriptor,
        config: &ConfigBlob<'_>,
        extent_m: Extent3D,
        events: &[AlertEvent],
        scene_buf: &mut [u8],
        now_ms: f64,
    ) -> Result<PaintedVolume, HostError> {
        if !clock_ok(now_ms, self.observed_ms) {
            return Err(HostError::InvalidClock);
        }
        self.observed_ms = Some(now_ms);
        if let Some(event) = self.liveness.tick(now_ms) {
            self.enqueue_or_drop(event);
        }
        let alerts = self.step_alerts(events, now_ms);
        let echo = self.observe_echo(now_ms);
        let data: PanelData = resolve(&self.state, &self.policy);
        let scene_len = self.draw_vol(panel, &data, config, Some(&alerts), extent_m, scene_buf)?;
        let bands = validate_volume_bands(&scene_buf[..scene_len]).map_err(HostError::Layers)?;
        let missing = panel.required_layers & !bands.present;
        if missing != 0 {
            return Err(HostError::MissingLayers { missing });
        }
        self.render_generation = self.render_generation.wrapping_add(1);
        if let Some(event) = self.liveness.painted(now_ms) {
            self.enqueue_or_drop(event);
        }
        Ok(PaintedVolume {
            scene_len,
            frame_generation: self.frame_generation,
            render_generation: self.render_generation,
            alerts,
            echo,
            view_pose: self.view_pose,
            bands,
        })
    }

    /// The render generation: completed frames, wrapping.
    pub const fn render_generation(&self) -> u32 {
        self.render_generation
    }

    /// Whether the liveness watchdog currently reports a stall.
    pub const fn stalled(&self) -> bool {
        self.liveness.stalled()
    }

    fn observe_echo(&mut self, now_ms: f64) -> [EchoStatus; 4] {
        let nav_source = self
            .state
            .nav
            .data
            .map_or(NavSource::None, |nav| nav.source);
        let echo = self
            .echo
            .observe(&self.state.selections, nav_source, now_ms);
        self.last_echo = echo;
        echo
    }

    fn draw_planar(
        &self,
        panel: &PanelDescriptor,
        data: &PanelData,
        config: &ConfigBlob<'_>,
        alerts: Option<&AlertOutput>,
        frame: DesignFrame,
        scene_buf: &mut [u8],
    ) -> Result<usize, HostError> {
        let ctx = DrawContext::new(data)
            .with_config(*config)
            .with_alerts(alerts)
            .with_echo(self.echo_summary())
            .with_view(self.view_pose);
        let mut writer = SceneWriter::new(scene_buf).map_err(HostError::Scene)?;
        (panel.draw)(
            &ctx,
            SceneTarget::Planar2D {
                frame,
                writer: &mut writer,
            },
        )
        .map_err(HostError::Draw)?;
        Ok(writer.finish())
    }

    fn draw_vol(
        &self,
        panel: &PanelDescriptor,
        data: &PanelData,
        config: &ConfigBlob<'_>,
        alerts: Option<&AlertOutput>,
        extent_m: Extent3D,
        scene_buf: &mut [u8],
    ) -> Result<usize, HostError> {
        let ctx = DrawContext::new(data)
            .with_config(*config)
            .with_alerts(alerts)
            .with_echo(self.echo_summary())
            .with_view(self.view_pose);
        let mut writer = VolumeSceneWriter::new(scene_buf).map_err(HostError::Scene)?;
        (panel.draw)(
            &ctx,
            SceneTarget::Volume3D {
                extent_m,
                writer: &mut writer,
            },
        )
        .map_err(HostError::Draw)?;
        Ok(writer.finish())
    }

    /// Steps the manager once with everything queued since the last
    /// paint followed by the events the shell supplies for this tick.
    fn step_alerts(&mut self, events: &[AlertEvent], now_ms: f64) -> AlertOutput {
        let mut batch = [AlertEvent::AcknowledgeAll; MAX_QUEUED_EVENTS * 2];
        let mut len = 0;
        for event in self.queued.iter().take(self.queued_len).flatten() {
            batch[len] = *event;
            len += 1;
        }
        for event in events.iter().take(MAX_QUEUED_EVENTS) {
            batch[len] = *event;
            len += 1;
        }
        self.queued = [None; MAX_QUEUED_EVENTS];
        self.queued_len = 0;
        let ctx = AlertContext {
            phase: self.config.phase,
            declutter: self.declutter,
            alerting_path_healthy: true,
        };
        // `clock_ok` guarantees a finite, non-negative reading; the
        // manager's clock is whole milliseconds.
        self.alerts
            .step(&self.profile, &batch[..len], ctx, now_ms as u64)
    }

    fn enqueue(&mut self, event: AlertEvent) -> Result<(), RequestRefused> {
        if self.queued_len >= MAX_QUEUED_EVENTS {
            return Err(RequestRefused::QueueFull);
        }
        self.queued[self.queued_len] = Some(event);
        self.queued_len += 1;
        Ok(())
    }

    /// A watchdog event that finds the queue full is dropped: the stall
    /// it reports is still visible through [`Host::stalled`].
    fn enqueue_or_drop(&mut self, event: AlertEvent) {
        self.enqueue(event).ok();
    }
}
