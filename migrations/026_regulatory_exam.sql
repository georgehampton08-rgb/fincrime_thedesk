-- Phase 3.6: Regulatory Examination tables
--
-- regulatory_exam: one row per exam cycle (quarterly by default).
-- exam_finding: one row per finding raised within an exam.
CREATE TABLE IF NOT EXISTS regulatory_exam (
    exam_id TEXT PRIMARY KEY,
    run_id TEXT NOT NULL REFERENCES run(run_id),
    tick_started INTEGER NOT NULL,
    tick_completed INTEGER,
    -- NULL while open
    examiner TEXT NOT NULL,
    -- "OCC" | "CFPB" | "FDIC" | "FRB"
    scope TEXT NOT NULL,
    -- "full" | "targeted_aml" | "targeted_complaints"
    status TEXT NOT NULL DEFAULT 'open',
    -- "open" | "closed"
    finding_count INTEGER NOT NULL DEFAULT 0,
    fine_total REAL NOT NULL DEFAULT 0.0,
    mou_issued INTEGER NOT NULL DEFAULT 0 -- boolean 0/1
);
CREATE INDEX IF NOT EXISTS idx_regulatory_exam_run ON regulatory_exam(run_id, tick_started);
CREATE TABLE IF NOT EXISTS exam_finding (
    finding_id TEXT PRIMARY KEY,
    exam_id TEXT NOT NULL REFERENCES regulatory_exam(exam_id),
    run_id TEXT NOT NULL,
    tick INTEGER NOT NULL,
    category TEXT NOT NULL,
    -- "aml" | "complaint_sla" | "sar_timeliness" | "data_integrity"
    severity TEXT NOT NULL,
    -- "minor" | "moderate" | "major" | "critical"
    description TEXT NOT NULL,
    fine_amount REAL NOT NULL DEFAULT 0.0
);
CREATE INDEX IF NOT EXISTS idx_exam_finding_exam ON exam_finding(exam_id);
CREATE INDEX IF NOT EXISTS idx_exam_finding_run ON exam_finding(run_id, tick);