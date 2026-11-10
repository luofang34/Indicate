//! Lettering judged in the bound face: a run in a face slot is measured
//! with the face the host binds there, and a run that face cannot
//! letter is refused.
#![allow(clippy::expect_used, clippy::panic)]

use indicate_instrument_glyphs::typeface::{
    Face, FaceDef, FaceTable, GlyphDef, Seg, Shape, StrokeDef, TextStyle, Typeface, TypefaceError,
    encode_face, encoded_len,
};
use indicate_instrument_registry::{
    DrawContext, GroupSet, PanelDescriptor, PanelDrawError, Region, Registry, SceneTarget,
    UnderlayContract,
};
use indicate_instrument_scene::{
    ATTR_CONFIG, Anchor, ArcAlign, ArcRun, ArcSense, FaceSlot, HAlign, LayerId, Rgba8, VAlign,
};
use indicate_instrument_state::GroupId;

use super::super::{
    AdmissionError, AdmissionWarning, admit, admit_with_faces, criticality_bands,
    criticality_bands_with_faces,
};
use super::{FIXTURE_FRAME, FIXTURE_SPACE};

const MS33558: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../testdata/typefaces/us-ms33558/us-ms33558.idtf"
));

/// `us-ms33558` at four times its width: the widest style a host may
/// bind, so a run that fits in nominal metrics overruns in this face.
fn wide() -> Face<'static> {
    let style = TextStyle {
        condense: 4.0,
        ..TextStyle::REGULAR
    };
    Face::typeface(Typeface::parse(MS33558).expect("parses"), style).expect("a valid style")
}

const fn probe(
    groups: GroupSet,
    group_regions: &'static [(GroupId, Region)],
    draw: indicate_instrument_registry::DrawFn,
) -> PanelDescriptor {
    PanelDescriptor {
        id: "lettered",
        title: "Lettered probe",
        space: FIXTURE_SPACE,
        required_layers: 1 << 2,
        required_groups: groups,
        underlay: UnderlayContract::None,
        config_schema: &[],
        accepted_intents: &[],
        aux_channels: &[],
        group_regions,
        extreme_states: &[],
        raster_baselines: &[],
        draw,
    }
}

/// Opens the tapes band lettering in slot 1.
fn lettered<'s, 'b>(
    target: SceneTarget<'s, 'b>,
) -> Result<&'s mut indicate_instrument_scene::SceneWriter<'b>, PanelDrawError> {
    let SceneTarget::Planar2D { writer, .. } = target else {
        return Err(PanelDrawError::WrongTarget);
    };
    writer.begin_layer(LayerId::Tapes)?;
    writer.fill_color(Rgba8::rgb(255, 255, 255))?;
    writer.font(FaceSlot(1))?;
    Ok(writer)
}

/// A printed legend ending 40 units short of the right edge: inside the
/// frame in nominal metrics, past it at four times the width.
fn draw_legend(_: &DrawContext<'_>, t: SceneTarget<'_, '_>) -> Result<(), PanelDrawError> {
    let w = lettered(t)?;
    let anchor = Anchor {
        h: HAlign::Left,
        v: VAlign::Middle,
    };
    w.text_attributed(ATTR_CONFIG, 300.0, 180.0, 14.0, anchor, "OIL TEMP")?;
    w.end_layer(LayerId::Tapes)?;
    Ok(())
}

#[test]
fn overflow_is_judged_in_the_bound_face() {
    static PANEL: [PanelDescriptor; 1] = [probe(GroupSet::EMPTY, &[], draw_legend)];
    let registry = Registry::new(&PANEL).expect("structurally valid");
    let quiet = admit(&registry).expect("admissible in the reference face");
    assert!(quiet.warnings.is_empty(), "{:?}", quiet.warnings);
    let slots = [Face::Reference, wide()];
    let report = admit_with_faces(&registry, &FaceTable::new(&slots))
        .expect("an overrun is counted, not refused");
    assert!(
        report.warnings.iter().any(|w| matches!(
            w,
            AdmissionWarning::FrameOverflow { text, .. } if text == "OIL TEMP"
        )),
        "{:?}",
        report.warnings
    );
}

/// The airspeed readout, right-aligned at x = 200 with its region drawn
/// around where nominal metrics centre it.
fn draw_readout(ctx: &DrawContext<'_>, t: SceneTarget<'_, '_>) -> Result<(), PanelDrawError> {
    let w = lettered(t)?;
    if ctx.data().ias_kt.status.shows_value() {
        let anchor = Anchor {
            h: HAlign::Right,
            v: VAlign::Middle,
        };
        w.text_attributed(GroupId::Air.to_u8(), 200.0, 100.0, 20.0, anchor, "120")?;
    }
    w.end_layer(LayerId::Tapes)?;
    Ok(())
}

const NEAR_THE_ANCHOR: Region = Region {
    x: 160.0,
    y: 80.0,
    width: 40.0,
    height: 40.0,
};

#[test]
fn a_readout_region_is_judged_in_the_bound_face() {
    static PANEL: [PanelDescriptor; 1] = [probe(
        GroupSet::of(&[GroupId::Air]),
        &[(GroupId::Air, NEAR_THE_ANCHOR)],
        draw_readout,
    )];
    let registry = Registry::new(&PANEL).expect("structurally valid");
    admit(&registry).expect("nominal metrics centre the readout in its region");
    let slots = [Face::Reference, wide()];
    // Four times as wide, the readout's centre lies far left of the
    // region declared for it: the region points at the wrong surface.
    assert!(matches!(
        admit_with_faces(&registry, &FaceTable::new(&slots)),
        Err(AdmissionError::GroupRegionEmpty { .. })
    ));
}

/// A face with only the letter I: any other character is missing.
fn only_i() -> std::vec::Vec<u8> {
    const STEM: &[StrokeDef<'static>] = &[StrokeDef {
        start: [100, 0],
        segments: &[Seg::Line([100, 700])],
    }];
    let def = FaceDef {
        id: "only-i",
        units_per_em: 1000,
        cap_height: 700,
        ascender: 800,
        descender: -200,
        default_pen: 100,
        alternates: &[],
        glyphs: &[GlyphDef {
            ch: 'I',
            alternate: 0,
            advance: 300,
            shape: Shape::Stroke(STEM),
        }],
        kerning: &[],
    };
    let mut bytes = std::vec![0u8; encoded_len(&def)];
    encode_face(&def, &mut bytes).expect("encodes");
    bytes
}

#[test]
fn a_run_the_bound_face_cannot_letter_is_refused() {
    static PANEL: [PanelDescriptor; 1] = [probe(GroupSet::EMPTY, &[], draw_legend)];
    let registry = Registry::new(&PANEL).expect("structurally valid");
    let bytes = only_i();
    let face = Face::typeface(Typeface::parse(&bytes).expect("parses"), TextStyle::REGULAR)
        .expect("a valid style");
    let slots = [Face::Reference, face];
    assert!(matches!(
        admit_with_faces(&registry, &FaceTable::new(&slots)),
        Err(AdmissionError::FaceLettering {
            slot: 1,
            source: TypefaceError::MissingGlyph { ch: 'O' },
            ..
        })
    ));
    // Bound to slot 0 only, slot 1 falls back to slot 0's face, exactly
    // as the rasterizer resolves it.
    let fallback = [face];
    assert!(matches!(
        admit_with_faces(&registry, &FaceTable::new(&fallback)),
        Err(AdmissionError::FaceLettering { slot: 1, .. })
    ));
}

/// A warning lettered along an arc in the annunciation band.
fn draw_arc_warning(_: &DrawContext<'_>, t: SceneTarget<'_, '_>) -> Result<(), PanelDrawError> {
    let SceneTarget::Planar2D { writer: w, .. } = t else {
        return Err(PanelDrawError::WrongTarget);
    };
    w.begin_layer(LayerId::Tapes)?;
    w.end_layer(LayerId::Tapes)?;
    w.begin_layer(LayerId::Annunciation)?;
    w.fill_color(Rgba8::rgb(255, 200, 0))?;
    w.font(FaceSlot(1))?;
    w.text_arc(&ArcRun {
        cx: FIXTURE_FRAME.width / 2.0,
        cy: FIXTURE_FRAME.height / 2.0,
        radius: 100.0,
        angle: 0.0,
        sense: ArcSense::Clockwise,
        align: ArcAlign::Center,
        size: 12.0,
        text: "LOW OIL",
    })?;
    w.end_layer(LayerId::Annunciation)?;
    Ok(())
}

#[test]
fn an_arc_warning_is_measured_into_the_criticality_band() {
    static PANEL: [PanelDescriptor; 1] = [probe(GroupSet::EMPTY, &[], draw_arc_warning)];
    let registry = Registry::new(&PANEL).expect("structurally valid");
    let reference = criticality_bands(&registry).expect("measures");
    let band = reference
        .first()
        .and_then(|p| p.band)
        .expect("an arc run inks its band");
    // The run is centred at angle 0, on the circle's +x side.
    let (cx, cy) = (FIXTURE_FRAME.width / 2.0, FIXTURE_FRAME.height / 2.0);
    assert!(
        band.x <= cx + 100.0 && band.x + band.width >= cx + 100.0,
        "{band:?}"
    );
    assert!(band.y <= cy && band.y + band.height >= cy, "{band:?}");
    let slots = [Face::Reference, wide()];
    let bound = criticality_bands_with_faces(&registry, &FaceTable::new(&slots))
        .expect("measures")
        .first()
        .and_then(|p| p.band)
        .expect("an arc run inks its band");
    // Four times as wide, the warning runs further round the circle.
    assert!(bound.height > band.height, "{bound:?} vs {band:?}");
}
