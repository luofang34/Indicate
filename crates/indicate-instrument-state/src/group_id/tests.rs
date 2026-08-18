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
fn reserved_and_batch_allocated_tags_have_no_status_slot_yet() {
    // 0x0E–0x11 stay planned, and 0x12–0x15 are allocated to the v8
    // batch (the registry table in the module doc) but their variants
    // land with the per-issue PRs. Until then none of these tags
    // resolves to a variant, so none can key a `GroupStatuses` slot.
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
fn index_is_not_wire_tag_arithmetic() {
    // `index()` must survive the first non-contiguous allocation: with
    // 0x0E–0x11 variantless, a `tag - 1` mapping would hand tag 0x12
    // slot 0x11, past the end of a 13-slot table. Pin the sparse
    // mapping's contract directly: the highest defined tag still maps
    // to the last slot.
    assert_eq!(GroupId::FlightDirector.to_u8(), 0x0D);
    assert_eq!(GroupId::FlightDirector.index(), GroupId::COUNT - 1);
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
