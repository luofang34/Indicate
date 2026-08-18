//! The fixture screen the bench composes, and the digest pinned over it.
//!
//! Two readers need the same screen: the bench, which validates and
//! reproduces the digest, and the release-manifest generator, which
//! records it. A second copy of the descriptor would let the manifest
//! state a digest no shell reproduces, so the descriptor and its pin
//! live here and are read from both.

use indicate_instrument_registry::{CompositionDescriptor, DesignFrame, Region, Slot};

/// The logical screen the fixture composition lays out on: two panel
/// frames wide and two tall.
pub const BENCH_SCREEN: DesignFrame = DesignFrame {
    width: 960.0,
    height: 720.0,
};

const fn tile(panel: &'static str, x: f32, y: f32) -> Slot {
    Slot {
        panel,
        rect: Region {
            x,
            y,
            width: 480.0,
            height: 360.0,
        },
        occludes: &[],
    }
}

/// The fixture screen: the three shipped panels tiled, each at the one
/// frame it declares. It overlaps nothing, so what it exercises here is
/// placement, the frame rule, and the digest; the occlusion and
/// dead-slot rules are covered by the registry's own must-fail
/// fixtures, which need panels shaped to break them.
pub const BENCH_COMPOSITION: CompositionDescriptor = CompositionDescriptor {
    screen: BENCH_SCREEN,
    slots: &[
        tile("pfd", 0.0, 0.0),
        tile("hsi", 480.0, 0.0),
        tile("monitor", 0.0, 360.0),
    ],
};

/// The pinned screen-composition digest over [`BENCH_COMPOSITION`]:
/// every shell composing this screen from this registry reproduces it.
pub const BENCH_COMPOSITION_DIGEST: &str =
    "ae3ed05d0eb319650840f3a8174853bcde651ecb158f9976e6a34a58c4da2b1e";
