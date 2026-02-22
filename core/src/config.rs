use crate::types::Tick;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductConfig {
    pub product_id: String,
    pub product_type: String,
    pub tier: String,
    pub label: String,
    pub monthly_fee: f64,
    pub overdraft_fee: f64,
    pub nsf_fee: f64,
    pub atm_fee: f64,
    pub wire_fee: f64,
    pub interest_rate: f64,
    pub min_balance_waive: Option<f64>,
    pub min_dd_waive: Option<f64>,
    pub target_segment: String,
}

#[derive(Debug, Clone, Deserialize)]
struct ProductCatalogFile {
    products: Vec<ProductConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SegmentConfig {
    pub id: String,
    pub label: String,
    pub population_share: f64,
    pub income_bands: Vec<String>,
    pub income_band_weights: Vec<f64>,
    pub monthly_txn_count_mean: f64,
    pub monthly_txn_count_std: f64,
    pub txn_amount_pareto_xmin: f64,
    pub txn_amount_pareto_alpha: f64,
    pub cash_intensity: f64,
    pub payroll_probability: f64,
    pub payroll_amount_mean: f64,
    pub payroll_amount_std: f64,
    pub overdraft_probability_per_tick: f64,
    pub nsf_probability_per_tick: f64,
    pub base_churn_rate_per_tick: f64,
    pub fee_sensitivity: f64,
    pub products: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplaintTrigger {
    pub event_type: String,
    #[serde(default)]
    pub fee_type: Option<String>,
    #[serde(default)]
    pub amount_threshold: Option<f64>,
    #[serde(default)]
    pub prior_breach: bool,
    pub probability: f64,
    pub issue_category: String,
    pub priority: String,
    pub sla_acknowledge_days: u64,
    pub sla_resolve_days: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolutionCode {
    pub code: String,
    pub satisfaction_delta: f64,
    pub churn_risk_delta: f64,
    pub avg_amount_refunded: f64,
}

#[derive(Debug, Clone, Deserialize)]
struct SegmentsFile {
    segments: Vec<SegmentConfig>,
}

#[derive(Debug, Clone, Deserialize)]
struct ComplaintConfigFile {
    triggers: Vec<ComplaintTrigger>,
    resolution_codes: Vec<ResolutionCode>,
}

// ── Phase 2.2: Offer catalog ───────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OfferConfig {
    pub offer_id: String,
    pub offer_type: String,
    pub label: String,
    pub product_id: Option<String>,
    pub bonus_amount: f64,
    pub requirements: OfferRequirements,
    pub eligibility: OfferEligibility,
    pub cost_model: OfferCostModel,
    pub fraud_risk: OfferFraudRisk,
    pub active: bool,
    pub start_tick: u64,
    pub end_tick: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OfferRequirements {
    pub min_direct_deposit: f64,
    pub min_balance: f64,
    pub duration_ticks: u64,
    pub new_to_bank_only: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OfferEligibility {
    pub target_segments: Vec<String>,
    pub exclude_segments: Vec<String>,
    pub min_credit_score: Option<f64>,
    pub max_existing_products: Option<usize>,
    #[serde(default)]
    pub min_churn_risk: Option<f64>,
    #[serde(default)]
    pub max_churn_risk: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OfferCostModel {
    pub bonus_paid_on_completion: bool,
    pub promo_rate_duration: u64,
    pub promo_rate_delta: f64,
    #[serde(default)]
    pub fee_waiver_duration: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OfferFraudRisk {
    pub bonus_seeker_probability: f64,
    pub velocity_flag_threshold: usize,
}

#[derive(Debug, Clone, Deserialize)]
struct OfferCatalogFile {
    offers: Vec<OfferConfig>,
}

// ── Phase 2.3: Churn model ─────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChurnModelConfig {
    pub model_version: String,
    pub update_frequency_ticks: Tick,
    pub segment_base_rates: HashMap<String, SegmentChurnParams>,
    pub churn_formula: ChurnFormulaWeights,
    pub life_events: Vec<LifeEventConfig>,
    pub churn_thresholds: ChurnThresholds,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SegmentChurnParams {
    pub monthly_churn_rate: f64,
    pub annual_churn_rate: f64,
    pub fee_sensitivity: f64,
    pub service_sensitivity: f64,
    pub offer_retention_effectiveness: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChurnFormulaWeights {
    pub base_rate_weight: f64,
    pub satisfaction_weight: f64,
    pub satisfaction_equilibrium: f64,
    pub fee_burden_weight: f64,
    pub fee_burden_threshold: f64,
    pub complaint_weight: f64,
    pub complaint_lookback_ticks: Tick,
    pub sla_breach_weight: f64,
    pub sla_breach_lookback_ticks: Tick,
    pub inactivity_weight: f64,
    pub inactivity_threshold_ticks: Tick,
    pub product_depth_bonus: f64,
    pub retention_offer_bonus: f64,
    pub life_event_multiplier: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LifeEventConfig {
    pub event_type: String,
    pub probability_per_year: f64,
    #[serde(default)]
    pub segments: Vec<String>,
    pub churn_risk_delta: f64,
    pub duration_ticks: Tick,
    pub behavioral_changes: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChurnThresholds {
    pub low_risk: f64,
    pub medium_risk: f64,
    pub high_risk: f64,
    pub imminent_churn: f64,
}

// ── Phase 2.4: Segment economics ───────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SegmentEconomicsConfig {
    pub cost_allocation_model: CostAllocationModel,
    pub clv_model: CLVModel,
    pub profitability_metrics: ProfitabilityMetrics,
    pub revenue_attribution: RevenueAttribution,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostAllocationModel {
    pub acquisition_cost_per_customer: HashMap<String, f64>,
    pub monthly_servicing_cost_per_customer: HashMap<String, f64>,
    pub complaint_handling_cost_per_complaint: HashMap<String, f64>,
    pub churn_replacement_cost_multiplier: f64,
    pub retention_offer_cost_fully_allocated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CLVModel {
    pub discount_rate_annual: f64,
    pub projection_horizon_years: u32,
    pub cross_sell_assumptions: serde_json::Value,
    pub default_tenure_assumptions: HashMap<String, Tick>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfitabilityMetrics {
    pub target_customer_margin: HashMap<String, f64>,
    pub warning_threshold_below_target: f64,
    pub cross_subsidy_flag_threshold: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevenueAttribution {
    pub interchange_revenue_per_swipe: f64,
    pub average_card_swipes_per_month: HashMap<String, u32>,
    pub nii_attribution_method: String,
    pub fee_attribution_method: String,
}

#[derive(Debug, Clone, Deserialize)]
struct SegmentEconomicsFile {
    cost_allocation_model: CostAllocationModel,
    clv_model: CLVModel,
    profitability_metrics: ProfitabilityMetrics,
    revenue_attribution: RevenueAttribution,
}

// ── Phase 2.5: Complaint analytics ─────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplaintAnalyticsConfig {
    pub pattern_detection: PatternDetectionConfig,
    pub root_cause_tracking: RootCauseConfig,
    pub resolution_effectiveness: ResolutionEffectivenessConfig,
    pub sla_performance: SLAPerformanceConfig,
    pub early_warning_indicators: EarlyWarningConfig,
    pub cost_analysis: ComplaintCostConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatternDetectionConfig {
    pub clustering_window_ticks: Tick,
    pub cluster_threshold_count: u32,
    pub velocity_spike_threshold: f64,
    pub issue_categories: HashMap<String, Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RootCauseConfig {
    pub fee_complaint_correlation_window: Tick,
    pub product_change_correlation_window: Tick,
    pub life_event_correlation_window: Tick,
    pub attribution_confidence_threshold: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolutionEffectivenessConfig {
    pub satisfaction_impact_weights: HashMap<String, f64>,
    pub churn_impact_weights: HashMap<String, f64>,
    pub effectiveness_measurement_window: Tick,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SLAPerformanceConfig {
    pub aging_buckets: Vec<serde_json::Value>,
    pub breach_prediction_thresholds: HashMap<String, f64>,
    pub critical_aging_threshold: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EarlyWarningConfig {
    pub velocity_comparison_periods: Vec<Tick>,
    pub breach_rate_warning_threshold: f64,
    pub repeat_complainer_threshold: u32,
    pub segment_concentration_warning: f64,
    pub issue_type_concentration_warning: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplaintCostConfig {
    pub average_handle_time_minutes: HashMap<String, u32>,
    pub loaded_hourly_rate: f64,
    pub escalation_cost_multiplier: f64,
    pub legal_review_cost: f64,
    pub write_off_authorization_cost: f64,
}

#[derive(Debug, Clone, Deserialize)]
struct ComplaintAnalyticsFile {
    pattern_detection: PatternDetectionConfig,
    root_cause_tracking: RootCauseConfig,
    resolution_effectiveness: ResolutionEffectivenessConfig,
    sla_performance: SLAPerformanceConfig,
    early_warning_indicators: EarlyWarningConfig,
    cost_analysis: ComplaintCostConfig,
}

// ── Phase 2.6: Risk appetite ──────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskAppetiteConfig {
    pub dials: Vec<DialConfig>,
    pub constraints: Vec<DialConstraint>,
    pub risk_profile_scoring: RiskProfileScoring,
    pub board_pressure: BoardPressureConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DialConfig {
    pub dial_id: String,
    pub label: String,
    pub description: String,
    pub min_value: f64,
    pub max_value: f64,
    pub default_value: f64,
    pub comfort_zone_min: f64,
    pub comfort_zone_max: f64,
    pub step_size: f64,
    pub impacts: HashMap<String, f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DialConstraint {
    pub constraint_id: String,
    pub description: String,
    pub condition: String,
    pub violation_message: String,
    pub enforcement: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskProfileScoring {
    pub dimensions: Vec<serde_json::Value>,
    pub risk_levels: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoardPressureConfig {
    pub comfort_zone_violation_threshold: u32,
    pub pressure_messages: HashMap<String, String>,
}

#[derive(Debug, Clone, Deserialize)]
struct RiskAppetiteFile {
    dials: Vec<DialConfig>,
    constraints: Vec<DialConstraint>,
    risk_profile_scoring: RiskProfileScoring,
    board_pressure: BoardPressureConfig,
}

// ── Phase 2.1: Fee constraints ─────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeeConstraint {
    pub fee_type: String,
    pub min_value: f64,
    pub max_value: f64,
    pub soft_limit: f64,
    pub soft_limit_warning: String,
    pub hard_limit_reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeeImpactFormula {
    #[serde(flatten)]
    pub parameters: serde_json::Value,
}

#[derive(Debug, Clone, Deserialize)]
struct FeeConstraintsFile {
    fee_types: Vec<FeeConstraint>,
    impact_formulas: HashMap<String, FeeImpactFormula>,
}

// ── Phase 3.1: Payment hub ────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentRailConfig {
    pub rail_id: String,
    pub rail_type: String,
    pub latency_type: String,
    pub settlement_delay_ticks: Tick,
    pub fraud_risk_multiplier: f64,
    pub operational_risk_base: f64,
    pub batch_window_ticks: Option<Tick>,
    pub cutoff_time_tick: Option<Tick>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentHubConfig {
    pub rails: Vec<PaymentRailConfig>,
    pub interchange_fee_rate: f64,
    pub auth_expiry_ticks: Tick,
}

#[derive(Debug, Clone, Deserialize)]
struct PaymentHubFile {
    rails: Vec<PaymentRailConfig>,
    interchange_fee_rate: f64,
    auth_expiry_ticks: Tick,
}

// ── Phase 3.2: Reconciliation config ─────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReconQueueConfig {
    pub rail_id: String,
    pub tolerance_amount: f64,
    pub auto_clear_threshold: f64,
    pub sla_days: i64,
    pub escalation_threshold: f64,
    pub escalation_age_days: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReconciliationConfig {
    pub queue_configs: Vec<ReconQueueConfig>,
    pub enable_auto_clear: bool,
    pub metrics_frequency_ticks: i64,
}

#[derive(Debug, Clone, Deserialize)]
struct ReconFile {
    queue_configs: Vec<ReconQueueConfig>,
    enable_auto_clear: bool,
    metrics_frequency_ticks: i64,
}

// ── Phase 3.5-prep: Identity & Address config ─────────────────────────────

/// A geographic region pool used for weighted random assignment of
/// state, city, zip prefix, SSN area-code range, and phone area codes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegionPool {
    pub region_id: String,
    pub city: String,
    pub state: String,
    pub zip_prefix: String,
    /// Inclusive range of SSN area codes historically associated with this region.
    pub ssn_area_range: (u16, u16),
    /// Relative population weight (used for deterministic weighted pick).
    pub weight: f64,
    /// Phone area codes active in this region.
    pub area_codes: Vec<String>,
}

/// Config for Tier-1 customer identity, address, and phone generation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdentityAddressConfig {
    pub regions: Vec<RegionPool>,
    /// Fraction of customers assigned synthetic SSN status (~0.02).
    pub synthetic_identity_rate: f64,
    /// Fraction of customers assigned homeless-shelter addresses (~0.015).
    pub homeless_rate: f64,
    /// Fraction of customers with P.O. box addresses (~0.03).
    pub po_box_rate: f64,
    /// Fraction of customers with CMRA addresses (~0.01).
    pub cmra_rate: f64,
    /// Alert threshold: number of customers sharing an address before alert fires.
    pub address_sharing_alert_threshold: i64,
    /// Fraction of customers whose primary phone is VoIP (~0.05).
    pub voip_rate: f64,
    /// Fraction of international customers (~0.008). Reserved for Tier 3.
    pub international_customer_rate: f64,
}

/// Internal file shape for identity_address.json
#[derive(Debug, Clone, Deserialize)]
struct IdentityRates {
    synthetic_identity_rate: f64,
    homeless_rate: f64,
    po_box_rate: f64,
    cmra_rate: f64,
    address_sharing_alert_threshold: i64,
    voip_rate: f64,
    international_customer_rate: f64,
}

#[derive(Debug, Clone, Deserialize)]
struct IdentityAddressFile {
    regions: Vec<RegionPool>,
    rates: IdentityRates,
}

// ── Phase 3.3: Incident & Outage config ───────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TechnologyTierConfig {
    pub tier_id: String,
    pub label: String,
    pub mtbf_multiplier: f64,
    pub mttr_multiplier: f64,
    pub degradation_prob: f64,
    pub upgrade_cost: f64,
    pub upgrade_duration_ticks: Tick,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncidentConfig {
    pub enabled: bool,
    pub severity_weights: Vec<(String, f64)>,  // ordered, not HashMap
    pub sla_deadlines: Vec<(String, Tick)>,     // ordered, not HashMap
    pub cascading_failures_enabled: bool,
    pub metrics_interval_ticks: Tick,
}

// ── Phase 3.6: Regulatory Exam config ─────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegulatoryExamConfig {
    pub enabled: bool,
    /// How often an exam cycle starts (in ticks).
    pub exam_interval_ticks: Tick,
    /// Duration of an exam (in ticks).
    pub exam_duration_ticks: Tick,
    /// Ordered list of examiners (cycles round-robin).
    pub examiners: Vec<String>,
    /// Fine per minor finding.
    pub fine_minor: f64,
    /// Fine per moderate finding.
    pub fine_moderate: f64,
    /// Fine per major finding.
    pub fine_major: f64,
    /// Fine per critical finding.
    pub fine_critical: f64,
    /// Number of critical findings that trigger an MOU.
    pub mou_critical_threshold: u32,
}

// ── Phase 3.6: Reputation config ──────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReputationConfig {
    pub enabled: bool,
    /// Starting reputation score [0, 100].
    pub initial_score: f64,
    /// Daily passive recovery when score < 80.
    pub recovery_per_tick: f64,
    /// Reputation impact per $1k of regulatory fine.
    pub fine_impact_per_1k: f64,
    /// Flat impact for an MOU.
    pub mou_impact: f64,
    /// Impact per SAR late filing.
    pub sar_late_impact: f64,
    /// Impact per SLA breach (complaint or incident).
    pub sla_breach_impact: f64,
    /// Score below which onboarding rate is penalised.
    pub onboarding_penalty_threshold: f64,
    /// Fraction of new customers blocked when score is at 0.
    pub max_onboarding_penalty: f64,
}

#[derive(Debug, Clone)]
pub struct SimConfig {
    pub segments: HashMap<String, SegmentConfig>,
    pub initial_population: usize,
    pub complaint_triggers: Vec<ComplaintTrigger>,
    pub resolution_codes: HashMap<String, ResolutionCode>,
    pub products: HashMap<String, ProductConfig>,
    pub fee_constraints: HashMap<String, FeeConstraint>,
    pub impact_formulas: HashMap<String, FeeImpactFormula>,
    pub offers: HashMap<String, OfferConfig>,
    pub churn_model: ChurnModelConfig,
    pub segment_economics: SegmentEconomicsConfig,
    pub complaint_analytics: ComplaintAnalyticsConfig,
    pub risk_appetite: RiskAppetiteConfig,
    pub payment_hub: PaymentHubConfig,
    pub reconciliation: ReconciliationConfig,
    pub identity_address: IdentityAddressConfig,
    pub incident: IncidentConfig,
    pub regulatory_exam: RegulatoryExamConfig,
    pub reputation: ReputationConfig,
}

impl SimConfig {
    /// Load from the data/ directory.
    /// In tests, use SimConfig::default_test().
    pub fn load(data_dir: &str) -> anyhow::Result<Self> {
        let path = format!("{data_dir}/segments/segments.json");
        let content = std::fs::read_to_string(&path)
            .map_err(|e| anyhow::anyhow!("Cannot read {path}: {e}"))?;
        let file: SegmentsFile = serde_json::from_str(&content)?;
        let segments = file
            .segments
            .into_iter()
            .map(|s| (s.id.clone(), s))
            .collect();

        let complaint_path = format!("{data_dir}/complaints/complaint_triggers.json");
        let complaint_content = std::fs::read_to_string(&complaint_path)
            .map_err(|e| anyhow::anyhow!("Cannot read {complaint_path}: {e}"))?;
        let complaint_file: ComplaintConfigFile = serde_json::from_str(&complaint_content)?;
        let resolution_codes = complaint_file
            .resolution_codes
            .into_iter()
            .map(|r| (r.code.clone(), r))
            .collect();

        let product_path = format!("{data_dir}/products/product_catalog.json");
        let product_content = std::fs::read_to_string(&product_path)
            .map_err(|e| anyhow::anyhow!("Cannot read {product_path}: {e}"))?;
        let product_file: ProductCatalogFile = serde_json::from_str(&product_content)?;
        let products = product_file
            .products
            .into_iter()
            .map(|p| (p.product_id.clone(), p))
            .collect();

        let fee_path = format!("{data_dir}/products/fee_constraints.json");
        let fee_content = std::fs::read_to_string(&fee_path)
            .map_err(|e| anyhow::anyhow!("Cannot read {fee_path}: {e}"))?;
        let fee_file: FeeConstraintsFile = serde_json::from_str(&fee_content)?;
        let fee_constraints = fee_file
            .fee_types
            .into_iter()
            .map(|f| (f.fee_type.clone(), f))
            .collect();

        let offer_path = format!("{data_dir}/offers/offer_catalog.json");
        let offer_content = std::fs::read_to_string(&offer_path)
            .map_err(|e| anyhow::anyhow!("Cannot read {offer_path}: {e}"))?;
        let offer_file: OfferCatalogFile = serde_json::from_str(&offer_content)?;
        let offers = offer_file
            .offers
            .into_iter()
            .map(|o| (o.offer_id.clone(), o))
            .collect();

        let churn_path = format!("{data_dir}/churn/churn_model_config.json");
        let churn_content = std::fs::read_to_string(&churn_path)
            .map_err(|e| anyhow::anyhow!("Cannot read {churn_path}: {e}"))?;
        let churn_model: ChurnModelConfig = serde_json::from_str(&churn_content)?;

        let seg_econ_path = format!("{data_dir}/economics/segment_economics_config.json");
        let seg_econ_content = std::fs::read_to_string(&seg_econ_path)
            .map_err(|e| anyhow::anyhow!("Cannot read {seg_econ_path}: {e}"))?;
        let seg_econ_file: SegmentEconomicsFile = serde_json::from_str(&seg_econ_content)?;
        let segment_economics = SegmentEconomicsConfig {
            cost_allocation_model: seg_econ_file.cost_allocation_model,
            clv_model: seg_econ_file.clv_model,
            profitability_metrics: seg_econ_file.profitability_metrics,
            revenue_attribution: seg_econ_file.revenue_attribution,
        };

        let complaint_analytics_path = format!("{data_dir}/complaints/analytics_config.json");
        let complaint_analytics_content = std::fs::read_to_string(&complaint_analytics_path)
            .map_err(|e| anyhow::anyhow!("Cannot read {complaint_analytics_path}: {e}"))?;
        let complaint_analytics_file: ComplaintAnalyticsFile =
            serde_json::from_str(&complaint_analytics_content)?;
        let complaint_analytics = ComplaintAnalyticsConfig {
            pattern_detection: complaint_analytics_file.pattern_detection,
            root_cause_tracking: complaint_analytics_file.root_cause_tracking,
            resolution_effectiveness: complaint_analytics_file.resolution_effectiveness,
            sla_performance: complaint_analytics_file.sla_performance,
            early_warning_indicators: complaint_analytics_file.early_warning_indicators,
            cost_analysis: complaint_analytics_file.cost_analysis,
        };

        let risk_path = format!("{data_dir}/risk/risk_appetite_config.json");
        let risk_content = std::fs::read_to_string(&risk_path)
            .map_err(|e| anyhow::anyhow!("Cannot read {risk_path}: {e}"))?;
        let risk_file: RiskAppetiteFile = serde_json::from_str(&risk_content)?;
        let risk_appetite = RiskAppetiteConfig {
            dials: risk_file.dials,
            constraints: risk_file.constraints,
            risk_profile_scoring: risk_file.risk_profile_scoring,
            board_pressure: risk_file.board_pressure,
        };

        let payment_path = format!("{data_dir}/payment/payment_rails_config.json");
        let payment_content = std::fs::read_to_string(&payment_path)
            .map_err(|e| anyhow::anyhow!("Cannot read {payment_path}: {e}"))?;
        let payment_file: PaymentHubFile = serde_json::from_str(&payment_content)?;
        let payment_hub = PaymentHubConfig {
            rails: payment_file.rails,
            interchange_fee_rate: payment_file.interchange_fee_rate,
            auth_expiry_ticks: payment_file.auth_expiry_ticks,
        };

        let identity_address = {
            let ia_path = format!("{data_dir}/identity/identity_address.json");
            let ia_content = std::fs::read_to_string(&ia_path)
                .map_err(|e| anyhow::anyhow!("Cannot read {ia_path}: {e}"))?;
            let ia_file: IdentityAddressFile = serde_json::from_str(&ia_content)?;
            IdentityAddressConfig {
                regions: ia_file.regions,
                synthetic_identity_rate: ia_file.rates.synthetic_identity_rate,
                homeless_rate: ia_file.rates.homeless_rate,
                po_box_rate: ia_file.rates.po_box_rate,
                cmra_rate: ia_file.rates.cmra_rate,
                address_sharing_alert_threshold: ia_file.rates.address_sharing_alert_threshold,
                voip_rate: ia_file.rates.voip_rate,
                international_customer_rate: ia_file.rates.international_customer_rate,
            }
        };

        Ok(Self {
            segments,
            initial_population: 500,
            complaint_triggers: complaint_file.triggers,
            resolution_codes,
            products,
            fee_constraints,
            impact_formulas: fee_file.impact_formulas,
            offers,
            churn_model,
            segment_economics,
            complaint_analytics,
            risk_appetite,
            payment_hub,
            reconciliation: {
                let recon_path =
                    format!("{data_dir}/reconciliation/recon_queue_config.json");
                let recon_content = std::fs::read_to_string(&recon_path)
                    .map_err(|e| anyhow::anyhow!("Cannot read {recon_path}: {e}"))?;
                let recon_file: ReconFile = serde_json::from_str(&recon_content)?;
                ReconciliationConfig {
                    queue_configs: recon_file.queue_configs,
                    enable_auto_clear: recon_file.enable_auto_clear,
                    metrics_frequency_ticks: recon_file.metrics_frequency_ticks,
                }
            },
            identity_address,
            incident: IncidentConfig {
                enabled: true,
                severity_weights: vec![
                    ("P0".into(), 0.05),
                    ("P1".into(), 0.15),
                    ("P2".into(), 0.30),
                    ("P3".into(), 0.50),
                ],
                sla_deadlines: vec![
                    ("P0".into(), 0),
                    ("P1".into(), 0),
                    ("P2".into(), 1),
                    ("P3".into(), 3),
                ],
                cascading_failures_enabled: true,
                metrics_interval_ticks: 7,
            },
            regulatory_exam: RegulatoryExamConfig {
                enabled: true,
                exam_interval_ticks: 90,
                exam_duration_ticks: 14,
                examiners: vec!["OCC".into(), "CFPB".into(), "FDIC".into()],
                fine_minor: 5_000.0,
                fine_moderate: 50_000.0,
                fine_major: 250_000.0,
                fine_critical: 1_000_000.0,
                mou_critical_threshold: 2,
            },
            reputation: ReputationConfig {
                enabled: true,
                initial_score: 75.0,
                recovery_per_tick: 0.05,
                fine_impact_per_1k: 0.1,
                mou_impact: 10.0,
                sar_late_impact: 2.0,
                sla_breach_impact: 0.5,
                onboarding_penalty_threshold: 40.0,
                max_onboarding_penalty: 0.50,
            },
        })
    }

    /// Config with hardcoded defaults for use in unit tests.
    pub fn default_test() -> Self {
        let seg = SegmentConfig {
            id: "mass_market".into(),
            label: "Mass Market".into(),
            population_share: 0.70,
            income_bands: vec!["low".into()],
            income_band_weights: vec![1.0],
            monthly_txn_count_mean: 20.0,
            monthly_txn_count_std: 4.0,
            txn_amount_pareto_xmin: 15.0,
            txn_amount_pareto_alpha: 1.8,
            cash_intensity: 0.35,
            payroll_probability: 0.5,
            payroll_amount_mean: 2000.0,
            payroll_amount_std: 400.0,
            overdraft_probability_per_tick: 0.005,
            nsf_probability_per_tick: 0.002,
            base_churn_rate_per_tick: 0.001,
            fee_sensitivity: 0.8,
            products: vec!["basic_checking".into()],
        };

        let seg_biz = SegmentConfig {
            id: "small_business".into(),
            label: "Small Business".into(),
            population_share: 0.20,
            income_bands: vec!["medium".into(), "high".into()],
            income_band_weights: vec![0.6, 0.4],
            monthly_txn_count_mean: 40.0,
            monthly_txn_count_std: 10.0,
            txn_amount_pareto_xmin: 50.0,
            txn_amount_pareto_alpha: 1.5,
            cash_intensity: 0.50,
            payroll_probability: 0.8,
            payroll_amount_mean: 5000.0,
            payroll_amount_std: 1500.0,
            overdraft_probability_per_tick: 0.008,
            nsf_probability_per_tick: 0.004,
            base_churn_rate_per_tick: 0.0008,
            fee_sensitivity: 0.5,
            products: vec!["basic_checking".into()],
        };

        let seg_premium = SegmentConfig {
            id: "premium".into(),
            label: "Premium".into(),
            population_share: 0.10,
            income_bands: vec!["high".into()],
            income_band_weights: vec![1.0],
            monthly_txn_count_mean: 30.0,
            monthly_txn_count_std: 6.0,
            txn_amount_pareto_xmin: 100.0,
            txn_amount_pareto_alpha: 1.3,
            cash_intensity: 0.10,
            payroll_probability: 0.7,
            payroll_amount_mean: 8000.0,
            payroll_amount_std: 2000.0,
            overdraft_probability_per_tick: 0.002,
            nsf_probability_per_tick: 0.001,
            base_churn_rate_per_tick: 0.0005,
            fee_sensitivity: 0.3,
            products: vec!["basic_checking".into()],
        };

        let triggers = vec![ComplaintTrigger {
            event_type: "fee_charged".into(),
            fee_type: Some("overdraft".into()),
            amount_threshold: None,
            prior_breach: false,
            probability: 0.12,
            issue_category: "fee_dispute".into(),
            priority: "standard".into(),
            sla_acknowledge_days: 2,
            sla_resolve_days: 15,
        }];

        let resolution_codes = [
            (
                "explanation_only".into(),
                ResolutionCode {
                    code: "explanation_only".into(),
                    satisfaction_delta: -0.02,
                    churn_risk_delta: 0.03,
                    avg_amount_refunded: 0.0,
                },
            ),
            (
                "monetary_relief".into(),
                ResolutionCode {
                    code: "monetary_relief".into(),
                    satisfaction_delta: 0.15,
                    churn_risk_delta: -0.10,
                    avg_amount_refunded: 27.08,
                },
            ),
        ]
        .into();

        let products = [(
            "basic_checking".into(),
            ProductConfig {
                product_id: "basic_checking".into(),
                product_type: "checking".into(),
                tier: "basic".into(),
                label: "Basic Checking".into(),
                monthly_fee: 0.0,
                overdraft_fee: 27.08,
                nsf_fee: 17.72,
                atm_fee: 2.50,
                wire_fee: 25.0,
                interest_rate: 0.0,
                min_balance_waive: None,
                min_dd_waive: None,
                target_segment: "mass_market".into(),
            },
        )]
        .into();

        let fee_constraints = [
            (
                "overdraft_fee".into(),
                FeeConstraint {
                    fee_type: "overdraft_fee".into(),
                    min_value: 0.0,
                    max_value: 35.0,
                    soft_limit: 29.0,
                    soft_limit_warning: "Overdraft fees above $29 add +0.10 to UDAAP risk score"
                        .into(),
                    hard_limit_reason: "FDIC guidance ceiling of $35 per overdraft event".into(),
                },
            ),
            (
                "monthly_fee".into(),
                FeeConstraint {
                    fee_type: "monthly_fee".into(),
                    min_value: 0.0,
                    max_value: 30.0,
                    soft_limit: 20.0,
                    soft_limit_warning:
                        "Fees above $20/month trigger 1.4x complaint rate multiplier".into(),
                    hard_limit_reason: "Federal disclosure requirements limit monthly fees to $30"
                        .into(),
                },
            ),
            (
                "nsf_fee".into(),
                FeeConstraint {
                    fee_type: "nsf_fee".into(),
                    min_value: 0.0,
                    max_value: 25.0,
                    soft_limit: 20.0,
                    soft_limit_warning: "NSF fees above $20 add +0.08 to UDAAP risk score".into(),
                    hard_limit_reason: "Industry best practice ceiling of $25".into(),
                },
            ),
            (
                "atm_fee".into(),
                FeeConstraint {
                    fee_type: "atm_fee".into(),
                    min_value: 0.0,
                    max_value: 8.0,
                    soft_limit: 5.0,
                    soft_limit_warning: "ATM fees above $5 trigger satisfaction delta -0.05".into(),
                    hard_limit_reason: "No regulatory ceiling, competitive pressure limits to ~$8"
                        .into(),
                },
            ),
            (
                "wire_fee".into(),
                FeeConstraint {
                    fee_type: "wire_fee".into(),
                    min_value: 0.0,
                    max_value: 50.0,
                    soft_limit: 35.0,
                    soft_limit_warning:
                        "Wire fees above $35 increase premium segment churn sensitivity +0.20"
                            .into(),
                    hard_limit_reason: "No regulatory ceiling, market-driven limit".into(),
                },
            ),
        ]
        .into();

        let impact_formulas = [
            (
                "overdraft_fee".into(),
                FeeImpactFormula {
                    parameters: serde_json::json!({
                        "udaap_risk_threshold": 29.0,
                        "udaap_risk_delta": 0.10
                    }),
                },
            ),
            (
                "nsf_fee".into(),
                FeeImpactFormula {
                    parameters: serde_json::json!({
                        "udaap_risk_threshold": 20.0,
                        "udaap_risk_delta": 0.08
                    }),
                },
            ),
        ]
        .into();

        let offers = [(
            "signup_bonus_100".into(),
            OfferConfig {
                offer_id: "signup_bonus_100".into(),
                offer_type: "signup_cash_bonus".into(),
                label: "$100 Sign-Up Bonus".into(),
                product_id: Some("basic_checking".into()),
                bonus_amount: 100.0,
                requirements: OfferRequirements {
                    min_direct_deposit: 500.0,
                    min_balance: 100.0,
                    duration_ticks: 60,
                    new_to_bank_only: true,
                },
                eligibility: OfferEligibility {
                    target_segments: vec!["mass_market".into()],
                    exclude_segments: vec![],
                    min_credit_score: None,
                    max_existing_products: None, // allow customers with their first account
                    min_churn_risk: None,
                    max_churn_risk: None,
                },
                cost_model: OfferCostModel {
                    bonus_paid_on_completion: true,
                    promo_rate_duration: 0,
                    promo_rate_delta: 0.0,
                    fee_waiver_duration: None,
                },
                fraud_risk: OfferFraudRisk {
                    bonus_seeker_probability: 0.15,
                    velocity_flag_threshold: 3,
                },
                active: true,
                start_tick: 0,
                end_tick: None,
            },
        )]
        .into();

        let churn_model = ChurnModelConfig {
            model_version: "2.3.0-test".into(),
            update_frequency_ticks: 30,
            segment_base_rates: [(
                "mass_market".into(),
                SegmentChurnParams {
                    monthly_churn_rate: 0.025,
                    annual_churn_rate: 0.26,
                    fee_sensitivity: 0.80,
                    service_sensitivity: 0.70,
                    offer_retention_effectiveness: 0.65,
                },
            )]
            .into(),
            churn_formula: ChurnFormulaWeights {
                base_rate_weight: 1.0,
                satisfaction_weight: 0.40,
                satisfaction_equilibrium: 0.65,
                fee_burden_weight: 0.25,
                fee_burden_threshold: 50.0,
                complaint_weight: 0.20,
                complaint_lookback_ticks: 90,
                sla_breach_weight: 0.35,
                sla_breach_lookback_ticks: 90,
                inactivity_weight: 0.15,
                inactivity_threshold_ticks: 60,
                product_depth_bonus: -0.08,
                retention_offer_bonus: -0.15,
                life_event_multiplier: 1.25,
            },
            life_events: vec![LifeEventConfig {
                event_type: "job_change".into(),
                probability_per_year: 0.15,
                segments: vec![],
                churn_risk_delta: 0.12,
                duration_ticks: 90,
                behavioral_changes: serde_json::json!({}),
            }],
            churn_thresholds: ChurnThresholds {
                low_risk: 0.30,
                medium_risk: 0.60,
                high_risk: 0.85,
                imminent_churn: 0.95,
            },
        };

        let segment_economics = SegmentEconomicsConfig {
            cost_allocation_model: CostAllocationModel {
                acquisition_cost_per_customer: [("mass_market".into(), 85.0)].into(),
                monthly_servicing_cost_per_customer: [("mass_market".into(), 4.50)].into(),
                complaint_handling_cost_per_complaint: [
                    ("standard".into(), 50.0),
                    ("high".into(), 120.0),
                    ("urgent".into(), 280.0),
                ]
                .into(),
                churn_replacement_cost_multiplier: 1.35,
                retention_offer_cost_fully_allocated: true,
            },
            clv_model: CLVModel {
                discount_rate_annual: 0.12,
                projection_horizon_years: 5,
                cross_sell_assumptions: serde_json::json!({}),
                default_tenure_assumptions: [("mass_market".into(), 730)].into(),
            },
            profitability_metrics: ProfitabilityMetrics {
                target_customer_margin: [("mass_market".into(), 0.18)].into(),
                warning_threshold_below_target: -0.10,
                cross_subsidy_flag_threshold: 0.20,
            },
            revenue_attribution: RevenueAttribution {
                interchange_revenue_per_swipe: 0.015,
                average_card_swipes_per_month: [("mass_market".into(), 18u32)].into(),
                nii_attribution_method: "balance_share".into(),
                fee_attribution_method: "direct".into(),
            },
        };

        let complaint_analytics = ComplaintAnalyticsConfig {
            pattern_detection: PatternDetectionConfig {
                clustering_window_ticks: 30,
                cluster_threshold_count: 5,
                velocity_spike_threshold: 1.5,
                issue_categories: HashMap::new(),
            },
            root_cause_tracking: RootCauseConfig {
                fee_complaint_correlation_window: 7,
                product_change_correlation_window: 14,
                life_event_correlation_window: 30,
                attribution_confidence_threshold: 0.70,
            },
            resolution_effectiveness: ResolutionEffectivenessConfig {
                satisfaction_impact_weights: HashMap::new(),
                churn_impact_weights: HashMap::new(),
                effectiveness_measurement_window: 90,
            },
            sla_performance: SLAPerformanceConfig {
                aging_buckets: vec![],
                breach_prediction_thresholds: HashMap::new(),
                critical_aging_threshold: 0.90,
            },
            early_warning_indicators: EarlyWarningConfig {
                velocity_comparison_periods: vec![7, 30, 90],
                breach_rate_warning_threshold: 0.15,
                repeat_complainer_threshold: 3,
                segment_concentration_warning: 0.60,
                issue_type_concentration_warning: 0.50,
            },
            cost_analysis: ComplaintCostConfig {
                average_handle_time_minutes: [("standard".into(), 35)].into(),
                loaded_hourly_rate: 45.0,
                escalation_cost_multiplier: 2.5,
                legal_review_cost: 350.0,
                write_off_authorization_cost: 125.0,
            },
        };

        let risk_appetite = RiskAppetiteConfig {
            dials: vec![
                DialConfig {
                    dial_id: "fee_aggressiveness".into(),
                    label: "Fee Aggressiveness".into(),
                    description: "Test dial".into(),
                    min_value: 0.0,
                    max_value: 2.0,
                    default_value: 1.0,
                    comfort_zone_min: 0.7,
                    comfort_zone_max: 1.3,
                    step_size: 0.1,
                    impacts: [("overdraft_fee_multiplier".into(), 1.0)].into(),
                },
                DialConfig {
                    dial_id: "growth_velocity".into(),
                    label: "Growth Velocity".into(),
                    description: "Test dial".into(),
                    min_value: 0.0,
                    max_value: 2.0,
                    default_value: 1.0,
                    comfort_zone_min: 0.8,
                    comfort_zone_max: 1.5,
                    step_size: 0.1,
                    impacts: HashMap::new(),
                },
                DialConfig {
                    dial_id: "service_level".into(),
                    label: "Service Level".into(),
                    description: "Test dial".into(),
                    min_value: 0.5,
                    max_value: 2.0,
                    default_value: 1.0,
                    comfort_zone_min: 0.9,
                    comfort_zone_max: 1.4,
                    step_size: 0.1,
                    impacts: HashMap::new(),
                },
                DialConfig {
                    dial_id: "retention_spend".into(),
                    label: "Retention Spend".into(),
                    description: "Test dial".into(),
                    min_value: 0.0,
                    max_value: 2.0,
                    default_value: 1.0,
                    comfort_zone_min: 0.7,
                    comfort_zone_max: 1.5,
                    step_size: 0.1,
                    impacts: HashMap::new(),
                },
                DialConfig {
                    dial_id: "compliance_stringency".into(),
                    label: "Compliance Stringency".into(),
                    description: "Test dial".into(),
                    min_value: 0.5,
                    max_value: 2.0,
                    default_value: 1.0,
                    comfort_zone_min: 0.9,
                    comfort_zone_max: 1.3,
                    step_size: 0.1,
                    impacts: HashMap::new(),
                },
            ],
            constraints: vec![
                DialConstraint {
                    constraint_id: "compliance_floor".into(),
                    description: "Cannot drop compliance below minimum".into(),
                    condition: "compliance_stringency < 0.6".into(),
                    violation_message: "Regulatory minimum compliance level is 0.6".into(),
                    enforcement: "hard_block".into(),
                },
                DialConstraint {
                    constraint_id: "fee_service_balance".into(),
                    description: "High fees require service".into(),
                    condition: "fee_aggressiveness > 1.4 AND service_level < 1.0".into(),
                    violation_message:
                        "Aggressive fees generate complaints that require strong service".into(),
                    enforcement: "warning".into(),
                },
            ],
            risk_profile_scoring: RiskProfileScoring {
                dimensions: vec![],
                risk_levels: HashMap::new(),
            },
            board_pressure: BoardPressureConfig {
                comfort_zone_violation_threshold: 2,
                pressure_messages: [
                    (
                        "fee_aggressiveness_high".into(),
                        "Board concerned about aggressive fees".into(),
                    ),
                    (
                        "service_level_low".into(),
                        "Board concerned about rising complaint rates".into(),
                    ),
                ]
                .into(),
            },
        };

        let payment_hub = PaymentHubConfig {
            rails: vec![
                PaymentRailConfig {
                    rail_id: "ACH".into(),
                    rail_type: "ACH".into(),
                    latency_type: "batch".into(),
                    settlement_delay_ticks: 1,
                    fraud_risk_multiplier: 0.5,
                    operational_risk_base: 0.001,
                    batch_window_ticks: Some(4),
                    cutoff_time_tick: None,
                },
                PaymentRailConfig {
                    rail_id: "wire".into(),
                    rail_type: "wire".into(),
                    latency_type: "real_time".into(),
                    settlement_delay_ticks: 0,
                    fraud_risk_multiplier: 1.5,
                    operational_risk_base: 0.002,
                    batch_window_ticks: None,
                    cutoff_time_tick: Some(18),
                },
                PaymentRailConfig {
                    rail_id: "RTP".into(),
                    rail_type: "RTP".into(),
                    latency_type: "real_time".into(),
                    settlement_delay_ticks: 0,
                    fraud_risk_multiplier: 2.0,
                    operational_risk_base: 0.0015,
                    batch_window_ticks: None,
                    cutoff_time_tick: None,
                },
                PaymentRailConfig {
                    rail_id: "card".into(),
                    rail_type: "card".into(),
                    latency_type: "batch".into(),
                    settlement_delay_ticks: 1,
                    fraud_risk_multiplier: 1.2,
                    operational_risk_base: 0.0012,
                    batch_window_ticks: Some(1),
                    cutoff_time_tick: None,
                },
            ],
            interchange_fee_rate: 0.025,
            auth_expiry_ticks: 7,
        };

        Self {
            segments: [
                ("mass_market".into(), seg),
                ("small_business".into(), seg_biz),
                ("premium".into(), seg_premium),
            ].into(),
            initial_population: 50,
            complaint_triggers: triggers,
            resolution_codes,
            products,
            fee_constraints,
            impact_formulas,
            offers,
            churn_model,
            segment_economics,
            complaint_analytics,
            risk_appetite,
            payment_hub,
            reconciliation: ReconciliationConfig {
                queue_configs: vec![
                    ReconQueueConfig {
                        rail_id: "ACH".into(),
                        tolerance_amount: 0.01,
                        auto_clear_threshold: 1.00,
                        sla_days: 3,
                        escalation_threshold: 100.00,
                        escalation_age_days: 7,
                    },
                    ReconQueueConfig {
                        rail_id: "wire".into(),
                        tolerance_amount: 0.01,
                        auto_clear_threshold: 0.50,
                        sla_days: 1,
                        escalation_threshold: 1000.00,
                        escalation_age_days: 3,
                    },
                    ReconQueueConfig {
                        rail_id: "RTP".into(),
                        tolerance_amount: 0.01,
                        auto_clear_threshold: 0.50,
                        sla_days: 1,
                        escalation_threshold: 500.00,
                        escalation_age_days: 3,
                    },
                    ReconQueueConfig {
                        rail_id: "card".into(),
                        tolerance_amount: 1.00,
                        auto_clear_threshold: 5.00,
                        sla_days: 3,
                        escalation_threshold: 100.00,
                        escalation_age_days: 7,
                    },
                ],
                enable_auto_clear: true,
                metrics_frequency_ticks: 7,
            },
            identity_address: IdentityAddressConfig {
                regions: vec![RegionPool {
                    region_id: "test_nyc".into(),
                    city: "New York".into(),
                    state: "NY".into(),
                    zip_prefix: "100".into(),
                    ssn_area_range: (50, 134),
                    weight: 1.0,
                    area_codes: vec!["212".into(), "718".into()],
                }],
                synthetic_identity_rate: 0.02,
                homeless_rate: 0.015,
                po_box_rate: 0.03,
                cmra_rate: 0.01,
                address_sharing_alert_threshold: 5,
                voip_rate: 0.05,
                international_customer_rate: 0.008,
            },
            incident: IncidentConfig {
                enabled: true,
                severity_weights: vec![
                    ("P0".into(), 0.05),
                    ("P1".into(), 0.15),
                    ("P2".into(), 0.30),
                    ("P3".into(), 0.50),
                ],
                sla_deadlines: vec![
                    ("P0".into(), 0),
                    ("P1".into(), 0),
                    ("P2".into(), 1),
                    ("P3".into(), 3),
                ],
                cascading_failures_enabled: true,
                metrics_interval_ticks: 7,
            },
            regulatory_exam: RegulatoryExamConfig {
                enabled: false, // disabled by default in tests (opt-in)
                exam_interval_ticks: 90,
                exam_duration_ticks: 14,
                examiners: vec!["OCC".into(), "CFPB".into(), "FDIC".into()],
                fine_minor: 5_000.0,
                fine_moderate: 50_000.0,
                fine_major: 250_000.0,
                fine_critical: 1_000_000.0,
                mou_critical_threshold: 2,
            },
            reputation: ReputationConfig {
                enabled: false, // disabled by default in tests (opt-in)
                initial_score: 75.0,
                recovery_per_tick: 0.05,
                fine_impact_per_1k: 0.1,
                mou_impact: 10.0,
                sar_late_impact: 2.0,
                sla_breach_impact: 0.5,
                onboarding_penalty_threshold: 40.0,
                max_onboarding_penalty: 0.50,
            },
        }
    }
}
