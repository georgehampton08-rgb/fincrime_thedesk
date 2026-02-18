//! Snapshot serialization — full simulation state to/from JSON.
//!
//! A snapshot is taken every SNAPSHOT_INTERVAL ticks.
//! It captures the complete state needed to resume simulation
//! from that tick without replaying from tick 0.

use crate::{
    clock::SimClock,
    types::{RunId, Tick},
};
use serde::{Deserialize, Serialize};

pub const SNAPSHOT_INTERVAL: Tick = 30; // monthly

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimSnapshot {
    pub run_id: RunId,
    pub tick: Tick,
    pub clock: SimClock,
    // Phase 1+ subsystem states added here as they are built.
    // Each subsystem exposes a SnapshotState struct.
}
