# ADR-004: Strict Simulation / UI Separation

**Status**: Accepted  
**Date**: Phase 0  
**Author**: FinCrime: The Desk — Architecture

## Decision

/core is a pure Rust library with zero UI, I/O, or
platform dependencies (except SQLite via rusqlite).
/client (Godot) communicates with /core only through
a defined API surface: query methods and PlayerCommand.

## Rationale

- Pure library enables testing without launching the game.
- Enables a future enterprise web client using the same
  simulation core.
- Prevents the most common architectural drift in game
  development: game logic leaking into the UI layer.

## Enforcement

- /core/Cargo.toml has no dependencies on any GUI or
  windowing crate.
- CI module boundary check: build /core in isolation
  (cargo build -p fincrime-core). Must succeed with zero
  warnings relating to unused UI imports.
- /client code that directly accesses SimEngine internals
  is rejected in code review.
