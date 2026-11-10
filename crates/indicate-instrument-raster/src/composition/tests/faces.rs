//! Composition letters each slot's face slots in the host's binding,
//! the same binding admission judged the panel with.

use indicate_instrument_glyphs::typeface::{Face, FaceTable, TextStyle, Typeface};
use indicate_instrument_registry::{
    CompositionDescriptor, DrawContext, PanelDescriptor, PanelDrawError, Registry, SceneTarget,
    Slot, UnderlayContract,
};
use indicate_instrument_scene::{
    Anchor, FaceSlot, HAlign, LayerId, MAX_SCENE_BYTES, Rgba8, VAlign,
};

use std::vec;
use std::vec::Vec;

use super::super::{CompositionInputs, render_composition, render_composition_with_faces};
use super::{INSET_FRAME, INSET_RECT, PANEL_FRAME, PFD_ALONE, builtin, fixture, typical};
use crate::{FrameId, FramebufferDims, RenderStatus};

const MS33558: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../testdata/typefaces/us-ms33558/us-ms33558.idtf"
));

/// A legend lettered in face slot 1.
fn draw_lettered(ctx: &DrawContext<'_>, target: SceneTarget<'_, '_>) -> Result<(), PanelDrawError> {
    ctx.config().require_schema(&[])?;
    let SceneTarget::Planar2D { writer, .. } = target else {
        return Err(PanelDrawError::WrongTarget);
    };
    writer.begin_layer(LayerId::Tapes)?;
    writer.save()?;
    writer.font(FaceSlot(1))?;
    writer.fill_color(Rgba8::rgb(255, 255, 255))?;
    let anchor = Anchor {
        h: HAlign::Center,
        v: VAlign::Middle,
    };
    writer.text(
        INSET_FRAME.width / 2.0,
        INSET_FRAME.height / 2.0,
        20.0,
        anchor,
        "OIL 80",
    )?;
    writer.restore()?;
    writer.end_layer(LayerId::Tapes)?;
    Ok(())
}

static PFD_AND_LETTERED: [PanelDescriptor; 2] = [
    indicate_instrument_panels::PFD_DESCRIPTOR,
    fixture("lettered", UnderlayContract::None, draw_lettered),
];

const LETTERED: CompositionDescriptor = CompositionDescriptor {
    screen: PANEL_FRAME,
    slots: &[
        PFD_ALONE.slots[0],
        Slot {
            panel: "lettered",
            rect: INSET_RECT,
            occludes: &[],
        },
    ],
};

fn compose(registry: &Registry, faces: Option<&FaceTable<'_>>) -> Vec<u8> {
    let (w, h) = (PANEL_FRAME.width as u32, PANEL_FRAME.height as u32);
    let mut pixels = vec![0u8; (w * h * 4) as usize];
    let mut scratch = vec![0u8; MAX_SCENE_BYTES];
    let data = typical();
    let mut inputs = CompositionInputs {
        data: &data,
        alerts: None,
        scratch: &mut scratch,
    };
    let dims = FramebufferDims::tight(w, h);
    let report = match faces {
        None => render_composition(
            registry,
            &LETTERED,
            &mut inputs,
            &mut pixels,
            dims,
            FrameId::default(),
        ),
        Some(faces) => render_composition_with_faces(
            registry,
            &LETTERED,
            &mut inputs,
            &mut pixels,
            dims,
            FrameId::default(),
            faces,
        ),
    }
    .expect("the lettered composition renders");
    assert_eq!(report.status, RenderStatus::Painted);
    pixels
}

#[test]
fn composing_without_faces_is_composing_with_the_default_binding() {
    let registry = Registry::new(&PFD_AND_LETTERED).expect("composes");
    assert_eq!(
        compose(&registry, None),
        compose(&registry, Some(&FaceTable::DEFAULT))
    );
    // The builtin compositions draw no face slot, so binding a face
    // changes none of their pixels either.
    let face = Face::typeface(
        Typeface::parse(MS33558).expect("the shipped face parses"),
        TextStyle::REGULAR,
    )
    .expect("a valid style");
    let slots = [Face::Reference, face];
    let faces = FaceTable::new(&slots);
    let builtin = builtin();
    let (w, h) = (PANEL_FRAME.width as u32, PANEL_FRAME.height as u32);
    let mut frames = [
        vec![0u8; (w * h * 4) as usize],
        vec![0u8; (w * h * 4) as usize],
    ];
    for (pixels, faces) in frames.iter_mut().zip([&FaceTable::DEFAULT, &faces]) {
        let mut scratch = vec![0u8; MAX_SCENE_BYTES];
        let data = typical();
        let mut inputs = CompositionInputs {
            data: &data,
            alerts: None,
            scratch: &mut scratch,
        };
        render_composition_with_faces(
            &builtin,
            &PFD_ALONE,
            &mut inputs,
            pixels,
            FramebufferDims::tight(w, h),
            FrameId::default(),
            faces,
        )
        .expect("the PFD composes");
    }
    assert_eq!(frames[0], frames[1]);
}

#[test]
fn a_slot_letters_in_the_face_the_host_binds() {
    let registry = Registry::new(&PFD_AND_LETTERED).expect("composes");
    let face = Face::typeface(
        Typeface::parse(MS33558).expect("the shipped face parses"),
        TextStyle::REGULAR,
    )
    .expect("a valid style");
    let slots = [Face::Reference, face];
    let bound = compose(&registry, Some(&FaceTable::new(&slots)));
    let reference = compose(&registry, None);
    assert_ne!(bound, reference, "the bound face changes the legend");
    // Only the lettered slot's rect differs: the PFD beneath letters in
    // slot 0, which both bindings leave on the reference face.
    let width = PANEL_FRAME.width as usize;
    for (index, (a, b)) in bound.chunks(4).zip(reference.chunks(4)).enumerate() {
        let (x, y) = ((index % width) as f32, (index / width) as f32);
        let inside = x >= INSET_RECT.x
            && x < INSET_RECT.right()
            && y >= INSET_RECT.y
            && y < INSET_RECT.bottom();
        if !inside {
            assert_eq!(a, b, "pixel ({x}, {y}) outside the lettered slot");
        }
    }
}
