//! Ink containment per face: every glyph a face draws, at several sizes
//! and rotations, paints only inside the ink box the same face's
//! metrics report. Layouts that must contain text (readout boxes, dial
//! numerals) rely on that box, so a backend that painted past it would
//! break them silently.

use indicate_instrument_glyphs::typeface::FaceTable;
use indicate_instrument_scene::{Anchor, FaceSlot, LayerId, MAX_SCENE_BYTES, SceneWriter};

use std::string::ToString;

use super::WHITE;
use super::faces::every_face;
use crate::{FrameId, FramebufferDims, render_with_faces};

const SIDE: u32 = 160;

fn lit_pixels(
    face_slot: FaceSlot,
    faces: &FaceTable<'_>,
    text: &str,
    size: f32,
    turn: f32,
) -> std::vec::Vec<(u32, u32)> {
    let mut buf = std::vec![0u8; MAX_SCENE_BYTES];
    let mut w = SceneWriter::new(&mut buf).expect("writer");
    w.begin_layer(LayerId::Tapes).expect("layer");
    w.font(face_slot).expect("font");
    w.fill_color(WHITE).expect("fill");
    w.translate(80.0, 80.0).expect("translate");
    w.rotate(turn).expect("rotate");
    w.text(
        0.0,
        0.0,
        size,
        Anchor {
            h: indicate_instrument_scene::HAlign::Left,
            v: indicate_instrument_scene::VAlign::Baseline,
        },
        text,
    )
    .expect("text");
    w.end_layer(LayerId::Tapes).expect("layer");
    let n = w.finish();
    let mut fb = std::vec![0u8; (SIDE * SIDE * 4) as usize];
    render_with_faces(
        &buf[..n],
        &mut fb,
        FramebufferDims::tight(SIDE, SIDE),
        FrameId::default(),
        faces,
    )
    .expect("renders");
    (0..SIDE)
        .flat_map(|y| (0..SIDE).map(move |x| (x, y)))
        .filter(|(x, y)| fb[((y * SIDE + x) * 4 + 3) as usize] != 0)
        .collect()
}

#[test]
fn every_glyph_of_every_face_paints_inside_its_ink_box() {
    let chars = "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ.-/°";
    for (name, face) in every_face() {
        let slots = [face];
        let faces = FaceTable::new(&slots);
        for size in [9.0f32, 14.0, 23.0, 40.0] {
            for turn in [0.0f32, 0.6] {
                for ch in chars.chars() {
                    let text = ch.to_string();
                    let ink = face
                        .ink_box(size, &text, crate::text::face::TOLERANCE_PX)
                        .expect("lays out")
                        .expect("inked");
                    let (s, c) = (libm::sinf(turn), libm::cosf(turn));
                    for (px, py) in lit_pixels(FaceSlot(0), &faces, &text, size, turn) {
                        // The pixel centre back in the run's own frame.
                        let (dx, dy) = (px as f32 + 0.5 - 80.0, py as f32 + 0.5 - 80.0);
                        let (x, y) = (dx * c + dy * s, -dx * s + dy * c);
                        // Half a pixel's diagonal of slack for the centre
                        // sampling, plus the Q8.8 snap.
                        let slack = 0.72;
                        assert!(
                            x >= ink.min_x - slack
                                && x <= ink.max_x + slack
                                && y >= ink.min_y - slack
                                && y <= ink.max_y + slack,
                            "{name}: '{ch}' at size {size}, turn {turn}: ({x}, {y}) outside {ink:?}",
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn an_unbound_slot_draws_in_slot_zero_and_a_missing_glyph_fails() {
    let (_, face) = every_face()[1];
    let slots = [face];
    let faces = FaceTable::new(&slots);
    let in_slot_zero = lit_pixels(FaceSlot(0), &faces, "40", 20.0, 0.0);
    let in_slot_five = lit_pixels(FaceSlot(5), &faces, "40", 20.0, 0.0);
    assert!(!in_slot_zero.is_empty());
    assert_eq!(in_slot_zero, in_slot_five);
    let mut buf = std::vec![0u8; MAX_SCENE_BYTES];
    let mut w = SceneWriter::new(&mut buf).expect("writer");
    w.begin_layer(LayerId::Tapes).expect("layer");
    // Lowercase is outside the stroke face, though the reference pack
    // has it: the bound face fails rather than borrowing a glyph.
    w.text(10.0, 20.0, 12.0, Anchor::CENTER, "kt")
        .expect("text");
    w.end_layer(LayerId::Tapes).expect("layer");
    let n = w.finish();
    let mut fb = std::vec![0u8; (SIDE * SIDE * 4) as usize];
    let result = render_with_faces(
        &buf[..n],
        &mut fb,
        FramebufferDims::tight(SIDE, SIDE),
        FrameId::default(),
        &faces,
    );
    assert!(
        matches!(result, Err(crate::RasterError::Typeface(_))),
        "{result:?}"
    );
}
