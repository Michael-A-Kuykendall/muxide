# ADR-0001: Async I/O Support

- Status: accepted
- Decided by: muxide maintainers
- Date: 2026-08-09
- Related bead: muxide-1uq.1

## Context

The ROADMAP lists "Async I/O support: Optional tokio-based async operations for large file handling" as a high-priority goal. Muxide is a minimal-dependency MP4 muxer. The core write path (`Muxer<W>`) is synchronous over `std::io::Write`. The fragmented muxer (`FragmentedMuxer`) produces self-contained byte segments (init + moof+mdat) and already has a flush contract.

## Decision

Add an optional `async` feature (default off) that exposes an async writer variant for the fragmented muxer. The classic whole-file muxer is left synchronous because its `finalize()` path rewrites the file (moov-before-mdat), which is inherently buffered and gains no value from async.

## Options considered

- **A: blanket async fn on all types** — rejected. Doubles the public API surface with no callers; the classic muxer's rewrite path is inherently buffered.
- **B: optional `async` feature, async variant focused on fMP4** — accepted. Only the segment-flush path benefits from async; the classic muxer stays sync. tokio stays optional so the default build has no async dependency.
- **C: `futures::io::AsyncWrite` trait abstraction** — rejected. Adds a dependency; the tokio ecosystem works with `tokio::io` and tokio is the natural async runtime for the target use case (recording apps).

## Rationale

fMP4 yields self-contained byte segments and already has a flush contract; that maps 1:1 onto async writers. The classic muxer needs file rewriting at finalize (moov-before-mdat); async adds no value there and would multiply risk. "Recording large file handling" from the roadmap matches the asynchronous flush pattern. tokio stays optional so the crate keeps its minimal-dependency default build.

## Implementation plan

- Add `async` feature gate in `Cargo.toml` (tokio as optional dependency, `tokio = { version = "...", features = ["io-util"], optional = true }`).
- Expose an async fragment muxer (e.g. `FragmentedMuxer` with an async writer wrapper or a separate `AsyncFragmentedMuxer`) behind the feature.
- The sync path is unchanged; `cargo build` (no features) is byte-identical to before.
- Tests cover: segment ordering, empty-flush no-op, writer error propagation.

## Consequences

- Default build unchanged (no tokio dep).
- Async path only available behind `async` feature.
- Classic muxer remains synchronous.