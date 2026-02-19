//! Operations Specialist role stub.
//!
//! Phase 3.8 will promote this to a full SimSubsystem that processes
//! the reconciliation exception queue. For now, this is a passive data
//! structure used by the ReconciliationSubsystem to model human capacity.

use serde::{Deserialize, Serialize};

/// An Operations Specialist who reviews and resolves reconciliation exceptions.
/// - `capacity_per_day`: Maximum exceptions this specialist can process per tick.
/// - `skill_level`: Affects resolution speed and write-off rate (0.0–1.0).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpsSpecialist {
    pub employee_id: String,
    pub name: String,
    pub capacity_per_day: u32,
    pub skill_level: f64, // 0.0 = novice, 1.0 = expert
}

impl OpsSpecialist {
    pub fn new(employee_id: String, name: String, capacity_per_day: u32, skill_level: f64) -> Self {
        Self {
            employee_id,
            name,
            capacity_per_day,
            skill_level: skill_level.clamp(0.0, 1.0),
        }
    }

    /// Effective daily throughput accounting for skill-based efficiency.
    /// A skill_level of 1.0 = full capacity, 0.5 = half capacity.
    pub fn effective_capacity(&self) -> u32 {
        ((self.capacity_per_day as f64) * (0.5 + self.skill_level * 0.5)) as u32
    }
}
