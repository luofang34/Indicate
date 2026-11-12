//! The source-selection surface of the crate, in one list.
//!
//! [`crate::source_compare`] compares the candidate sources of one
//! display function, and [`crate::source_monitor`] wires that comparison
//! into resolution. A consumer uses the two together, so the crate root
//! re-exports this list instead of one list for each module.

pub use crate::source_compare::{
    AirframeSourcePolicy, AttitudeMeasure, Candidate, Comparable, ComparisonState, FrameTag,
    HeadingMeasure, IntegrityLevel, MAX_SOURCES, ScalarMeasure, ScalarUnit, SourceAltitude,
    SourceComparator, SourceComparison, SourceEpoch, SourceId, SourceList, SourcePolicyError,
    SourcePolicyLimits, VectorMeasure,
};
pub use crate::source_monitor::{
    SourceInputs, SourceMonitorReport, SourceMonitors, SourcePolicies, SourceSelection, SourceStep,
    Sourced, SourcedFunction, resolve_with_sources,
};
