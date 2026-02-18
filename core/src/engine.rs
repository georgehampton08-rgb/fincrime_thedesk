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
    config::ResolutionCode,
    error::SimResult,
    event::{EventLogEntry, SimEvent},
    macro_subsystem::MacroSubsystem,
    rng::{RngBank, SubsystemSlot},
    snapshot::{SimSnapshot, SNAPSHOT_INTERVAL},
    store::SimStore,
    subsystem::SimSubsystem,
    types::{RunId, Tick},
};
use std::collections::HashMap;

pub struct SimEngine {
    pub run_id:          RunId,
    pub clock:           SimClock,
    pub rng_bank:        RngBank,
    seed:                u64,
    subsystems:          Vec<(SubsystemSlot, Box<dyn SimSubsystem>)>,
    pub store:           SimStore,
    resolution_codes:    HashMap<String, ResolutionCode>,
    pending_commands:    Vec<SimEvent>,
}

impl SimEngine {
    pub fn new(run_id: RunId, seed: u64, store: SimStore) -> Self {
        Self {
            clock:            SimClock::new(run_id.clone()),
            rng_bank:         RngBank::new(seed),
            seed,
            subsystems:       Vec::new(),
            store,
            run_id,
            resolution_codes: HashMap::new(),
            pending_commands: Vec::new(),
        }
    }

    /// Build a fully wired engine with all subsystems registered.
    /// Call this instead of new() + manual register() calls.
    pub fn build(
        run_id:    RunId,
        seed:      u64,
        store:     &SimStore,
        data_dir:  &str,
    ) -> anyhow::Result<Self> {
        let config = crate::config::SimConfig::load(data_dir)?;

        // Each subsystem needs its own store connection for concurrent access
        let store_customer   = store.reopen()?;
        let store_txn        = store.reopen()?;
        let store_complaint  = store.reopen()?;
        let store_economics  = store.reopen()?;
        let store_pricing    = store.reopen()?;

        let mut engine = SimEngine::new(run_id.clone(), seed, store.reopen()?);
        engine.resolution_codes = config.resolution_codes.clone();

        // EXECUTION ORDER — fixed, documented, never reordered.
        // Phase 0: engine internals (no subsystem)
        // Phase 1A:
        engine.register(
            SubsystemSlot::Macro,
            Box::new(MacroSubsystem::new()),
        );
        // Phase 1B:
        engine.register(
            SubsystemSlot::Customer,
            Box::new(crate::customer_subsystem::CustomerSubsystem::new(
                run_id.clone(),
                config.clone(),
                store_customer,
            )),
        );
        engine.register(
            SubsystemSlot::Transaction,
            Box::new(crate::transaction_subsystem::TransactionSubsystem::new(
                run_id.clone(),
                store_txn,
            )),
        );
        // Phase 1C:
        engine.register(
            SubsystemSlot::Complaint,
            Box::new(crate::complaint_subsystem::ComplaintSubsystem::new(
                run_id.clone(),
                config.clone(),
                store_complaint,
            )),
        );
        // Phase 2.1: Pricing (runs before Economics so fee changes affect same tick's P&L)
        engine.register(
            SubsystemSlot::Pricing,
            Box::new(crate::pricing_subsystem::PricingSubsystem::new(
                run_id.clone(),
                config.clone(),
                store_pricing,
            )),
        );
        // Phase 1D:
        engine.register(
            SubsystemSlot::Economics,
            Box::new(crate::economics_subsystem::EconomicsSubsystem::new(
                run_id.clone(),
                config.clone(),
                store_economics,
            )),
        );
        // Phase 3:  Fraud, Regulatory (stubs)
        Ok(engine)
    }

    /// Test-only build using in-memory config.
    pub fn build_test(run_id: RunId, seed: u64) -> SimResult<Self> {
        // Use a temp file so reopen() works (in-memory doesn't share across connections)
        let temp_path = format!("./test_{}.db", uuid::Uuid::new_v4());
        let store = SimStore::open(&temp_path)?;
        store.migrate()?;
        store.insert_run(&run_id, seed, "0.1.0-test")?;

        let config = crate::config::SimConfig::default_test();
        let store_customer  = store.reopen()?;
        let store_txn       = store.reopen()?;
        let store_complaint = store.reopen()?;
        let store_economics = store.reopen()?;
        let store_pricing   = store.reopen()?;

        let mut engine = SimEngine::new(run_id.clone(), seed, store.reopen()?);
        engine.resolution_codes = config.resolution_codes.clone();

        engine.register(
            SubsystemSlot::Macro,
            Box::new(MacroSubsystem::new()),
        );
        engine.register(
            SubsystemSlot::Customer,
            Box::new(crate::customer_subsystem::CustomerSubsystem::new(
                run_id.clone(),
                config.clone(),
                store_customer,
            )),
        );
        engine.register(
            SubsystemSlot::Transaction,
            Box::new(crate::transaction_subsystem::TransactionSubsystem::new(
                run_id.clone(),
                store_txn,
            )),
        );
        engine.register(
            SubsystemSlot::Complaint,
            Box::new(crate::complaint_subsystem::ComplaintSubsystem::new(
                run_id.clone(),
                config.clone(),
                store_complaint,
            )),
        );
        // Phase 2.1: Pricing (runs before Economics)
        engine.register(
            SubsystemSlot::Pricing,
            Box::new(crate::pricing_subsystem::PricingSubsystem::new(
                run_id.clone(),
                config.clone(),
                store_pricing,
            )),
        );
        engine.register(
            SubsystemSlot::Economics,
            Box::new(crate::economics_subsystem::EconomicsSubsystem::new(
                run_id,
                config,
                store_economics,
            )),
        );
        Ok(engine)
    }

    /// Register a subsystem. Call in the documented execution order.
    pub fn register(&mut self, slot: SubsystemSlot, subsystem: Box<dyn SimSubsystem>) {
        self.subsystems.push((slot, subsystem));
    }

    /// Submit a player command to be processed on the next tick.
    pub fn submit_command(&mut self, cmd: crate::command::PlayerCommand) -> SimResult<()> {
        let command_id = self.store.store_player_command(
            &self.run_id,
            self.clock.current_tick,
            &cmd,
        )?;

        let cmd_type = match &cmd {
            crate::command::PlayerCommand::Pause           => "pause",
            crate::command::PlayerCommand::Resume          => "resume",
            crate::command::PlayerCommand::SetSpeed { .. } => "set_speed",
            crate::command::PlayerCommand::CloseComplaint { .. } => "close_complaint",
            crate::command::PlayerCommand::SetProductFee { .. }  => "set_product_fee",
        };

        self.pending_commands.push(SimEvent::PlayerCommandReceived {
            tick:         self.clock.current_tick,
            command_id:   command_id.to_string(),
            command_type: cmd_type.to_string(),
        });

        Ok(())
    }

    /// Advance one tick. This is the core simulation step.
    pub fn tick(&mut self) -> SimResult<Vec<SimEvent>> {
        assert!(!self.clock.paused, "tick() called on paused engine");

        let current_tick = self.clock.advance();
        let mut tick_events: Vec<SimEvent> = vec![
            SimEvent::TickStarted { tick: current_tick }
        ];

        // Inject any pending player commands into this tick's event stream
        if !self.pending_commands.is_empty() {
            tick_events.extend(self.pending_commands.drain(..));
        }

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

    // ── Complaint wrapper methods (for tests and sim-runner) ──────────────────

    pub fn store_complaint_count(&self, run_id: &str) -> SimResult<i64> {
        self.store.complaint_count(run_id)
    }

    pub fn store_sla_breach_count(&self, run_id: &str) -> SimResult<i64> {
        self.store.sla_breach_count(run_id)
    }

    pub fn store_first_open_complaint(
        &self,
        run_id: &str,
    ) -> SimResult<Option<crate::complaint_subsystem::ComplaintRecord>> {
        self.store.first_open_complaint(run_id)
    }

    pub fn store_customer_satisfaction(&self, run_id: &str, customer_id: &str) -> SimResult<f64> {
        self.store.customer_satisfaction(run_id, customer_id)
    }

    /// Close a complaint and apply the resolution's satisfaction delta directly.
    /// Used by tests and the future UI layer for player-initiated resolutions.
    pub fn store_close_complaint_direct(
        &self,
        run_id: &str,
        complaint_id: &str,
        tick: Tick,
        resolution_code: &str,
        amount_refunded: f64,
    ) -> SimResult<()> {
        let complaint = self.store.get_complaint(run_id, complaint_id)?;
        self.store.close_complaint(run_id, complaint_id, tick, resolution_code, amount_refunded)?;
        if let Some(rc) = self.resolution_codes.get(resolution_code) {
            self.store.update_customer_satisfaction(run_id, &complaint.customer_id, rc.satisfaction_delta)?;
            self.store.adjust_customer_churn_risk(run_id, &complaint.customer_id, rc.churn_risk_delta)?;
        }
        Ok(())
    }

    pub fn store_complaint_backlog(&self, run_id: &str) -> SimResult<i64> {
        self.store.complaint_backlog(run_id)
    }

    pub fn store_fee_event_count(&self, run_id: &str) -> SimResult<i64> {
        self.store.fee_event_count(run_id)
    }

    pub fn store_churned_count(&self, run_id: &str) -> SimResult<i64> {
        self.store.churned_customer_count(run_id)
    }

    // ── Economics wrapper methods (for tests and sim-runner) ──────────────────

    pub fn store_pnl_count(&self, run_id: &str) -> SimResult<i64> {
        self.store.pnl_count(run_id)
    }

    pub fn store_latest_pnl(
        &self,
        run_id: &str,
    ) -> SimResult<Option<crate::economics_subsystem::PnLSnapshot>> {
        let snaps = self.store.latest_pnl_snapshots(run_id, 1)?;
        Ok(snaps.into_iter().next())
    }

    pub fn store_all_pnl_snapshots(
        &self,
        run_id: &str,
    ) -> SimResult<Vec<crate::economics_subsystem::PnLSnapshot>> {
        self.store.all_pnl_snapshots(run_id)
    }

    // ── Pricing wrapper methods (for tests and sim-runner) ──────────────────

    pub fn store_product_state(
        &self,
        run_id: &str,
        product_id: &str,
    ) -> SimResult<crate::pricing_subsystem::ProductState> {
        self.store.get_product_state(run_id, product_id)
    }

    pub fn store_udaap_score(&self, run_id: &str) -> SimResult<f64> {
        self.store.get_udaap_score(run_id)
    }

    pub fn store_fee_change_history(
        &self,
        run_id: &str,
        product_id: &str,
        limit: usize,
    ) -> SimResult<Vec<crate::store::FeeChangeRecord>> {
        self.store.fee_change_history(run_id, product_id, limit)
    }
}

/// Extract a stable string name from a SimEvent variant.
/// Used for the event_type column in event_log.
fn event_type_name(event: &SimEvent) -> &'static str {
    match event {
        SimEvent::TickStarted { .. }           => "tick_started",
        SimEvent::TickCompleted { .. }         => "tick_completed",
        SimEvent::RunInitialized { .. }        => "run_initialized",
        SimEvent::MacroStateUpdated { .. }     => "macro_state_updated",
        SimEvent::PlayerCommandReceived { .. } => "player_command_received",
        SimEvent::CustomerOnboarded { .. }     => "customer_onboarded",
        SimEvent::CustomerChurned { .. }       => "customer_churned",
        SimEvent::FeeCharged { .. }            => "fee_charged",
        SimEvent::ComplaintFiled { .. }        => "complaint_filed",
        SimEvent::ComplaintResolved { .. }     => "complaint_resolved",
        SimEvent::SLABreached { .. }           => "sla_breached",
        SimEvent::QuarterlyPnLComputed { .. }  => "quarterly_pnl_computed",
        SimEvent::ProductFeeChanged { .. }     => "product_fee_changed",
        SimEvent::FeeChangeRejected { .. }     => "fee_change_rejected",
    }
}
