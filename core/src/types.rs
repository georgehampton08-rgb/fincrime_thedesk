//! Shared primitive types used across the entire simulation.

/// A simulation tick. One tick = one in-game day.
pub type Tick = u64;

/// A stable, unique identifier for any entity in the simulation.
pub type EntityId = String;

/// The canonical run identifier.
pub type RunId = String;
