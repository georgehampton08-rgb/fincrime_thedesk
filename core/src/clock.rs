//! Simulation clock — owns tick state, speed control, and pause.

use crate::types::{RunId, Tick};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SimClock {
    pub run_id:       RunId,
    pub current_tick: Tick,
    pub speed:        SimSpeed,
    pub paused:       bool,
}

impl SimClock {
    pub fn new(run_id: RunId) -> Self {
        Self {
            run_id,
            current_tick: 0,
            speed: SimSpeed::Normal,
            paused: true,
        }
    }

    /// Advance one tick. Returns the new tick number.
    /// Panics if called while paused — callers must check.
    pub fn advance(&mut self) -> Tick {
        assert!(!self.paused, "advance() called on paused clock");
        self.current_tick += 1;
        self.current_tick
    }

    pub fn pause(&mut self)  { self.paused = true;  }
    pub fn resume(&mut self) { self.paused = false; }

    pub fn set_speed(&mut self, speed: SimSpeed) {
        self.speed = speed;
    }

    pub fn ticks_per_real_second(&self) -> u32 {
        match self.speed {
            SimSpeed::Normal      => 1,
            SimSpeed::Accelerated => 7,
            SimSpeed::FastForward => 30,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SimSpeed {
    Normal,       // 1 tick/step  (1 day per ~3 real seconds)
    Accelerated,  // 7 ticks/step (1 week per ~3 real seconds)
    FastForward,  // 30 ticks/step (1 month per ~3 real seconds)
}
