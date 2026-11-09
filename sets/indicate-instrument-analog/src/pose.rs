//! Where every moving part of a dial stands, from resolved state or
//! from raw values.
//!
//! This is the whole per-frame cost of a 3D gauge: [`reading`] picks
//! the dial's two values out of [`PanelData`], and [`pose`] turns them
//! into one angle per pointer and one offset per drum wheel. Nothing
//! here draws or decodes a scene.

use indicate_instrument_state::{BesideSamples, PanelData};

use crate::law::wheel_positions;
use crate::spec::{
    AnalogDialSpec, CardSpec, Channel, FailurePose, MAX_CARDS, MAX_EXTRA_CHANNELS, MAX_POINTERS,
    MAX_TAPES, MAX_WHEELS, MAX_WINDOWS, TapeSpec, WindowSpec,
};

/// The dial's values in dial units; `None` where the state shows no
/// value. Build one by hand with `..DialReading::default()`.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct DialReading {
    /// The primary channel.
    pub primary: Option<f32>,
    /// The secondary channel.
    pub secondary: Option<f32>,
    /// The extra channels, in [`AnalogDialSpec::extra`] order.
    pub extra: [Option<f32>; MAX_EXTRA_CHANNELS],
}

impl DialReading {
    /// The value `channel` reads.
    #[must_use]
    pub fn of(&self, channel: Channel) -> Option<f32> {
        match channel {
            Channel::Primary => self.primary,
            Channel::Secondary => self.secondary,
            Channel::Extra(n) => self.extra.get(usize::from(n)).copied().flatten(),
        }
    }
}

/// Reads `spec`'s channels from resolved state and the record groups
/// beside it (engines and fuel tanks), in dial units.
#[must_use]
pub fn reading(
    spec: &AnalogDialSpec<'_>,
    data: &PanelData,
    beside: &BesideSamples<'_>,
) -> DialReading {
    let mut extra = [None; MAX_EXTRA_CHANNELS];
    for (slot, channel) in extra.iter_mut().zip(spec.extra) {
        *slot = channel.read(data, beside);
    }
    DialReading {
        primary: spec.primary.read(data, beside),
        secondary: spec
            .secondary
            .and_then(|channel| channel.read(data, beside)),
        extra,
    }
}

/// One pointer's pose.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct PointerPose {
    /// Rotation about the pivot, radians clockwise from twelve o'clock:
    /// the one number a 3D backend applies to the extruded needle.
    pub angle_rad: f32,
    /// Whether the pointer is drawn at all.
    pub visible: bool,
    /// Whether the pointer shows a value, rather than resting at its
    /// failure pose.
    pub live: bool,
}

/// One drum window's pose.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct WindowPose {
    /// Each wheel's position in digits, rightmost first; see
    /// [`crate::wheel_positions`].
    pub wheels: [f32; MAX_WHEELS],
    /// Whether the drum shows a value. A drum without one shows no
    /// digits, because a digit would claim a value the state lacks.
    pub live: bool,
}

/// One card's pose: a turn about its pivot, then a slide along its own
/// vertical axis.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct CardPose {
    /// Rotation about the pivot, radians clockwise.
    pub angle_rad: f32,
    /// Travel along the turned card's vertical axis, face units,
    /// positive down the card.
    pub offset: f32,
    /// Whether the card is drawn at all.
    pub visible: bool,
    /// Whether the card shows a value, rather than resting at its
    /// failure pose.
    pub live: bool,
}

/// One tape's pose.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct TapePose {
    /// The value under the window's reading point, wrapped into
    /// `[0, period)`. The physical drum stands at
    /// `value / period` of a turn.
    pub value: f32,
    /// Whether the tape shows a value. A tape without one shows a blank
    /// card, because a printed heading would claim one.
    pub live: bool,
}

/// Every moving part of one dial.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct DialPose {
    /// Pointer poses, in [`AnalogDialSpec::pointers`] order.
    pub pointers: [PointerPose; MAX_POINTERS],
    /// Window poses, in [`AnalogDialSpec::windows`] order.
    pub windows: [WindowPose; MAX_WINDOWS],
    /// Card poses, in [`AnalogDialSpec::cards`] order.
    pub cards: [CardPose; MAX_CARDS],
    /// Tape poses, in [`AnalogDialSpec::tapes`] order.
    pub tapes: [TapePose; MAX_TAPES],
    /// Whether the failure flag shows: some moving part has no value.
    pub flag: bool,
}

/// Poses every moving part of `spec` for `reading`.
///
/// `spec` is assumed to have passed [`AnalogDialSpec::check`]; parts
/// past the fixed capacities are not posed.
#[must_use]
pub fn pose(spec: &AnalogDialSpec<'_>, reading: &DialReading) -> DialPose {
    let mut out = DialPose::default();
    let primary = reading
        .primary
        .map(|v| clamp_to(v, spec.input_range))
        .filter(|v| v.is_finite());
    let value = |channel: Channel| match channel {
        Channel::Primary => primary,
        other => reading.of(other).filter(|v| v.is_finite()),
    };
    for (slot, pointer) in out.pointers.iter_mut().zip(spec.pointers) {
        *slot = match (value(pointer.channel), pointer.on_failure) {
            (Some(v), _) => PointerPose {
                angle_rad: pointer.law.angle_rad(v),
                visible: true,
                live: true,
            },
            (None, FailurePose::Park { angle_deg }) => PointerPose {
                angle_rad: angle_deg.to_radians(),
                visible: true,
                live: false,
            },
            (None, FailurePose::Hide) => PointerPose::default(),
        };
        out.flag |= !slot.live;
    }
    for (slot, window) in out.windows.iter_mut().zip(spec.windows) {
        *slot = window_pose(window, value(window.channel));
        out.flag |= !slot.live;
    }
    for (slot, card) in out.cards.iter_mut().zip(spec.cards) {
        *slot = card_pose(card, &value);
        out.flag |= !slot.live;
    }
    for (slot, tape) in out.tapes.iter_mut().zip(spec.tapes) {
        *slot = tape_pose(tape, value(tape.channel));
        out.flag |= !slot.live;
    }
    out
}

fn card_pose(card: &CardSpec<'_>, value: &impl Fn(Channel) -> Option<f32>) -> CardPose {
    let angle = card
        .turn
        .map(|turn| value(turn.channel).map(|v| turn.law.angle_rad(v)));
    let offset = card.travel.map(|travel| {
        value(travel.channel).map(|v| {
            let [lo, hi] = travel.limit;
            (v * travel.per_unit).clamp(lo, hi)
        })
    });
    match (angle, offset) {
        (Some(None), _) | (_, Some(None)) => match card.on_failure {
            FailurePose::Park { angle_deg } => CardPose {
                angle_rad: angle_deg.to_radians(),
                offset: 0.0,
                visible: true,
                live: false,
            },
            FailurePose::Hide => CardPose::default(),
        },
        (angle, offset) => CardPose {
            angle_rad: angle.flatten().unwrap_or(0.0),
            offset: offset.flatten().unwrap_or(0.0),
            visible: true,
            live: true,
        },
    }
}

fn tape_pose(tape: &TapeSpec<'_>, value: Option<f32>) -> TapePose {
    let Some(v) = value else {
        return TapePose::default();
    };
    let wrapped = v - libm::floorf(v / tape.period) * tape.period;
    TapePose {
        // Rounding can land a value a hair under zero on exactly the
        // period; the reading point is then at the period's start.
        value: if wrapped >= tape.period { 0.0 } else { wrapped },
        live: true,
    }
}

fn clamp_to(value: f32, range: Option<[f32; 2]>) -> f32 {
    match range {
        Some([lo, hi]) if lo < hi => value.clamp(lo, hi),
        _ => value,
    }
}

fn window_pose(window: &WindowSpec<'_>, value: Option<f32>) -> WindowPose {
    let Some(v) = value else {
        return WindowPose::default();
    };
    // Rounded to a thousandth of a digit, so 29.92 does not show its
    // rightmost wheel a hair short of 2 because 29.92 has no exact f32.
    let shown =
        libm::roundf(clamp_to(v, Some(window.range)) * window.value_scale * 1000.0) / 1000.0;
    WindowPose {
        wheels: wheel_positions::<MAX_WHEELS>(shown, usize::from(window.wheels)),
        live: true,
    }
}
