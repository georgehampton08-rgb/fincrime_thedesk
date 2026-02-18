//! Deterministic random number generation.
//!
//! RULE: Nothing in the simulation may call any platform RNG.
//! All randomness flows through SubsystemRng instances derived
//! from the single master seed stored on the Run record.
//!
//! Each subsystem gets its own RNG stream, seeded deterministically
//! from (master_seed XOR subsystem_index). This means:
//!   - Adding a new subsystem never changes existing subsystems' streams.
//!   - Each subsystem's stream is fully reproducible in isolation.

use rand::SeedableRng;
use rand_pcg::Pcg64Mcg;

/// A named, deterministic RNG for a single subsystem.
pub struct SubsystemRng {
    pub name: &'static str,
    inner: Pcg64Mcg,
}

impl SubsystemRng {
    /// Create a subsystem RNG from the master seed and a stable
    /// subsystem index. The index must never change once assigned.
    pub fn new(master_seed: u64, subsystem_index: u64) -> Self {
        let derived_seed = master_seed ^ (subsystem_index.wrapping_mul(0x9e37_79b9_7f4a_7c15));
        Self {
            name: "unnamed",
            inner: Pcg64Mcg::seed_from_u64(derived_seed),
        }
    }

    pub fn with_name(mut self, name: &'static str) -> Self {
        self.name = name;
        self
    }

    /// Roll a float in [0.0, 1.0).
    pub fn next_f64(&mut self) -> f64 {
        use rand::RngCore;
        let bits = self.inner.next_u64();
        (bits >> 11) as f64 * (1.0 / (1u64 << 53) as f64)
    }

    /// Draw a raw u64 (full range).
    pub fn next_u64(&mut self) -> u64 {
        use rand::RngCore;
        self.inner.next_u64()
    }

    /// Roll a u64 in [0, n).
    pub fn next_u64_below(&mut self, n: u64) -> u64 {
        use rand::RngCore;
        assert!(n > 0, "n must be > 0");
        self.inner.next_u64() % n
    }

    /// Bernoulli trial: returns true with probability p.
    pub fn chance(&mut self, p: f64) -> bool {
        self.next_f64() < p
    }

    /// Sample from a simplified Pareto distribution.
    /// x_min: minimum value, alpha: shape parameter (higher = less skewed).
    pub fn pareto(&mut self, x_min: f64, alpha: f64) -> f64 {
        let u = self.next_f64().max(1e-10);
        x_min * u.powf(-1.0 / alpha)
    }
}

/// All subsystem RNGs for a single run, indexed by stable slot.
pub struct RngBank {
    master_seed: u64,
}

impl RngBank {
    pub fn new(master_seed: u64) -> Self {
        Self { master_seed }
    }

    pub fn for_subsystem(&self, slot: SubsystemSlot) -> SubsystemRng {
        SubsystemRng::new(self.master_seed, slot as u64).with_name(slot.name())
    }
}

/// Stable subsystem slot assignments.
/// NEVER reorder or remove entries — only append.
/// Reordering changes every subsystem's seed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u64)]
pub enum SubsystemSlot {
    Macro = 0,
    Customer = 1,
    Account = 2,
    Transaction = 3,
    Complaint = 4,
    Economics = 5,
    Fraud = 6,
    Regulatory = 7,
    Pricing = 8,
    Offer = 9,
    Churn = 10,
    ComplaintAnalytics = 11, // Phase 2.5
    RiskAppetite = 12,       // Phase 2.6
    PaymentHub = 13,         // Phase 3.1
                             // Add new subsystems here — append only.
}

impl SubsystemSlot {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Macro => "macro",
            Self::Customer => "customer",
            Self::Account => "account",
            Self::Transaction => "transaction",
            Self::Complaint => "complaint",
            Self::Economics => "economics",
            Self::Fraud => "fraud",
            Self::Regulatory => "regulatory",
            Self::Pricing => "pricing",
            Self::Offer => "offer",
            Self::Churn => "churn",
            Self::ComplaintAnalytics => "complaint_analytics",
            Self::RiskAppetite => "risk_appetite",
            Self::PaymentHub => "payment_hub",
        }
    }
}
