//! The event bus — all inter-subsystem communication.
//!
//! RULE: Subsystems communicate ONLY through events.
//! A subsystem may never call another subsystem's functions directly.
//! A subsystem may never read another subsystem's internal state.

use crate::types::{EntityId, RunId, Tick};
use serde::{Deserialize, Serialize};

/// Every event emitted during simulation.
/// Variants are added per phase — never removed or reordered.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SimEvent {
    // ── Engine events ──────────────────────────────
    TickStarted    { tick: Tick },
    TickCompleted  { tick: Tick },
    RunInitialized { run_id: RunId, seed: u64 },

    // ── Macro events ───────────────────────────────
    MacroStateUpdated {
        tick: Tick,
        base_rate: f64,
        economic_phase: EconomicPhase,
        fraud_multiplier: f64,
    },

    // ── Player command events ──────────────────────
    PlayerCommandReceived {
        tick: Tick,
        command_id: EntityId,
        command_type: String,
    },

    // ── Phase 1+ events added here as stubs ────────
    // CustomerOnboarded   { .. }
    // TransactionBooked   { .. }
    // ComplaintFiled      { .. }
    // etc.
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EconomicPhase {
    Expansion,
    Peak,
    Contraction,
    Trough,
}

impl EconomicPhase {
    pub fn fraud_multiplier(&self) -> f64 {
        match self {
            Self::Expansion   => 1.0,
            Self::Peak        => 1.1,
            Self::Contraction => 1.35,
            Self::Trough      => 1.6,
        }
    }
}

/// The event log entry as persisted to SQLite.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventLogEntry {
    pub id:         Option<i64>,
    pub run_id:     RunId,
    pub tick:       Tick,
    pub subsystem:  String,
    pub event_type: String,
    pub payload:    String, // JSON-serialized SimEvent
}
