#![allow(clippy::expect_used, clippy::panic)]

use indicate_alerts::AlertOutput;
use indicate_instrument_registry::{
    BackgroundCapability, ConfigBlob, DesignFrame, GroupSet, PanelDescriptor, PanelDrawError,
    Registry,
};
use indicate_instrument_scene::{LayerId, PaintMode, SceneWriter};
use indicate_instrument_state::PanelData;

use super::check_frame_varies;
use crate::admission::error::AdmissionError;

const MIN: DesignFrame = DesignFrame {
    width: 480.0,
    height: 360.0,
};
const MAX: DesignFrame = DesignFrame {
    width: 600.0,
    height: 450.0,
};

/// Paints a rect the size of the frame it was handed, so its bytes move
/// with the argument.
fn draw_varying(
    _data: &PanelData,
    _config: &ConfigBlob<'_>,
    _alerts: Option<&AlertOutput>,
    frame: DesignFrame,
    scene: &mut SceneWriter<'_>,
) -> Result<(), PanelDrawError> {
    scene.begin_layer(LayerId::Tapes)?;
    scene.rect(PaintMode::Fill, 0.0, 0.0, frame.width, frame.height)?;
    scene.end_layer(LayerId::Tapes)?;
    Ok(())
}

/// Paints the same rect whatever it is handed — the panel the check
/// exists to catch.
fn draw_fixed(
    _data: &PanelData,
    _config: &ConfigBlob<'_>,
    _alerts: Option<&AlertOutput>,
    _frame: DesignFrame,
    scene: &mut SceneWriter<'_>,
) -> Result<(), PanelDrawError> {
    scene.begin_layer(LayerId::Tapes)?;
    scene.rect(PaintMode::Fill, 0.0, 0.0, 480.0, 360.0)?;
    scene.end_layer(LayerId::Tapes)?;
    Ok(())
}

const fn ranged(id: &'static str, draw: indicate_instrument_registry::DrawFn) -> PanelDescriptor {
    PanelDescriptor {
        id,
        title: "Probe",
        required_layers: 0b0000_0100,
        required_groups: GroupSet::EMPTY,
        frame_min: MIN,
        frame_max: MAX,
        frame_step: (120.0, 90.0),
        aspect_min: 1.30,
        aspect_max: 1.37,
        canonical_frames: &[MIN, MAX],
        background: BackgroundCapability::NotUsed,
        config_schema: &[],
        group_regions: &[],
        extreme_states: &[],
        raster_baselines: &[],
        draw,
    }
}

static VARYING: PanelDescriptor = ranged("varying", draw_varying);
static FIXED: PanelDescriptor = ranged("fixed", draw_fixed);

#[test]
fn a_panel_that_ignores_its_frame_is_refused() {
    assert_eq!(
        check_frame_varies(&FIXED),
        Err(AdmissionError::FrameIgnored {
            panel: "fixed",
            min: MIN,
            max: MAX,
        })
    );
}

#[test]
fn a_panel_that_uses_its_frame_is_admitted() {
    assert_eq!(check_frame_varies(&VARYING), Ok(()));
}

/// A degenerate range is a declaration that the panel does not vary,
/// which is what every shipped panel says today. The check must not ask
/// a panel to differ from itself.
#[test]
fn a_degenerate_range_is_not_asked_to_vary() {
    static FIXED_RANGE: PanelDescriptor = PanelDescriptor {
        frame_max: MIN,
        canonical_frames: &[MIN],
        ..ranged("degenerate", draw_fixed)
    };
    assert_eq!(check_frame_varies(&FIXED_RANGE), Ok(()));
}

/// The shipped panels all declare a degenerate range, so the check is
/// inert for them — pinned here so a panel that gains a range without
/// gaining a layout cannot land quietly.
#[test]
fn the_shipped_panels_are_unaffected() {
    let registry = Registry::new(indicate_instrument_panels::BUILTIN_PANELS).expect("composes");
    for panel in registry.panels() {
        assert_eq!(check_frame_varies(panel), Ok(()), "{}", panel.id);
    }
}
