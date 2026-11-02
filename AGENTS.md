# Shared Agent Instructions for parley

This crate is Tool 8 of The Complete TideOS Eval Kit.

## Key Rules

1. Single static binary under 1.5 MB with zero `-sys` FFI dependencies.
2. Maintain `#![forbid(unsafe_code)]` at all times.
3. Universal `--json` output must use canonical `HarnessEnvelope<T>`.
4. Run all gates through the quiet runner: `node Tools/agent-harness/q.mjs -- <cmd>`.
5. Enforce zero em-dashes and zero en-dashes across all files.
