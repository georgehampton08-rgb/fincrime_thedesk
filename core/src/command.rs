use crate::types::{RunId, Tick};
use serde::{Deserialize, Serialize};

/// All player-issued commands.
/// Variants added per phase — never removed or reordered.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub enum PlayerCommand {
    // ── Clock control ─────────────────────────────
    Pause,
    Resume,
    SetSpeed {
        speed: crate::clock::SimSpeed,
    },

    // ── Phase 1C ──────────────────────────────────
    CloseComplaint {
        complaint_id: String,
        resolution_code: String,
    },

    // ── Phase 2.1 ─────────────────────────────────
    SetProductFee {
        product_id: String,
        fee_type: String, // "monthly_fee" | "overdraft_fee" | "nsf_fee" | "atm_fee" | "wire_fee"
        new_value: f64,
    },
    // ── Phase 2.6 ─────────────────────────────────
    SetRiskDial {
        dial_id: String,
        new_value: f64,
    },
}

/// A queued player command with its submission tick.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueuedCommand {
    pub run_id: RunId,
    pub queued_at: Tick,
    pub command_id: String,
    pub command: PlayerCommand,
}
