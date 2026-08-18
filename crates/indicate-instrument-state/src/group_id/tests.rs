//! Group-vocabulary invariants: bijective tags, canonical order, and
//! withholding that leaves no trace of the withheld group.

#![allow(clippy::expect_used, clippy::panic)]

use super::{GroupId, withhold_group};
use crate::abi::v8::fixtures;
use crate::signal::{FreshnessPolicy, SignalStatus};

#[test]
fn all_is_ascending_and_bijective_with_from_u8() {
    let mut prev = 0u8;
    for id in GroupId::ALL {
        assert!(id.to_u8() > prev, "ALL must ascend");
        prev = id.to_u8();
        assert_eq!(GroupId::from_u8(id.to_u8()), Some(id));
    }
    assert_eq!(GroupId::ALL.len(), GroupId::COUNT);
}

#[test]
fn unassigned_tags_do_not_resolve() {
    assert_eq!(GroupId::from_u8(0x00), None);
    for value in 0x16u8..=0xFF {
        assert_eq!(GroupId::from_u8(value), None, "tag {value:#04x}");
    }
}

#[test]
fn reserved_and_allocated_tags_have_no_status_slot_yet() {
    // 0x0E–0x11 are planned and 0x12–0x15 are allocated with their
    // layouts fixed, but neither has a variant. Until one lands, no such
    // tag resolves to a variant, so none can key a `GroupStatuses` slot.
    for value in 0x0Eu8..=0x15 {
        assert_eq!(GroupId::from_u8(value), None, "tag {value:#04x}");
    }
}

#[test]
fn index_is_dense_over_all() {
    for (position, id) in GroupId::ALL.iter().enumerate() {
        assert_eq!(id.index(), position);
    }
}

#[test]
fn the_highest_tag_maps_to_the_last_slot() {
    // Says only what it can say while every assigned id is contiguous:
    // `tag - 1` gives these same answers, so this does not distinguish
    // the match from arithmetic. The test below is what would catch the
    // arithmetic.
    assert_eq!(GroupId::FlightDirector.to_u8(), 0x0D);
    assert_eq!(GroupId::FlightDirector.index(), GroupId::COUNT - 1);
}

#[test]
fn wire_tag_arithmetic_would_index_past_the_table() {
    // The next id the registry allocates is not the next tag after the
    // last variant, so the first allocation to gain a variant makes
    // `tag - 1` index past a `[SignalStatus; COUNT]` table. `index()` is
    // a match for this reason; this test fails if a future allocation
    // ever makes the arithmetic safe again, at which point the reason
    // has to be restated rather than silently lost.
    const NEXT_ALLOCATED: u8 = 0x12;
    assert_eq!(GroupId::from_u8(NEXT_ALLOCATED), None, "still variantless");
    let arithmetic_slot = usize::from(NEXT_ALLOCATED) - 1;
    assert!(
        arithmetic_slot >= GroupId::COUNT,
        "slot {arithmetic_slot} from tag arithmetic is still inside a \
         {}-slot table",
        GroupId::COUNT
    );
}

#[test]
fn withholding_a_stamped_group_resolves_missing() {
    let full = fixtures::full();
    let policy = FreshnessPolicy::default();
    for group in [
        GroupId::Attitude,
        GroupId::Kinematics,
        GroupId::Air,
        GroupId::Nav,
        GroupId::Wind,
        GroupId::Heading,
        GroupId::Variation,
        GroupId::Dynamics,
        GroupId::MonitorText,
    ] {
        let withheld = withhold_group(&full, group);
        let data = crate::resolve(&withheld, &policy);
        assert_eq!(
            data.groups.status(group),
            SignalStatus::Missing,
            "{group:?} must resolve Missing when withheld"
        );
    }
}

#[test]
fn withholding_is_idempotent_and_leaves_other_groups_fed() {
    let full = fixtures::full();
    let once = withhold_group(&full, GroupId::Air);
    let twice = withhold_group(&once, GroupId::Air);
    assert_eq!(once, twice);
    assert!(once.attitude.data.is_some());
    assert!(once.nav.data.is_some());
    assert!(once.air.data.is_none());
    assert_eq!(once.air.age_ms, None);
}

#[test]
fn withholding_trust_returns_the_fail_closed_defaults() {
    let full = fixtures::full();
    let withheld = withhold_group(&full, GroupId::Trust);
    assert_eq!(withheld.quality, Default::default());
    assert_eq!(withheld.valid, Default::default());
    assert_eq!(withheld.snapshot, Default::default());
}
