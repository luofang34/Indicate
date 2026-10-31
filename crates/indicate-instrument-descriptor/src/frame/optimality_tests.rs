//! Optimality: what `choose_frame` returns, against an exhaustive grid scan.

#![allow(clippy::expect_used, clippy::panic)]

use crate::descriptor::{DesignFrame, InstrumentDescriptor};
use crate::space::RenderSpace;

use super::tests::{MIN, RANGED, frame, planar_space};

fn largest_by_scan(d: &InstrumentDescriptor, space: DesignFrame) -> Option<DesignFrame> {
    let RenderSpace::Planar2D {
        frame_min,
        frame_max,
        frame_step,
        ..
    } = d.space
    else {
        return None;
    };
    let mut best: Option<DesignFrame> = None;
    let w_end = frame_max.width.min(space.width);
    let h_end = frame_max.height.min(space.height);
    let mut w = frame_min.width;
    while w <= w_end {
        let mut h = frame_min.height;
        while h <= h_end {
            let candidate = frame(w, h);
            if candidate.width <= space.width
                && candidate.height <= space.height
                && d.accepts(candidate).is_ok()
            {
                let better =
                    best.is_none_or(|b| candidate.width * candidate.height > b.width * b.height);
                if better {
                    best = Some(candidate);
                }
            }
            h += frame_step.1;
        }
        w += frame_step.0;
    }
    best
}

const FIXED_ASPECT: InstrumentDescriptor = InstrumentDescriptor {
    space: planar_space(
        MIN,
        frame(960.0, 720.0),
        (10.0, 10.0),
        4.0 / 3.0,
        4.0 / 3.0,
        &[MIN, frame(960.0, 720.0)],
    ),
    ..RANGED
};

const NARROW_BAND: InstrumentDescriptor = InstrumentDescriptor {
    space: planar_space(
        MIN,
        frame(960.0, 680.0),
        (16.0, 16.0),
        1.20,
        1.50,
        &[MIN, frame(960.0, 680.0)],
    ),
    ..RANGED
};

const WIDE_SPAN: InstrumentDescriptor = InstrumentDescriptor {
    space: planar_space(
        frame(320.0, 180.0),
        frame(5120.0, 2880.0),
        (1.0, 1.0),
        16.0 / 9.0,
        16.0 / 9.0,
        &[frame(320.0, 180.0), frame(5120.0, 2880.0)],
    ),
    ..RANGED
};

const ODD_STEPS: InstrumentDescriptor = InstrumentDescriptor {
    space: planar_space(
        MIN,
        frame(900.0, 690.0),
        (7.0, 11.0),
        1.20,
        1.50,
        &[MIN, frame(900.0, 690.0)],
    ),
    ..RANGED
};

#[test]
fn a_wide_short_space_is_served_from_a_wide_range() {
    assert_eq!(
        WIDE_SPAN.choose_frame(frame(5120.0, 200.0)),
        Ok(frame(352.0, 198.0))
    );
    assert_eq!(
        WIDE_SPAN.choose_frame(frame(5120.0, 400.0)),
        Ok(frame(704.0, 396.0))
    );
    assert_eq!(
        WIDE_SPAN.choose_frame(frame(4000.0, 300.0)),
        Ok(frame(528.0, 297.0))
    );
}

#[test]
fn a_fixed_aspect_is_served_not_refused() {
    assert_eq!(FIXED_ASPECT.choose_frame(frame(500.0, 380.0)), Ok(MIN));
    assert_eq!(
        FIXED_ASPECT.choose_frame(frame(900.0, 500.0)),
        Ok(frame(640.0, 480.0))
    );
}

#[test]
fn an_aspect_bound_landing_below_a_grid_line_does_not_lose_a_step() {
    assert_eq!(
        NARROW_BAND.choose_frame(frame(539.0, 457.0)),
        Ok(frame(528.0, 440.0))
    );
    assert_eq!(
        ODD_STEPS.choose_frame(frame(565.0, 490.0)),
        Ok(frame(564.0, 470.0))
    );
}

fn agrees_with(
    d: &InstrumentDescriptor,
    space: DesignFrame,
    chosen: DesignFrame,
    best: DesignFrame,
) {
    let RenderSpace::Planar2D { frame_min, .. } = d.space else {
        panic!("expected Planar2D");
    };
    assert_eq!(
        chosen.width * chosen.height,
        best.width * best.height,
        "chose {chosen:?} in {space:?}, scan found {best:?}"
    );
    assert!(
        chosen.width <= space.width && chosen.height <= space.height,
        "chose {chosen:?}, which does not fit {space:?}"
    );
    assert!(
        chosen.width >= frame_min.width && chosen.height >= frame_min.height,
        "chose {chosen:?}, below the declared floor"
    );
}

#[test]
fn choose_frame_agrees_with_an_exhaustive_scan() {
    let mut checked = 0;
    let mut refusals = 0;
    for d in [&RANGED, &FIXED_ASPECT, &NARROW_BAND, &ODD_STEPS] {
        let mut w = 400.0f32;
        while w <= 1000.0 {
            let mut h = 300.0f32;
            while h <= 760.0 {
                let space = frame(w, h);
                let scanned = largest_by_scan(d, space);
                match (d.choose_frame(space), scanned) {
                    (Ok(chosen), Some(best)) => {
                        agrees_with(d, space, chosen, best);
                        checked += 1;
                    }
                    (Err(_), None) => refusals += 1,
                    (Ok(chosen), None) => {
                        panic!("chose {chosen:?} for {w}x{h}, which the scan refuses")
                    }
                    (Err(why), Some(best)) => {
                        panic!("refused {w}x{h} as {why:?}, but {best:?} fits")
                    }
                }
                h += 13.0;
            }
            w += 17.0;
        }
    }
    assert!(checked > 500, "the sweep agreed on {checked} answers");
    assert!(refusals > 0, "the sweep exercised {refusals} refusals");

    let mut wide = 0;
    let mut w = 1600.0f32;
    while w <= 5120.0 {
        let mut h = 190.0f32;
        while h <= 700.0 {
            let space = frame(w, h);
            match (
                WIDE_SPAN.choose_frame(space),
                largest_by_scan(&WIDE_SPAN, space),
            ) {
                (Ok(chosen), Some(best)) => {
                    assert_eq!(
                        chosen.width * chosen.height,
                        best.width * best.height,
                        "for {w}x{h} chose {chosen:?}, scan found {best:?}"
                    );
                    wide += 1;
                }
                (Err(_), None) => {}
                (Ok(chosen), None) => panic!("chose {chosen:?} for {w}x{h}, scan refuses"),
                (Err(why), Some(best)) => panic!("refused {w}x{h} as {why:?}, {best:?} fits"),
            }
            h += 97.0;
        }
        w += 611.0;
    }
    assert!(wide > 20, "the wide sweep agreed on {wide} answers");
}

#[test]
fn a_floor_never_lands_above_the_value_it_came_from() {
    let space = 800.3f32;
    let floored = super::floor_on_grid(space, 360.0, 1.7).expect("a finite grid");
    assert!(
        floored <= space,
        "floored {floored} above {space}, so a frame would exceed its space"
    );
}
