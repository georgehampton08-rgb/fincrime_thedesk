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

    // ── Phase 3.3: Incident & Outage events ──────────────────────────────
    IncidentCreated {
        tick: Tick,
        incident_id: String,
        component: String,
        severity: String,
        description: String,
    },
    IncidentResolved {
        tick: Tick,
        incident_id: String,
        component: String,
        duration_ticks: Tick,
    },
    IncidentSLABreach {
        tick: Tick,
        incident_id: String,
        severity: String,
        ticks_overdue: Tick,
    },
    ComponentStatusChanged {
        tick: Tick,
        component_id: String,
        old_status: String,
        new_status: String,
        reason: String,
    },
    ComponentUpgradeStarted {
        tick: Tick,
        component: String,
        from_tier: String,
        to_tier: String,
        cost: f64,
        duration_ticks: Tick,
    },
    ComponentUpgradeCompleted {
        tick: Tick,
        component: String,
        new_tier: String,
    },
    CascadingImpactApplied {
        tick: Tick,
        incident_id: String,
        impact_type: String,
        affected_component: String,
        impact_value: f64,
    },
    SystemMetricsComputed {
        tick: Tick,
        component_id: String,
        uptime_pct_30d: f64,
        total_incidents_30d: i64,
    },
    // ── Phase 3.4: Card Dispute & Chargeback events ───────────────────────────
    DisputeFiled {
        tick: Tick,
        dispute_id: String,
        authorization_id: String,
        customer_id: String,
        amount: f64,
        reason: String,
    },
    DisputeStatusChanged {
        tick: Tick,
        dispute_id: String,
        old_status: String,
        new_status: String,
    },
    ProvisionalCreditIssued {
        tick: Tick,
        dispute_id: String,
        account_id: String,
        amount: f64,
    },
    DisputeResolved {
        tick: Tick,
        dispute_id: String,
        outcome: String,
        customer_won: bool,
    },
    ChargebackIssued {
        tick: Tick,
        dispute_id: String,
        amount: f64,
        merchant_name: String,
    },
    FriendlyFraudDetected {
        tick: Tick,
        dispute_id: String,
        customer_id: String,
        fraud_score: f64,
    },
    ChargebackMetricsComputed {
        tick: Tick,
        disputes_filed_7d: i64,
        win_rate_7d: f64,
        chargeback_amount_7d: f64,
    },
    // Phase 3.5: Fraud Detection & AML
    FraudPatternDetected {
        tick: Tick,
        pattern_id: String,
        pattern_type: String,
        customer_id: String,
        confidence_score: f64,
    },
    FraudAlertGenerated {
        tick: Tick,
        alert_id: String,
        alert_type: String,
        entity_id: String,
        fraud_score: f64,
        severity: String,
    },
    // Phase 3.5 Week 4: AML Screening & Risk Rating
    AMLScreeningHit {
        tick: Tick,
        screening_id: String,
        screening_type: String,
        customer_id: String,
        match_type: String,
        match_score: f64,
    },
    AMLAlertGenerated {
        tick: Tick,
        alert_id: String,
        alert_type: String,
        customer_id: String,
        severity: String,
        risk_score: f64,
    },
    AMLRiskRatingComputed {
        tick: Tick,
        customer_id: String,
        risk_rating: String,
        risk_score: f64,
        requires_edd: bool,
    },
    AMLMetricsComputed {
        tick: Tick,
        screenings_7d: i64,
        sanctions_hits_7d: i64,
        pep_matches_7d: i64,
        alerts_generated_7d: i64,
    },

    // Phase 3.5 Week 5: Transaction Monitoring
    TransactionMonitoringAlert {
        tick: Tick,
        alert_id: String,
        alert_type: String,
        customer_id: String,
        alert_score: f64,
        description: String,
    },
    CTRFiled {
        tick: Tick,
        ctr_id: String,
        customer_id: String,
        amount: f64,
        transaction_type: String,
    },
    TransactionMonitoringMetricsComputed {
        tick: Tick,
        alerts_generated: i64,
        ctrs_filed: i64,
    },

    // Phase 3.5 Week 6: SAR Filing & Integration
    SARFiled {
        tick: Tick,
        sar_id: String,
        customer_id: String,
        activity_type: String,
        suspicious_amount: f64,
    },
    SARLateFiling {
        tick: Tick,
        sar_id: String,
        customer_id: String,
        days_late: i64,
        regulatory_fine: f64,
    },
    SARMetricsComputed {
        tick: Tick,
        sars_filed: i64,
        sars_late: i64,
        total_fines: f64,
    },

    // ── Phase 3.6: Regulatory Examination ─────────────────────────
    RegulatoryExamStarted {
        tick: Tick,
        exam_id: String,
        examiner: String,
        scope: String,
    },
    ExamFindingRecorded {
        tick: Tick,
        exam_id: String,
        finding_id: String,
        category: String,
        severity: String,
        fine_amount: f64,
    },
    RegulatoryExamClosed {
        tick: Tick,
        exam_id: String,
        examiner: String,
        finding_count: i64,
        fine_total: f64,
        mou_issued: bool,
    },
    MOUReceived {
        tick: Tick,
        exam_id: String,
        examiner: String,
        fine_total: f64,
    },

    // ── Phase 3.6: Reputation Management ──────────────────────────
    ReputationUpdated {
        tick: Tick,
        score: f64,
        delta: f64,
        primary_driver: String,
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
