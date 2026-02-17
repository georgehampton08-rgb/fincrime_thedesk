use thiserror::Error;

#[derive(Error, Debug)]
pub enum SimError {
    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Invalid tick: expected {expected}, got {actual}")]
    TickMismatch { expected: u64, actual: u64 },

    #[error("Subsystem '{name}' not found")]
    SubsystemNotFound { name: String },

    #[error("Run not initialized")]
    RunNotInitialized,

    #[error("Determinism violation: state diverged at tick {tick}")]
    DeterminismViolation { tick: u64 },

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

pub type SimResult<T> = Result<T, SimError>;
