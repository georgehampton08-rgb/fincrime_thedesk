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

    // ── Phase 2.5: Complaint analytics events ────────────────────
    ComplaintWarningFired {
        tick: Tick,
        alert_type: String,
        severity: String,
        segment: Option<String>,
    },

    // ── Phase 2.6: Risk appetite events ─────────────────────
    RiskDialChanged {
        tick: Tick,
        dial_id: String,
        old_value: f64,
        new_value: f64,
        warnings: Option<String>,
    },

    RiskDialRejected {
        tick: Tick,
        dial_id: String,
        attempted_value: f64,
        reason: String,
    },

    BoardPressureFired {
        tick: Tick,
        pressure_type: String,
        message: String,
        severity: String,
    },

    // ── Phase 3.1: Payment rail events ──────────────────────────────
    PaymentBatchCreated {
        tick: Tick,
        batch_id: String,
        rail_id: String,
        item_count: i64,
        total_amount: f64,
    },
    PaymentBatchSettled {
        tick: Tick,
        batch_id: String,
        rail_id: String,
        exceptions: i64,
    },
    PaymentBatchFailed {
        tick: Tick,
        batch_id: String,
        rail_id: String,
        reason: String,
    },
    CardAuthorizationCreated {
        tick: Tick,
        authorization_id: String,
        account_id: String,
        amount: f64,
        merchant_name: String,
    },
    CardSettled {
        tick: Tick,
        authorization_id: String,
        original_auth_amount: f64,
        settled_amount: f64,
    },

    // ── Phase 3.2: Reconciliation events ────────────────────────────
    ReconExceptionCreated {
        tick: Tick,
        exception_id: String,
        rail_id: String,
        delta_amount: f64,
    },
    ReconExceptionAutoCleared {
        tick: Tick,
        exception_id: String,
        delta_amount: f64,
    },
    ReconExceptionSLABreach {
        tick: Tick,
        exception_id: String,
        age_days: Tick,
    },
    ReconExceptionEscalated {
        tick: Tick,
        exception_id: String,
        reason: String, // 'age' or 'amount'
    },
    ReconExceptionResolved {
        tick: Tick,
        exception_id: String,
        resolution_type: String,
        write_off_amount: f64,
    },

    // ── Phase 3.5-prep: Identity & Address events ──────────────────────────
    /// Fired once per customer at onboarding when identity record is persisted.
    CustomerIdentityCreated {
        tick: Tick,
        customer_id: EntityId,
        ssn_status: String,    // 'valid' | 'synthetic'
        identity_type: String, // 'natural_person' | 'synthetic'
    },
    /// Fired when multiple customers are found sharing the same physical address.
    AddressSharingAlert {
        tick: Tick,
        address_key: String,   // "<street>, <city>, <state> <zip>"
        customer_count: i64,
        alert_type: String,    // 'high_density' | 'potential_bust_out'
    },
    /// Fired when multiple customers share the same phone number.
    PhoneSharingAlert {
        tick: Tick,
        full_number: String,
        customer_count: i64,
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
