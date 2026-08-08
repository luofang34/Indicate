//! Typed errors for the xtask entry point.

/// Why an xtask command could not run.
#[derive(Debug, thiserror::Error)]
pub enum XtaskError {
    /// Argument parsing failed: an unknown command or a stray argument.
    #[error("usage error: {message}")]
    Usage {
        /// What was wrong with the arguments.
        message: String,
    },
    /// An I/O operation failed.
    #[error("I/O failure during {context}: {source}")]
    Io {
        /// What was being done.
        context: &'static str,
        /// The underlying OS error.
        #[source]
        source: std::io::Error,
    },
    /// A named file could not be read or written.
    #[error("{action} {path} failed: {source}")]
    File {
        /// What was attempted, as a participle: `reading`, `writing`.
        action: &'static str,
        /// The file involved.
        path: String,
        /// The underlying OS error.
        #[source]
        source: std::io::Error,
    },
    /// A value the manifest must state cannot be read from its source of
    /// truth, so the generator refuses to emit a manifest missing it.
    #[error("cannot pin {value}: {reason}")]
    UnpinnableValue {
        /// The manifest field that has no honest value.
        value: &'static str,
        /// Why the source of truth did not answer.
        reason: String,
    },
    /// A pinned constant disagrees with what this tree computes. The
    /// manifest states pinned values, so a manifest generated over a
    /// stale pin would launder the disagreement into a machine-readable
    /// claim.
    #[error("{value}: this tree computes {computed}, the pin declares {declared}")]
    PinMismatch {
        /// The value that disagrees.
        value: &'static str,
        /// What the tree computes now.
        computed: String,
        /// What the pinned constant says.
        declared: String,
    },
    /// The generator reads the state ABI version from one module, and
    /// the crate has since declared a newer one.
    #[error(
        "the generator reads the state ABI from {compiled}, but the crate declares {newest}; \
         point it at the newer module before it can state the version in force"
    )]
    AbiModuleDrift {
        /// The newest module the crate declares.
        newest: String,
        /// The module this generator was built against.
        compiled: String,
    },
    /// The shipped panel set does not compose into a valid registry, so
    /// there is no digest to record.
    #[error("the shipped panel set is not a valid registry: {source}")]
    Registry {
        /// The registry's own reason.
        #[source]
        source: indicate_instrument_registry::RegistryError,
    },
    /// A digest over the registry could not be computed.
    #[error("digest over the shipped panel set failed: {source}")]
    Digest {
        /// The digest's own reason.
        #[source]
        source: indicate_instrument_registry::DigestError,
    },
    /// The fixture screen composition was refused, so its digest is not
    /// a value any shell could reproduce.
    #[error("the fixture screen composition was refused: {source}")]
    Composition {
        /// The composition's own reason.
        #[source]
        source: indicate_instrument_registry::CompositionError,
    },
}
