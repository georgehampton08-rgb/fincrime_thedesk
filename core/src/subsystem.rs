//! Subsystem trait and registry.
//!
//! RULE: Every subsystem implements SimSubsystem.
//! The engine calls update() on each registered subsystem
//! in registration order, every tick.
//! Execution order is fixed and documented in engine.rs.

use crate::{
    error::SimResult,
    event::SimEvent,
    rng::SubsystemRng,
    types::Tick,
};
use std::any::Any;

/// The contract every subsystem must fulfill.
pub trait SimSubsystem: Send {
    /// Unique stable name for this subsystem.
    fn name(&self) -> &'static str;

    /// Called once per tick by the engine.
    ///
    /// - `tick`:      the current tick number
    /// - `events_in`: events emitted by earlier subsystems this tick
    /// - `rng`:       this subsystem's deterministic RNG for this tick
    ///
    /// Returns a vec of new events to add to the tick's event log.
    fn update(
        &mut self,
        tick: Tick,
        events_in: &[SimEvent],
        rng: &mut SubsystemRng,
    ) -> SimResult<Vec<SimEvent>>;

    /// For downcasting in tests and tooling only.
    /// Production sim code never uses this.
    fn as_any(&self) -> &dyn Any;
}
