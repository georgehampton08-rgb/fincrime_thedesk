use serde::{Deserialize, Serialize};
use crate::types::{RunId, Tick};

/// All player-issued commands.
/// Variants added per phase — never removed or reordered.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub enum PlayerCommand {
    // ── Clock control ─────────────────────────────
    Pause,
    Resume,
    SetSpeed { speed: crate::clock::SimSpeed },

    // ── Phase 1C ──────────────────────────────────
    CloseComplaint {
        complaint_id:    String,
        resolution_code: String,
    },

    // ── Phase 1D+ ─────────────────────────────────
    // SetProductFee { product_id: String, fee_type: String, amount: f64 },

    // ── Phase 2+ ──────────────────────────────────
    // SetRiskAppetite { parameter: String, value: f64 },
    // CreateOffer { .. },
}

/// A queued player command with its submission tick.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueuedCommand {
    pub run_id:     RunId,
    pub queued_at:  Tick,
    pub command_id: String,
    pub command:    PlayerCommand,
}
