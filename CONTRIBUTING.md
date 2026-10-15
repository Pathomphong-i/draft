# Contributing to Draft (`dft`)

Thank you for your interest in contributing to **Draft** (`dft`), the open-source Distributed Version Control System for Parallel Timelines & Multi-Agent Workflows!

Draft is licensed under the **GNU General Public License v2.0 (GPL-2.0-only)** — the same license as the Linux kernel and Git. By contributing to this repository, you agree that your contributions will be licensed under the GPL-2.0-only license.

---

## Architecture & Workspace Layout

Draft is written in 100% pure Rust (Edition 2021) and organized as a Cargo workspace:

| Crate | Path | Responsibility |
|---|---|---|
| `daft-core` | `crates/daft-core` | Content-Addressable Storage (SHA-256 + zlib), binary index (`DFTI`), refs, commit DAG, Myers diff, 3-way merge |
| `daft-dimension` | `crates/daft-dimension` | Parallel dimensions, CoW engine (`clonefile` / `FICLONE` / hardlinks), Break-on-Write, live state fork, snapshots |
| `daft-awareness` | `crates/daft-awareness` | Cross-dimension `observe`, `radar` & hot zones, `foresee` conflict prediction, `entropy` scoring, `territory` claims/fences |
| `daft-convergence` | `crates/daft-convergence` | N-way `converge`, `collapse`, `cascade`, topological `weave`, and commit range `splice` |
| `daft-sync` | `crates/daft-sync` | Live file `entangle` engine and `cronos` background daemon with Write-Ahead Log (WAL) crash recovery |
| `daft-agent` | `crates/daft-agent` | Multi-agent identity registry, heartbeats, status aggregation, and lock-free Maildir messaging |
| `daft-cli` | `crates/daft-cli` | `dft` command-line binary, Layer 1 & Layer 2 command dispatch, Git bridge, bundle engine, and 3D Multiverse Web UI |

---

## Development Prerequisites

- **Rust Toolchain**: `1.75.0` or newer (`rustup update stable`)
- **OS**: macOS (APFS recommended for `clonefile(2)` tests) or Linux (Btrfs/XFS for `FICLONE` or ext4 for hardlink CoW)

---

## Building & Testing

1. **Build the workspace**:
   ```bash
   cargo build --workspace
   ```

2. **Run all unit, integration, and adversarial stress tests**:
   ```bash
   cargo test --workspace
   ```

3. **Run the 4-tier Opaque-Box E2E test suite**:
   ```bash
   cargo test --test tier1_runner
   ```

4. **Verify formatting and Clippy lints**:
   All pull requests must pass `rustfmt` and `clippy` with zero warnings:
   ```bash
   cargo fmt --all -- --check
   cargo clippy --workspace --all-targets -- -D warnings
   ```

---

## Coding Standards

1. **Zero Data Corruption**: Never mutate CAS objects or references in place. Always write to a temporary file (`NamedTempFile`), `fsync`, and atomically rename (`persist`) into place.
2. **Per-Dimension Concurrency**: Never introduce a global repository lock that blocks parallel agents operating in distinct dimensions. Use per-dimension `DimensionLockGuard` and atomic CAS writes.
3. **Break-on-Write Safety**: Whenever mutating a file inside a dimension workspace that may use POSIX hardlinks, ensure Break-on-Write unlinking is respected before truncation or in-place write.
4. **Structured Output**: New or updated CLI commands should support the global `--json` flag for deterministic machine parsing by AI agents.
5. **Test Coverage**: Every new feature or bug fix must include unit tests and, for CLI-facing changes, E2E test coverage in `tests/e2e/`.
