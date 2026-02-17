use crate::{
    error::SimResult,
    event::{EconomicPhase, SimEvent},
    rng::SubsystemRng,
    subsystem::SimSubsystem,
    types::Tick,
};
use serde::{Deserialize, Serialize};

pub const MACRO_UPDATE_INTERVAL: Tick = 90; // quarterly

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MacroState {
    pub base_rate:        f64,
    pub economic_phase:   EconomicPhase,
    pub fraud_multiplier: f64,
    /// Ticks remaining in current economic phase.
    phase_ticks_left:     Tick,
}

impl Default for MacroState {
    fn default() -> Self {
        Self {
            base_rate:        0.05,
            economic_phase:   EconomicPhase::Expansion,
            fraud_multiplier: 1.0,
            phase_ticks_left: 360, // 4 quarters to start
        }
    }
}

impl MacroState {
    fn advance_phase(&mut self, rng: &mut SubsystemRng) {
        self.economic_phase = match self.economic_phase {
            EconomicPhase::Expansion   => EconomicPhase::Peak,
            EconomicPhase::Peak        => EconomicPhase::Contraction,
            EconomicPhase::Contraction => EconomicPhase::Trough,
            EconomicPhase::Trough      => EconomicPhase::Expansion,
        };
        // Next phase lasts 4–8 quarters (360–720 ticks)
        let quarters = 4 + rng.next_u64_below(5); // 4..=8
        self.phase_ticks_left = quarters * 90;
        self.fraud_multiplier = self.economic_phase.fraud_multiplier();
    }

    fn adjust_rate(&mut self, rng: &mut SubsystemRng) {
        // Rate moves ±0.25% per quarter with slight phase bias
        let direction: f64 = match self.economic_phase {
            EconomicPhase::Expansion   =>  0.5, // more likely up
            EconomicPhase::Peak        =>  0.0, // neutral
            EconomicPhase::Contraction => -0.5, // more likely down
            EconomicPhase::Trough      => -0.5, // more likely down
        };
        let roll = rng.next_f64() - 0.5 + direction * 0.2;
        let delta = if roll > 0.0 { 0.0025 } else { -0.0025 };
        self.base_rate = (self.base_rate + delta).clamp(0.005, 0.12);
    }
}

pub struct MacroSubsystem {
    pub state: MacroState,
}

impl MacroSubsystem {
    pub fn new() -> Self {
        Self { state: MacroState::default() }
    }
}

impl Default for MacroSubsystem {
    fn default() -> Self { Self::new() }
}

impl SimSubsystem for MacroSubsystem {
    fn name(&self) -> &'static str { "macro" }

    fn update(
        &mut self,
        tick: Tick,
        _events_in: &[SimEvent],
        rng: &mut SubsystemRng,
    ) -> SimResult<Vec<SimEvent>> {
        // Only compute on quarterly boundaries.
        if !tick.is_multiple_of(MACRO_UPDATE_INTERVAL) {
            return Ok(vec![]);
        }

        // Decrement phase counter; advance phase if exhausted.
        self.state.phase_ticks_left =
            self.state.phase_ticks_left.saturating_sub(MACRO_UPDATE_INTERVAL);

        if self.state.phase_ticks_left == 0 {
            self.state.advance_phase(rng);
        } else {
            self.state.adjust_rate(rng);
        }

        log::debug!(
            "tick={tick} macro: phase={:?} rate={:.4} fraud_mult={:.2}",
            self.state.economic_phase,
            self.state.base_rate,
            self.state.fraud_multiplier
        );

        Ok(vec![SimEvent::MacroStateUpdated {
            tick,
            base_rate:        self.state.base_rate,
            economic_phase:   self.state.economic_phase.clone(),
            fraud_multiplier: self.state.fraud_multiplier,
        }])
    }

    fn as_any(&self) -> &dyn std::any::Any { self }
}
