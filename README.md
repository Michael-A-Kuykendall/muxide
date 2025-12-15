# Muxide

**Muxide** is a recording‑oriented multimedia container writer for Rust.  Its goal is to provide a simple, ergonomic API for muxing encoded video and audio frames into an MP4 container with real‑world playback guarantees.

This crate is built following the principles of the *Slice‑Gated Engineering Doctrine*, where work is broken into small, verifiable slices with clear acceptance gates.  See the `docs/charter.md` and `docs/contract.md` for the high‑level goals and API contract of the project.

> *This README only explains what the project aims to be.  Implementation details and API stability are driven by the charter and contract documents.*