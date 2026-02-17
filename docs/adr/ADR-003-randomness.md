# ADR-003: Seeded Per-Subsystem PRNG (PCG64)

**Status**: Accepted  
**Date**: Phase 0  
**Author**: FinCrime: The Desk — Architecture

## Decision

All simulation randomness flows through SubsystemRng
instances derived deterministically from a master seed.
Platform RNGs (rand::thread_rng(), SystemTime, etc.)
are forbidden in /core.

## Rationale

- Deterministic replay requires all randomness to be
  seed-controlled and reproducible.
- Per-subsystem RNGs mean adding a new subsystem never
  changes existing subsystems' random streams.
- PCG64Mcg: fast, high-quality, well-studied generator
  with no platform dependencies.

## Enforcement

- CI lint step: grep for "thread_rng" and "SystemTime"
  in /core — fail the build if found.
- The RngBank uses stable slot indices (SubsystemSlot
  enum). Slots are append-only. Reordering is forbidden.
