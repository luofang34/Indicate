//! Why a packet could not be encoded or decoded. Every variant is
//! fail-closed: a refused packet changes no state.

/// A wire refusal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum WireError {
    /// The packet does not start with the `RREF` reply header.
    #[error("packet is not an RREF reply")]
    NotRref,
    /// The packet ends inside a record: the producer wrote fewer bytes
    /// than a whole record, so nothing after the last whole one is
    /// trusted and the packet is refused rather than partly applied.
    #[error("RREF reply ends inside a record ({trailing} trailing bytes)")]
    TruncatedRecord {
        /// Bytes after the last whole record.
        trailing: usize,
    },
    /// A dataref path is longer than the fixed field the packet carries.
    #[error("dataref path of {len} bytes exceeds the {max}-byte field")]
    DatarefTooLong {
        /// The path length offered.
        len: usize,
        /// The field size.
        max: usize,
    },
    /// A dataref path contains a byte the field cannot carry (a NUL or a
    /// non-ASCII byte).
    #[error("dataref path contains a byte the field cannot carry")]
    DatarefNotAscii,
}
