use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductConfig {
    pub product_id:        String,
    pub product_type:      String,
    pub tier:              String,
    pub label:             String,
    pub monthly_fee:       f64,
    pub overdraft_fee:     f64,
    pub nsf_fee:           f64,
    pub atm_fee:           f64,
    pub wire_fee:          f64,
    pub interest_rate:     f64,
    pub min_balance_waive: Option<f64>,
    pub min_dd_waive:      Option<f64>,
    pub target_segment:    String,
}

#[derive(Debug, Clone, Deserialize)]
struct ProductCatalogFile {
    products: Vec<ProductConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SegmentConfig {
    pub id:                         String,
    pub label:                      String,
    pub population_share:           f64,
    pub income_bands:               Vec<String>,
    pub income_band_weights:        Vec<f64>,
    pub monthly_txn_count_mean:     f64,
    pub monthly_txn_count_std:      f64,
    pub txn_amount_pareto_xmin:     f64,
    pub txn_amount_pareto_alpha:    f64,
    pub cash_intensity:             f64,
    pub payroll_probability:        f64,
    pub payroll_amount_mean:        f64,
    pub payroll_amount_std:         f64,
    pub overdraft_probability_per_tick: f64,
    pub nsf_probability_per_tick:   f64,
    pub base_churn_rate_per_tick:   f64,
    pub fee_sensitivity:            f64,
    pub products:                   Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplaintTrigger {
    pub event_type:           String,
    #[serde(default)]
    pub fee_type:             Option<String>,
    #[serde(default)]
    pub amount_threshold:     Option<f64>,
    #[serde(default)]
    pub prior_breach:         bool,
    pub probability:          f64,
    pub issue_category:       String,
    pub priority:             String,
    pub sla_acknowledge_days: u64,
    pub sla_resolve_days:     u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolutionCode {
    pub code:                String,
    pub satisfaction_delta:  f64,
    pub churn_risk_delta:    f64,
    pub avg_amount_refunded: f64,
}

#[derive(Debug, Clone, Deserialize)]
struct SegmentsFile {
    segments: Vec<SegmentConfig>,
}

#[derive(Debug, Clone, Deserialize)]
struct ComplaintConfigFile {
    triggers:         Vec<ComplaintTrigger>,
    resolution_codes: Vec<ResolutionCode>,
}

#[derive(Debug, Clone)]
pub struct SimConfig {
    pub segments:            HashMap<String, SegmentConfig>,
    pub initial_population:  usize,
    pub complaint_triggers:  Vec<ComplaintTrigger>,
    pub resolution_codes:    HashMap<String, ResolutionCode>,
    pub products:            HashMap<String, ProductConfig>,
}

impl SimConfig {
    /// Load from the data/ directory.
    /// In tests, use SimConfig::default_test().
    pub fn load(data_dir: &str) -> anyhow::Result<Self> {
        let path = format!("{data_dir}/segments/segments.json");
        let content = std::fs::read_to_string(&path)
            .map_err(|e| anyhow::anyhow!("Cannot read {path}: {e}"))?;
        let file: SegmentsFile = serde_json::from_str(&content)?;
        let segments = file.segments.into_iter()
            .map(|s| (s.id.clone(), s))
            .collect();

        let complaint_path = format!("{data_dir}/complaints/complaint_triggers.json");
        let complaint_content = std::fs::read_to_string(&complaint_path)
            .map_err(|e| anyhow::anyhow!("Cannot read {complaint_path}: {e}"))?;
        let complaint_file: ComplaintConfigFile = serde_json::from_str(&complaint_content)?;
        let resolution_codes = complaint_file.resolution_codes.into_iter()
            .map(|r| (r.code.clone(), r))
            .collect();

        let product_path = format!("{data_dir}/products/product_catalog.json");
        let product_content = std::fs::read_to_string(&product_path)
            .map_err(|e| anyhow::anyhow!("Cannot read {product_path}: {e}"))?;
        let product_file: ProductCatalogFile = serde_json::from_str(&product_content)?;
        let products = product_file.products.into_iter()
            .map(|p| (p.product_id.clone(), p))
            .collect();

        Ok(Self {
            segments,
            initial_population: 500,
            complaint_triggers: complaint_file.triggers,
            resolution_codes,
            products,
        })
    }

    /// Config with hardcoded defaults for use in unit tests.
    pub fn default_test() -> Self {
        let seg = SegmentConfig {
            id:                          "mass_market".into(),
            label:                       "Mass Market".into(),
            population_share:            1.0,
            income_bands:                vec!["low".into()],
            income_band_weights:         vec![1.0],
            monthly_txn_count_mean:      20.0,
            monthly_txn_count_std:       4.0,
            txn_amount_pareto_xmin:      15.0,
            txn_amount_pareto_alpha:     1.8,
            cash_intensity:              0.35,
            payroll_probability:         0.5,
            payroll_amount_mean:         2000.0,
            payroll_amount_std:          400.0,
            overdraft_probability_per_tick: 0.005,
            nsf_probability_per_tick:    0.002,
            base_churn_rate_per_tick:    0.001,
            fee_sensitivity:             0.8,
            products:                    vec!["basic_checking".into()],
        };

        let triggers = vec![
            ComplaintTrigger {
                event_type:           "fee_charged".into(),
                fee_type:             Some("overdraft".into()),
                amount_threshold:     None,
                prior_breach:         false,
                probability:          0.12,
                issue_category:       "fee_dispute".into(),
                priority:             "standard".into(),
                sla_acknowledge_days: 2,
                sla_resolve_days:     15,
            },
        ];

        let resolution_codes = [
            (
                "explanation_only".into(),
                ResolutionCode {
                    code:                "explanation_only".into(),
                    satisfaction_delta:  -0.02,
                    churn_risk_delta:    0.03,
                    avg_amount_refunded: 0.0,
                },
            ),
            (
                "monetary_relief".into(),
                ResolutionCode {
                    code:                "monetary_relief".into(),
                    satisfaction_delta:  0.15,
                    churn_risk_delta:    -0.10,
                    avg_amount_refunded: 27.08,
                },
            ),
        ].into();

        let products = [(
            "basic_checking".into(),
            ProductConfig {
                product_id:        "basic_checking".into(),
                product_type:      "checking".into(),
                tier:              "basic".into(),
                label:             "Basic Checking".into(),
                monthly_fee:       0.0,
                overdraft_fee:     27.08,
                nsf_fee:           17.72,
                atm_fee:           2.50,
                wire_fee:          25.0,
                interest_rate:     0.0,
                min_balance_waive: None,
                min_dd_waive:      None,
                target_segment:    "mass_market".into(),
            },
        )].into();

        Self {
            segments: [("mass_market".into(), seg)].into(),
            initial_population: 50,
            complaint_triggers: triggers,
            resolution_codes,
            products,
        }
    }
}
