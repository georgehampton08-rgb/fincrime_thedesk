# ADR-001: Daily Discrete Tick Model

**Status**: Accepted  
**Date**: Phase 0  
**Author**: FinCrime: The Desk — Architecture

## Decision

The simulation advances in discrete daily ticks.
One tick = one in-game day. Time is a u64 counter, never a
real-world timestamp.

## Alternatives Considered

- **Real-time with pause**: Common in grand strategy games.
  Rejected: increases cognitive load in a text-heavy sim,
  complicates deterministic testing, creates UI pressure
  to react rather than plan.
- **Weekly ticks**: Considered for performance.
  Rejected: daily ticks match banking's own operating
  cadence (daily batch processing, D+1 reconciliations)
  and allow modeling intraday events correctly.

## Consequences

- All scheduled events use Tick offsets, never wall-clock time.
- SLA deadlines, SAR filing deadlines, and regulatory exam
  dates are expressed as tick counts.
- Fast-forward is implemented by running multiple ticks per
  real-world frame — the engine does not know about real time.
