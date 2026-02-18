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
    TickStarted {
        tick: Tick,
    },
    TickCompleted {
        tick: Tick,
    },
    RunInitialized {
        run_id: RunId,
        seed: u64,
    },

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

    // ── Phase 1B: Customer and transaction events ──
    CustomerOnboarded {
        tick: Tick,
        customer_id: EntityId,
        segment: String,
        account_id: EntityId,
    },
    CustomerChurned {
        tick: Tick,
        customer_id: EntityId,
        segment: String,
        churn_risk: f64,
    },
    FeeCharged {
        tick: Tick,
        customer_id: EntityId,
        account_id: EntityId,
        fee_type: String, // "overdraft" | "nsf" | "atm"
        amount: f64,
    },

    // ── Phase 1C: Complaint and service events ─────
    ComplaintFiled {
        tick: Tick,
        complaint_id: EntityId,
        customer_id: EntityId,
        issue: String,
        priority: String,
    },
    ComplaintResolved {
        tick: Tick,
        complaint_id: EntityId,
        customer_id: EntityId,
        resolution_code: String,
        satisfaction_delta: f64,
    },
    SLABreached {
        tick: Tick,
        complaint_id: EntityId,
        customer_id: EntityId,
        days_overdue: i32,
    },

    // ── Phase 1D: Economics events ─────────────────────────────
    QuarterlyPnLComputed {
        tick: Tick,
        period: String,
        gross_income: f64,
        pre_tax_profit: f64,
        nim: f64,
        efficiency_ratio: f64,
    },

    // ── Phase 2.1: Pricing events ───────────────────────────────
    ProductFeeChanged {
        tick: Tick,
        product_id: EntityId,
        fee_type: String,
        old_value: f64,
        new_value: f64,
        warning: Option<String>,
    },

    FeeChangeRejected {
        tick: Tick,
        product_id: EntityId,
        fee_type: String,
        reason: String,
    },

    // ── Phase 2.2: Offer events ─────────────────────────────────
    OfferMatched {
        tick: Tick,
        customer_id: EntityId,
        offer_id: String,
        bonus_amount: f64,
    },

    OfferCompleted {
        tick: Tick,
        customer_id: EntityId,
        offer_id: String,
    },

    OfferBonusPaid {
        tick: Tick,
        customer_id: EntityId,
        offer_id: String,
        amount: f64,
        bonus_seeker_flag: bool,
    },

    // ── Phase 2.3: Churn events ─────────────────────────────────
    LifeEventOccurred {
        tick: Tick,
        customer_id: EntityId,
        event_type: String,
        duration: Tick,
    },
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
            Self::Expansion => 1.0,
            Self::Peak => 1.1,
            Self::Contraction => 1.35,
            Self::Trough => 1.6,
        }
    }
}

/// The event log entry as persisted to SQLite.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventLogEntry {
    pub id: Option<i64>,
    pub run_id: RunId,
    pub tick: Tick,
    pub subsystem: String,
    pub event_type: String,
    pub payload: String, // JSON-serialized SimEvent
}
