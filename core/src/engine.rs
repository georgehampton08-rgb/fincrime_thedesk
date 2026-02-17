//! The simulation engine — the heart of FinCrime: The Desk.
//!
//! EXECUTION ORDER (fixed, documented, never reordered):
//!   1. Macro subsystem
//!   2. Customer subsystem     (Phase 1B)
//!   3. Account subsystem      (Phase 1B)
//!   4. Transaction subsystem  (Phase 1B)
//!   5. Complaint subsystem    (Phase 1C)
//!   6. Economics subsystem    (Phase 1D)
//!   7. Fraud subsystem        (Phase 3)
//!   8. Regulatory subsystem   (Phase 3)
//!
//! RULES:
//!   - Subsystems execute in registration order, every tick.
//!   - Each subsystem reads ONLY the prior tick's state.
//!   - No subsystem calls another subsystem's functions directly.
//!   - All randomness flows through the RngBank.
//!   - All state changes are recorded in the event log.

use crate::{
    clock::SimClock,
    error::SimResult,
    event::{EventLogEntry, SimEvent},
    macro_subsystem::MacroSubsystem,
    rng::{RngBank, SubsystemSlot},
    snapshot::{SimSnapshot, SNAPSHOT_INTERVAL},
    store::SimStore,
    subsystem::SimSubsystem,
    types::{RunId, Tick},
};

pub struct SimEngine {
    pub run_id:     RunId,
    pub clock:      SimClock,
    pub rng_bank:   RngBank,
    seed:           u64,
    subsystems:     Vec<(SubsystemSlot, Box<dyn SimSubsystem>)>,
    store:          SimStore,
}

impl SimEngine {
    pub fn new(run_id: RunId, seed: u64, store: SimStore) -> Self {
        Self {
            clock:      SimClock::new(run_id.clone()),
            rng_bank:   RngBank::new(seed),
            seed,
            subsystems: Vec::new(),
            store,
            run_id,
        }
    }

    /// Build a fully wired engine with all subsystems registered.
    /// Call this instead of new() + manual register() calls.
    pub fn build(run_id: RunId, seed: u64, store: SimStore) -> Self {
        let mut engine = SimEngine::new(run_id, seed, store);
        
        // EXECUTION ORDER — fixed, documented, never reordered.
        // Phase 0: engine internals (no subsystem)
        // Phase 1A:
        engine.register(
            SubsystemSlot::Macro,
            Box::new(MacroSubsystem::new()),
        );
        // Phase 1B: Customer, Account, Transaction (stubs for now)
        // Phase 1C: Complaint (stub)
        // Phase 1D: Economics (stub)
        // Phase 3:  Fraud, Regulatory (stubs)
        engine
    }

    /// Register a subsystem. Call in the documented execution order.
    pub fn register(&mut self, slot: SubsystemSlot, subsystem: Box<dyn SimSubsystem>) {
        self.subsystems.push((slot, subsystem));
    }

    /// Advance one tick. This is the core simulation step.
    pub fn tick(&mut self) -> SimResult<Vec<SimEvent>> {
        assert!(!self.clock.paused, "tick() called on paused engine");

        let current_tick = self.clock.advance();
        let mut tick_events: Vec<SimEvent> = vec![
            SimEvent::TickStarted { tick: current_tick }
        ];

        // Execute each subsystem in registration order.
        // Each subsystem sees all events emitted so far this tick.
        for (slot, subsystem) in &mut self.subsystems {
            let mut rng = self.rng_bank.for_subsystem(*slot);
            let new_events = subsystem.update(current_tick, &tick_events, &mut rng)?;

            // Persist each new event to the log.
            for event in &new_events {
                let entry = EventLogEntry {
                    id:         None,
                    run_id:     self.run_id.clone(),
                    tick:       current_tick,
                    subsystem:  subsystem.name().to_string(),
                    event_type: event_type_name(event).to_string(),
                    payload:    serde_json::to_string(event)?,
                };
                self.store.append_event(&entry)?;
            }

            tick_events.extend(new_events);
        }

        tick_events.push(SimEvent::TickCompleted { tick: current_tick });

        // Snapshot every SNAPSHOT_INTERVAL ticks.
        if current_tick.is_multiple_of(SNAPSHOT_INTERVAL) {
            self.take_snapshot(current_tick)?;
        }

        Ok(tick_events)
    }

    /// Run n ticks in a loop. Used for testing and fast-forward.
    pub fn run_ticks(&mut self, n: u64) -> SimResult<()> {
        // Emit RunInitialized at tick 0 so seed differences are observable.
        if self.clock.current_tick == 0 {
            let init_event = SimEvent::RunInitialized {
                run_id: self.run_id.clone(),
                seed: self.seed,
            };
            let entry = EventLogEntry {
                id:         None,
                run_id:     self.run_id.clone(),
                tick:       0,
                subsystem:  "engine".to_string(),
                event_type: event_type_name(&init_event).to_string(),
                payload:    serde_json::to_string(&init_event)?,
            };
            self.store.append_event(&entry)?;
        }
        self.clock.resume();
        for _ in 0..n {
            self.tick()?;
        }
        self.clock.pause();
        Ok(())
    }

    /// Query events for a specific tick from the store.
    /// Used by the determinism test and replay tooling.
    pub fn store_events_for_tick(
        &self,
        run_id: &str,
        tick: Tick,
    ) -> SimResult<Vec<EventLogEntry>> {
        self.store.events_for_tick(run_id, tick)
    }

    /// Query the MacroSubsystem's current state.
    /// Used by sim-runner to print end-of-run summaries.
    pub fn last_macro_state(&self) -> Option<&crate::macro_subsystem::MacroState> {
        use std::any::Any;
        self.subsystems.iter().find_map(|(_, sub)| {
            sub.as_any()
                .downcast_ref::<crate::macro_subsystem::MacroSubsystem>()
                .map(|m| &m.state)
        })
    }

    fn take_snapshot(&self, tick: Tick) -> SimResult<()> {
        let snapshot = SimSnapshot {
            run_id: self.run_id.clone(),
            tick,
            clock:  self.clock.clone(),
        };
        let json = serde_json::to_string(&snapshot)?;
        self.store.save_snapshot(&self.run_id, tick, &json)?;
        log::debug!("Snapshot saved at tick {tick}");
        Ok(())
    }
}

/// Extract a stable string name from a SimEvent variant.
/// Used for the event_type column in event_log.
fn event_type_name(event: &SimEvent) -> &'static str {
    match event {
        SimEvent::TickStarted { .. }          => "tick_started",
        SimEvent::TickCompleted { .. }        => "tick_completed",
        SimEvent::RunInitialized { .. }       => "run_initialized",
        SimEvent::MacroStateUpdated { .. }    => "macro_state_updated",
        SimEvent::PlayerCommandReceived { .. }=> "player_command_received",
    }
}
