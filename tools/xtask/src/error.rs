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
}
