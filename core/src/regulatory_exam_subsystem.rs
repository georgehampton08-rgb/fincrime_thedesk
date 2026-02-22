//! Regulatory Examination subsystem — Phase 3.6.
//!
//! Models periodic regulatory examinations (OCC, CFPB, FDIC, FRB).
//!
//! This subsystem:
//!   1. Opens an exam cycle every `exam_interval_ticks` ticks.
//!   2. During the exam window scans the event log for compliance
//!      evidence (SLA breaches, SAR late filings, AML critical alerts).
//!   3. At the end of the exam window issues findings, levies fines,
//!      and optionally issues an MOU (Memorandum of Understanding).
//!
//! Downstream: the `ReputationSubsystem` reads `MOUReceived` and
//!   `RegulatoryExamClosed` events from this tick's output.
//!
//! Execution: every tick, after TransactionMonitoring.
//! Depends on: none (pulls from DB).

use crate::{
    config::RegulatoryExamConfig,
    error::SimResult,
    event::SimEvent,
    rng::SubsystemRng,
    store::{regulatory_exam::RegulatoryExamRow, SimStore},
    subsystem::SimSubsystem,
    types::{RunId, Tick},
};

// ── Finding generation ───────────────────────────────────────────────────────

/// Categories of findings and how they are detected.
struct FindingSpec {
    category:    &'static str,
    severity:    &'static str,
    description: &'static str,
}

/// Derive findings by counting negative signal events in the exam window.
fn derive_findings(
    run_id:           &str,
    exam_id:          &str,
    store:            &SimStore,
    config:           &RegulatoryExamConfig,
    tick_start:       Tick,
    tick_end:         Tick,
    rng:              &mut SubsystemRng,
) -> SimResult<(Vec<ExamFinding>, f64, u32)> {
    // Count signals from the event log in the exam window.
    let sla_breaches: i64 = store.count_events_in_range(
        run_id, tick_start, tick_end, "sla_breached",
    ).unwrap_or(0);

    let incident_sla_breaches: i64 = store.count_events_in_range(
        run_id, tick_start, tick_end, "incident_s_l_a_breach",
    ).unwrap_or(0);

    let sar_late: i64 = store.count_events_in_range(
        run_id, tick_start, tick_end, "sar_late_filing",
    ).unwrap_or(0);

    let total_breaches = sla_breaches + incident_sla_breaches;

    let mut findings: Vec<ExamFinding> = Vec::new();
    let mut fine_total = 0.0f64;
    let mut critical_count = 0u32;

    // SAR timeliness findings
    if sar_late > 0 {
        let spec = match sar_late {
            1 => FindingSpec { category: "sar_timeliness", severity: "moderate",
                description: "Late SAR filing detected in exam window" },
            2..=4 => FindingSpec { category: "sar_timeliness", severity: "major",
                description: "Multiple late SAR filings detected" },
            _ => FindingSpec { category: "sar_timeliness", severity: "critical",
                description: "Systemic SAR filing failures — excessive late filings" },
        };
        let fine = fine_for_severity(spec.severity, config);
        let finding_id = format!("fnd-{}-sar-{}", exam_id, rng.next_u64() % 100000);
        findings.push(ExamFinding {
            finding_id,
            category: spec.category.into(),
            severity: spec.severity.into(),
            description: spec.description.into(),
            fine_amount: fine,
        });
        fine_total += fine;
        if spec.severity == "critical" { critical_count += 1; }
    }

    // Complaint SLA findings
    if total_breaches > 10 {
        let spec = if total_breaches > 50 {
            FindingSpec { category: "complaint_sla", severity: "major",
                description: "Persistent SLA breach pattern across exam window" }
        } else {
            FindingSpec { category: "complaint_sla", severity: "moderate",
                description: "Elevated complaint SLA breach rate in exam window" }
        };
        let fine = fine_for_severity(spec.severity, config);
        let finding_id = format!("fnd-{}-sla-{}", exam_id, rng.next_u64() % 100000);
        findings.push(ExamFinding {
            finding_id,
            category: spec.category.into(),
            severity: spec.severity.into(),
            description: spec.description.into(),
            fine_amount: fine,
        });
        fine_total += fine;
    } else if total_breaches > 0 {
        // Small number: minor finding
        let finding_id = format!("fnd-{}-sla-{}", exam_id, rng.next_u64() % 100000);
        let fine = config.fine_minor;
        findings.push(ExamFinding {
            finding_id,
            category: "complaint_sla".into(),
            severity: "minor".into(),
            description: "Minor complaint SLA deviations noted".into(),
            fine_amount: fine,
        });
        fine_total += fine;
    }

    // Probabilistic data-integrity finding (low base rate, slightly elevated if events exist)
    let data_integrity_prob = 0.10;
    if rng.chance(data_integrity_prob) {
        let finding_id = format!("fnd-{}-di-{}", exam_id, rng.next_u64() % 100000);
        let fine = config.fine_minor;
        findings.push(ExamFinding {
            finding_id,
            category: "data_integrity".into(),
            severity: "minor".into(),
            description: "Minor data integrity gaps identified".into(),
            fine_amount: fine,
        });
        fine_total += fine;
    }

    Ok((findings, fine_total, critical_count))
}

fn fine_for_severity(severity: &str, config: &RegulatoryExamConfig) -> f64 {
    match severity {
        "minor"    => config.fine_minor,
        "moderate" => config.fine_moderate,
        "major"    => config.fine_major,
        "critical" => config.fine_critical,
        _          => config.fine_minor,
    }
}

struct ExamFinding {
    finding_id:  String,
    category:    String,
    severity:    String,
    description: String,
    fine_amount: f64,
}

// ── Subsystem ────────────────────────────────────────────────────────────────

pub struct RegulatoryExamSubsystem {
    run_id:       RunId,
    config:       RegulatoryExamConfig,
    store:        SimStore,
    examiner_idx: usize,
}

impl RegulatoryExamSubsystem {
    pub fn new(run_id: RunId, config: RegulatoryExamConfig, store: SimStore) -> Self {
        Self { run_id, config, store, examiner_idx: 0 }
    }

    fn next_examiner(&mut self) -> String {
        let name = self.config.examiners
            .get(self.examiner_idx % self.config.examiners.len().max(1))
            .cloned()
            .unwrap_or_else(|| "OCC".into());
        self.examiner_idx += 1;
        name
    }

    /// Open a new exam at this tick.
    fn open_exam(&mut self, tick: Tick, rng: &mut SubsystemRng) -> SimResult<Vec<SimEvent>> {
        let examiner = self.next_examiner();
        // Alternate scope round-robin from the examiner index
        let scope = if self.examiner_idx % 3 == 0 {
            "targeted_aml"
        } else if self.examiner_idx % 3 == 1 {
            "targeted_complaints"
        } else {
            "full"
        };
        let exam_id = format!(
            "exam-{}-{}-{}",
            examiner.to_lowercase(),
            tick,
            rng.next_u64() % 10000
        );

        self.store.insert_regulatory_exam(
            &self.run_id, &exam_id, tick, &examiner, scope,
        )?;

        log::info!("tick={tick} regulatory exam opened: {exam_id} ({examiner} / {scope})");

        Ok(vec![SimEvent::RegulatoryExamStarted {
            tick,
            exam_id,
            examiner,
            scope: scope.into(),
        }])
    }

    /// Close the exam: evaluate evidence, record findings, emit events.
    fn close_exam(
        &self,
        tick: Tick,
        exam: &RegulatoryExamRow,
        rng:  &mut SubsystemRng,
    ) -> SimResult<Vec<SimEvent>> {
        let mut events = Vec::new();

        let (findings, fine_total, critical_count) = derive_findings(
            &self.run_id,
            &exam.exam_id,
            &self.store,
            &self.config,
            exam.tick_started,
            tick,
            rng,
        )?;

        // Persist findings
        for f in &findings {
            self.store.insert_exam_finding(
                &self.run_id,
                &exam.exam_id,
                &f.finding_id,
                tick,
                &f.category,
                &f.severity,
                &f.description,
                f.fine_amount,
            )?;
            events.push(SimEvent::ExamFindingRecorded {
                tick,
                exam_id: exam.exam_id.clone(),
                finding_id: f.finding_id.clone(),
                category: f.category.clone(),
                severity: f.severity.clone(),
                fine_amount: f.fine_amount,
            });
        }

        let mou_issued = critical_count >= self.config.mou_critical_threshold;
        let finding_count = findings.len() as i64;

        self.store.close_regulatory_exam(
            &self.run_id,
            &exam.exam_id,
            tick,
            fine_total,
            finding_count,
            mou_issued,
        )?;

        events.push(SimEvent::RegulatoryExamClosed {
            tick,
            exam_id: exam.exam_id.clone(),
            examiner: exam.examiner.clone(),
            finding_count,
            fine_total,
            mou_issued,
        });

        if mou_issued {
            events.push(SimEvent::MOUReceived {
                tick,
                exam_id: exam.exam_id.clone(),
                examiner: exam.examiner.clone(),
                fine_total,
            });
            log::warn!(
                "tick={tick} MOU issued by {} — {} findings, ${:.0} total fines",
                exam.examiner, finding_count, fine_total
            );
        } else {
            log::info!(
                "tick={tick} exam {} closed: {} findings, ${:.0}",
                exam.exam_id, finding_count, fine_total
            );
        }

        Ok(events)
    }
}

impl SimSubsystem for RegulatoryExamSubsystem {
    fn name(&self) -> &'static str {
        "regulatory_exam"
    }

    fn update(
        &mut self,
        tick:       Tick,
        _events_in: &[SimEvent],
        rng:        &mut SubsystemRng,
    ) -> SimResult<Vec<SimEvent>> {
        let mut out = Vec::new();

        if tick == 0 || !self.config.enabled {
            return Ok(out);
        }

        // 1. Check if an open exam's window has elapsed — close it first.
        if let Some(exam) = self.store.get_open_exam(&self.run_id)? {
            let elapsed = tick.saturating_sub(exam.tick_started);
            if elapsed >= self.config.exam_duration_ticks {
                out.extend(self.close_exam(tick, &exam, rng)?);
            }
        }

        // 2. Open a new exam at the configured interval (offset by 1 so tick 1 isn't instant).
        if tick > 1 && (tick - 1) % self.config.exam_interval_ticks == 0 {
            // Only open if no exam is currently running
            if self.store.get_open_exam(&self.run_id)?.is_none() {
                out.extend(self.open_exam(tick, rng)?);
            }
        }

        Ok(out)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
