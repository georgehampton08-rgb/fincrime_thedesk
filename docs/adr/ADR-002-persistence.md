# ADR-002: SQLite with WAL Mode

**Status**: Accepted  
**Date**: Phase 0  
**Author**: FinCrime: The Desk — Architecture

## Decision

SQLite in WAL (Write-Ahead Logging) mode for the MVP
desktop single-player version. The persistence layer is
abstracted behind SimStore so the backend can be swapped
for Postgres in Phase 4 (enterprise multi-user).

## Rationale

- Zero configuration for desktop deployment.
- WAL mode allows concurrent reads from the UI thread
  while the simulation thread writes.
- Sufficient for single-player scale (millions of rows
  across a full playthrough).
- rusqlite with the bundled feature provides a
  self-contained binary — no external database process.

## Consequences

- SimStore is the ONLY place SQL is executed.
  Subsystems call SimStore methods, never raw SQL.
- All schema changes are versioned migration files
  in /migrations, numbered sequentially.
- Phase 4 enterprise mode swaps SimStore for a
  PostgresStore that implements the same trait.
