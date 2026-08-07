//! The template panel: one label, one readout, one honest dash-out.
//!
//! The panel shows indicated airspeed. That choice is doing work: a
//! number a shell could fabricate is exactly what the admission harness
//! polices, so the smallest interesting panel is one that paints a
//! numeral it must justify.

use indicate_alerts::AlertOutput;
use indicate_instrument_descriptor::{
    BackgroundCapability, ConfigBlob, DesignFrame, GroupSet, PanelDescriptor, PanelDrawError,
    PanelSet, Region,
};
use indicate_instrument_scene::{Anchor, LayerId, PaintMode, SceneWriter};
use indicate_instrument_state::{GroupId, PanelData};
use indicate_instrument_symbology::{fmt_label, palette, safety};

/// Design-frame width. A panel's logical space is its own declaration,
/// not a house size: backends scale it to whatever viewport they own.
const FRAME_W: f32 = 240.0;

/// Design-frame height.
const FRAME_H: f32 = 120.0;

const LABEL_X: f32 = 16.0;
const LABEL_Y: f32 = 34.0;
const LABEL_SIZE: f32 = 16.0;

/// The readout is right-anchored, so a value that grows wider grows
/// leftward into empty frame instead of off the right edge.
const VALUE_RIGHT_X: f32 = 224.0;
const VALUE_Y: f32 = 78.0;

/// Sized against the value buffer, not against the corpus: the widest
/// label the buffer can hold — a full eight characters, measured by
/// `indicate_instrument_scene::nominal_text_ink_width` — still starts
/// inside the frame from `VALUE_RIGHT_X`. Fitting only the values the
/// fixtures happen to produce is what leaves a panel with counted
/// frame-overflow warnings the day a wider one arrives; the crate's
/// admission test holds that count at zero.
const VALUE_SIZE: f32 = 24.0;

/// A required-layer mask is a bitset over [`LayerId`] wire values.
const fn layer_bit(layer: LayerId) -> u8 {
    1u8 << layer.to_u8()
}

/// Draws the template panel from resolved state.
///
/// Two bands, in ascending order: `Background` carries the opaque ground
/// the descriptor promises, `Tapes` the label and the readout. Each
/// `begin_layer`/`end_layer` pair emits the mandatory state-isolation
/// save and restore, which is the layer envelope admission checks.
fn draw_template_panel(
    data: &PanelData,
    config: &ConfigBlob<'_>,
    _alerts: Option<&AlertOutput>,
    scene: &mut SceneWriter<'_>,
) -> Result<(), PanelDrawError> {
    // The empty schema already makes any keyed blob a shell-side
    // rejection; re-checking here keeps the property when a shell skips
    // its gate.
    config.require_schema(TEMPLATE_DESCRIPTOR.config_schema)?;

    // `Opaque` is a promise to a compositor, and this is how it is kept:
    // one axis-aligned, unclipped, full-alpha rect over the whole frame.
    // A ground assembled from polygons, or drawn under a clip, does not
    // satisfy the check however opaque it looks.
    scene.begin_layer(LayerId::Background)?;
    scene.fill_color(palette::BLACK)?;
    scene.rect(PaintMode::Fill, 0.0, 0.0, FRAME_W, FRAME_H)?;
    scene.end_layer(LayerId::Background)?;

    scene.begin_layer(LayerId::Tapes)?;
    scene.fill_color(palette::GREY)?;
    // Fixed furniture: a run with no digits needs no provenance claim,
    // because there is no value in it to fabricate.
    scene.text(LABEL_X, LABEL_Y, LABEL_SIZE, Anchor::MIDDLE_LEFT, "IAS")?;
    draw_airspeed(data, scene)?;
    scene.end_layer(LayerId::Tapes)?;
    Ok(())
}

/// The readout, and the whole honest-status rule in one branch.
fn draw_airspeed(data: &PanelData, scene: &mut SceneWriter<'_>) -> Result<(), PanelDrawError> {
    let ias = data.ias_kt;
    if ias.status.shows_value() {
        scene.fill_color(palette::WHITE)?;
        // Every run carrying a digit goes through `text_attributed`: the
        // claim names the state group the number derives from, and the
        // withholding matrix tests it. An unclaimed numeral is refused
        // outright — omitting the tag is not an escape, it is the
        // failure.
        let value = fmt_label!(8, "{:.0}kt", ias.value);
        scene.text_attributed(
            GroupId::Air.to_u8(),
            VALUE_RIGHT_X,
            VALUE_Y,
            VALUE_SIZE,
            Anchor::MIDDLE_RIGHT,
            value.as_str(),
        )?;
    } else {
        // Unclaimed on purpose. A claim is tested against withholding
        // wherever its run is drawn, so dashes tagged `Air` would be
        // refused in the very case they exist for: the panel would be
        // claiming to show air data while showing that it has none.
        // Dashes are a failure cue, not blank space, so they paint in
        // the never-skinnable failure red.
        scene.fill_color(safety::FAILURE_RED)?;
        scene.text(
            VALUE_RIGHT_X,
            VALUE_Y,
            VALUE_SIZE,
            Anchor::MIDDLE_RIGHT,
            "---",
        )?;
    }
    Ok(())
}

/// The template panel, as data.
///
/// Every field below is a statement a shell, a compositor, or the
/// admission harness acts on. Filling one in wrongly is not a style
/// defect; it is a claim the panel then has to keep.
pub const TEMPLATE_DESCRIPTOR: PanelDescriptor = PanelDescriptor {
    // Lowercase, digits and dashes only — the registry refuses anything
    // else at init. Canvas ids, health keys and evidence records key off
    // this string, so it outlives whatever the constant is called.
    id: "template",
    // Operator-facing, and non-empty: a registry refuses a panel that
    // cannot label a health or layout surface.
    title: "Template",
    // The bands that must be present and complete in every frame,
    // however degraded the state. The readout lives in `Tapes`, so
    // `Tapes` is required and nothing else is. `Background` is
    // deliberately absent even though this panel always paints it: it is
    // the one band a compositor may replace or drop, and requiring it
    // would promise something the contract lets a shell take away.
    required_layers: layer_bit(LayerId::Tapes),
    // The withholding matrix. Admission redraws the panel once per group
    // named here with that group withheld, and refuses any visible run
    // still claiming it. Declare the groups the readout DERIVES from:
    // `Air` carries the airspeed, and `Trust` decides whether it may be
    // shown at all — with trust withheld the source has declared
    // nothing, so the value dashes out even though the air data is still
    // there. A group a panel merely reads past belongs nowhere here.
    required_groups: GroupSet::of(&[GroupId::Air, GroupId::Trust]),
    // The logical space this panel draws against. It need not match any
    // other panel's; the registry asks only that it be finite and
    // positive, and every geometry check downstream is expressed in it.
    design_frame: DesignFrame {
        width: FRAME_W,
        height: FRAME_H,
    },
    // A compositor plans around this declaration, so admission refuses
    // both directions: painting a band declared `NotUsed`, and failing
    // to cover a band declared owned. `Opaque` is the honest answer for
    // a panel whose text needs a ground of its own.
    background: BackgroundCapability::Opaque,
    // No configuration. An empty schema makes any keyed blob a
    // shell-side rejection; a panel with real keys lists them in
    // strictly ascending order, and decodes them in `draw`.
    config_schema: &[],
    // Where a consumed group paints its readout, in design-frame units.
    // The admission harness does not read this — honest status is proven
    // by the provenance claims on the runs, wherever the ink lands — but
    // the registry validates the geometry at init, and a shell that
    // presents readout ownership or a dash-out declaration reads it.
    // `Trust` gets no region: it qualifies a readout rather than owning
    // one, and the map is allowed to be partial.
    group_regions: &[(
        GroupId::Air,
        Region {
            x: 60.0,
            y: 64.0,
            width: 168.0,
            height: 28.0,
        },
    )],
    // Stress fixtures beyond the four shared canonical states, which
    // every panel meets regardless. A panel whose geometry can leave the
    // frame, or whose readouts have a widest case no shared fixture
    // reaches, contributes its own here. This one has nothing to add.
    extreme_states: &[],
    // The pinned reference-rasterizer hash of this panel's typical
    // frame, once a baseline travels with the descriptor. `None` is the
    // honest value until then, not a placeholder to fill with anything.
    raster_baseline: None,
    // The entry point: pure resolved state to scene. A panel never
    // reaches for a clock, a source, or a shell — everything it may draw
    // from arrives in the arguments.
    draw: draw_template_panel,
};

/// The set a shell names to compose this panel.
///
/// A provider crate exports one set rather than loose descriptors, so
/// gaining a panel is a change here instead of one line in every shell.
/// Set identity stays out of the scene digest: regrouping the same
/// panels without reordering them leaves cross-shell identity untouched.
pub const TEMPLATE_SET: PanelSet = PanelSet {
    id: "template",
    panels: &[TEMPLATE_DESCRIPTOR],
};

#[cfg(test)]
mod tests;
