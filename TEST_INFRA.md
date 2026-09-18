# Daft (`dft`) E2E Test Infrastructure Specification

**Document Version:** 1.0.0  
**Status:** Approved & Active  
**Scope:** Independent, Opaque-Box E2E Testing Track (Tiers 1–4)  
**Authoritative Sources:** `ORIGINAL_REQUEST.md`, `PROJECT.md`

---

## 1. Executive Summary & Testing Philosophy

Daft (`dft`) is an open-source, Rust-based version control system built for parallel AI agent workflows. Its hallmark innovations—parallel dimensions, copy-on-write workspaces, real-time cross-dimension radar, predictive conflict detection (`foresee`), ownership boundaries (`territory`), autonomous background synchronization (`cronos`, `entangle`), multi-agent coordination, and multiverse visualization (`timeline`)—require an uncompromising testing strategy.

The Daft project enforces a **Dual-Track Engineering Model**:
- **Track 1 (Implementation Track):** Iterative milestone development (M1 through M6) delivering core engines, CLI layers, and subsystem crates.
- **Track 2 (E2E Testing Track):** An independent, requirement-driven, opaque-box test harness that establishes rigorous acceptance gates in parallel with development.

### The Opaque-Box Principle
All end-to-end tests interact with Daft **strictly from the outside**:
1. **Zero Struct Coupling:** Tests execute the external CLI executable (`dft`) via subprocess invocations. Tests do not link against internal crate structs or assume private in-memory layouts.
2. **Observable Invariants:** Tests verify behaviors solely through CLI exit codes, standard streams (`stdout`, `stderr`), and verifiable on-disk filesystem artifacts (e.g., `.dft/` repository metadata, staging index, content-addressable storage, dimension workspaces, and worktree files).
3. **Black-Box Determinism:** Given a well-defined initial repository state and a sequence of CLI commands, tests assert that Daft produces the exact filesystem mutations and outputs dictated by `ORIGINAL_REQUEST.md`.

### Progressive Testability & Isolation
1. **Hermetic Sandboxing:** Each test allocates a temporary, cryptographically isolated sandbox directory (under `std::env::temp_dir()` or `/tmp/dft_test_<uuid>`).
2. **Environment Sanitization:** Subprocesses run with sanitized environment variables (`HOME`, `DFT_CONFIG`, `DFT_DIR`) preventing host leakage.
3. **Self-Contained Lifecycle:** Tests initialize their own repositories, create test fixtures, run assertions, and clean up their temporary directories upon exit via a RAII `Drop` guard. No test depends on execution order or shared state.

---

## 2. Test Architecture & Directory Topology

The E2E test suite resides entirely within `tests/e2e/`:

```
tests/e2e/
├── runner.rs                  # Self-contained executable test runner & metric aggregator
├── common/
│   └── mod.rs                 # Hermetic sandbox fixture, CLI subprocess invoker & assertions
├── tier1_features/            # Tier 1: Feature Coverage (>=5 test cases per feature)
│   ├── mod.rs                 # Tier 1 test registry and runner
│   ├── core_layer1.rs         # Core VCS, init, add, commit, branch, merge, diff, log, etc.
│   ├── dimensions.rs          # Parallel dimension lifecycle, CoW, workspaces, fork, snapshots
│   ├── awareness.rs           # Radar, hot zones, foresee, entropy, territory claims & fences
│   ├── convergence.rs         # Collapse, converge, cascade, weave, splice operations
│   ├── sync_daemon.rs         # Entanglement linked dimensions and Cronos sync daemon
│   ├── agent_timeline.rs      # Agent identity, mailbox, heartbeats, multiverse timeline graph
│   └── git_remote.rs          # Git import/export bridges, remote wire transfer
├── tier2_bounds/              # Tier 2: Boundary & Corner Cases
│   ├── mod.rs                 # Tier 2 test registry and runner
│   └── boundary_cases.rs      # Empty repos, huge files, deep trees, invalid syntax, unicode
├── tier3_pairwise/            # Tier 3: Pairwise Cross-Feature Combinations
│   ├── mod.rs                 # Tier 3 test registry and runner
│   └── pairwise_combinations.rs # Cross-feature multi-dimensional interactions
└── tier4_workload/            # Tier 4: Real-World Multi-Agent Workload Scenarios
    ├── mod.rs                 # Tier 4 test registry and runner
    └── multi_agent_scenarios.rs # 5+ parallel agents, hot zones, autonomous sync, collapse
```

---

## 3. Test Tiers & Categorization

The test suite is structured into four progressive acceptance tiers:

```
┌─────────────────────────────────────────────────────────────┐
│ Tier 4: Real-World Multi-Agent Workload Scenarios           │
│ (5+ parallel agents, hot zones, autonomous sync, collapse)  │
├─────────────────────────────────────────────────────────────┤
│ Tier 3: Pairwise Cross-Feature Combinations                 │
│ (Dimension + Stash, Territory Fence + Commit, etc.)         │
├─────────────────────────────────────────────────────────────┤
│ Tier 2: Boundary & Corner Cases                             │
│ (Empty repos, 50MB files, 50-deep trees, unicode, syntax)   │
├─────────────────────────────────────────────────────────────┤
│ Tier 1: Feature Coverage (>=5 tests per inventoried feature)│
│ (Core VCS, Dimensions, Awareness, Convergence, Sync, Agent) │
└─────────────────────────────────────────────────────────────┘
```

### Tier 1: Feature Coverage (>=5 Tests per Feature)
Validates each feature from the Feature Inventory (`PROJECT.md`) in isolation:
- **Core & Layer 1 (`core_layer1.rs`):** `init`, `add`, `commit`, `status`, `branch`, `checkout`, `switch`, `merge` (fast-forward and 3-way), `rebase`, `cherry-pick`, `diff`, `log`, `reset` (soft, mixed, hard), `restore`, `rm`, `mv`, `stash`, `clean`, `tag`, `blame`, `show`, `grep`, `reflog`, plumbing (`hash-object`, `cat-file`, `write-tree`, `ls-files`), maintenance (`gc`, `fsck`).
- **Parallel Dimensions (`dimensions.rs`):** `dimension create`, `dimension list`, `dimension enter`, `dimension destroy`, `dimension fork` (live uncommitted state), `dimension snapshot`, `dimension rename`, `dimension info`.
- **Awareness & Conflict Prevention (`awareness.rs`):** `radar`, `radar <path>`, `radar --hot`, `radar watch`, `foresee`, `foresee --continuous`, `overlap`, `entropy`, `claim`, `yield`, `fence`, `territory`, `territory audit`.
- **Convergence Operations (`convergence.rs`):** `collapse`, `converge`, `cascade`, `weave`, `splice`.
- **Entangle & Cronos (`sync_daemon.rs`):** `entangle`, `entangle list`, `entangle break`, `entangle log`, `cronos start`, `cronos stop`, `cronos status`, `cronos log`, `cronos pause`, `cronos resume`, `cronos config`.
- **Agent & Timeline (`agent_timeline.rs`):** `agent register`, `agent list`, `agent assign`, `agent status`, `agent broadcast`, `agent inbox`, `agent heartbeat`, `timeline`, `timeline <dim>`, `timeline --ancestry`, `timeline export`.
- **Git Interoperability & Remote (`git_remote.rs`):** `import git`, `export git`, `compat git-bridge`, `remote`, `fetch`, `pull`, `push`.

### Tier 2: Boundary Value Analysis & Corner Cases (`tier2_bounds/`)
Stress-tests the engine against system boundaries, invalid states, and resource constraints:
1. **Zero-State Repositories:** Running commands on freshly initialized repositories with zero commits (e.g., `status`, `log`, `diff`, `branch`, `dimension create`).
2. **Extreme File Sizes & Binaries:** Staging and committing binary blobs, files with null bytes, and 50MB+ datasets.
3. **Deep Directory Hierarchies:** Deeply nested paths (50+ folder levels) verifying path normalization and filesystem buffer handling.
4. **Invalid Syntax & Arguments:** Malformed CLI parameters, missing required arguments, non-existent revisions, and invalid dimension names.
5. **Internationalization & Path Encoding:** Unicode filenames, non-ASCII characters, spaces, punctuation, and emoji in file and branch names.
6. **Uncommitted Dirty State Transitions:** Switching branches or destroying active dimensions with uncommitted modifications.
7. **Concurrency & Race Conditions:** Concurrent CLI invocations accessing CAS and index locks simultaneously.
8. **Permissions & Read-Only Boundaries:** Read-only target directories, unwriteable working trees, and permission error propagation.

### Tier 3: Pairwise Cross-Feature Combinations (`tier3_pairwise/`)
Exercises combinations of two or more distinct subsystems interacting simultaneously:
1. **Dimension + Branch + Stash:** Stashing dirty modifications in Dimension A while creating and checking out a new branch in Dimension B.
2. **Territory Fence + Commit:** Setting a hard fence on a path in Dimension A and verifying that Dimension B is blocked from committing changes to that path.
3. **Dimension Live Fork + Uncommitted Changes + Status:** Forking Dimension B from live Dimension A containing uncommitted edits; verifying Dimension B inherits the exact uncommitted state while Dimension A remains untouched.
4. **Entangle + Cronos + Diff:** Establishing an entanglement rule between two dimensions, making edits in Dimension A, allowing Cronos sync to propagate changes, and asserting zero diff between entangled paths.
5. **Foresee + Converge:** Running `foresee` to verify zero predicted conflicts between two divergent dimensions, then performing `dft converge` to complete a clean merge.
6. **Weave + Commits + Timeline:** Interleaving commits across two independent dimensions, running `dft weave` to synthesize a causal topological history, and inspecting `dft timeline`.
7. **Observe + Dirty State:** Inspecting uncommitted working tree changes of a parallel dimension using `dft observe status` and `dft observe diff` without context switching.
8. **Collapse + Territory Claims:** Collapsing multiple dimensions into mainline while respecting territory claim boundaries.

### Tier 4: Real-World Multi-Agent Workload Scenarios (`tier4_workload/`)
End-to-end simulation of complex, collaborative multi-agent software engineering workflows:
1. **Scenario 1 — 5-Agent Parallel Swarm:**
   Five autonomous agents (`agent-auth`, `agent-billing`, `agent-ui`, `agent-search`, `agent-db`) register identities, claim their respective directory territories, operate in isolated parallel dimensions, make concurrent commits, report heartbeats, and exchange coordination messages via `dft agent broadcast`.
2. **Scenario 2 — Hot-Zone Radar Detection & Collision Prevention:**
   Two agents independently modify a shared configuration file. `dft radar --hot` flags the hot zone; `dft foresee` predicts a 3-way merge conflict; the agents communicate via inbox, one yields territory, and the second rebases before convergence.
3. **Scenario 3 — Autonomous Background Synchronization via Cronos:**
   Multiple dimensions share a common utility library. With `dft entangle` active and `dft cronos start` running, non-conflicting enhancements in the core library auto-propagate across all active dimensions, maintaining WAL logs and audit trails.
4. **Scenario 4 — Full Wavefunction Collapse to Mainline:**
   Four parallel feature dimensions diverge over 20+ commits. The orchestrator runs `dft foresee`, executes `dft converge` across complementary features, and finally executes `dft collapse --into mainline` to distill all features into a cohesive mainline commit with full provenance.

---

## 4. Expected Output Derivation & Authoritative Sources

To ensure test fidelity, all expected outputs are derived from authoritative sources:
1. **Primary Source:** `ORIGINAL_REQUEST.md` (System specifications, CLI commands, and R1–R10 acceptance criteria).
2. **Architectural Blueprint:** `PROJECT.md` (Crate topology, on-disk `.dft/` filesystem layout, and interface contracts).
3. **Deterministic State Assertions:**
   - Command exit code must be `0` for valid operations, non-zero (`!= 0`) for expected error conditions.
   - Filesystem mutations are verified via physical inspection of files in the sandbox working directory and `.dft/` internal directories.
   - SHA-256 CAS hashes and blob integrity are verified directly against expected content hashes.
4. **Dynamic Variance Masking:**
   - Timestamps, PIDs, UUIDs, and temporary directory paths are verified using prefix/contains matching or regex patterns rather than fragile literal strings.

---

## 5. Test Execution Framework & Tooling Guide

### Running the Test Runner

The test runner `tests/e2e/runner.rs` is a zero-dependency Rust executable that compiles directly with standard `rustc` or runs via `cargo`:

#### Option A: Direct Compilation & Execution
```bash
# Compile the standalone runner
rustc --edition 2021 tests/e2e/runner.rs -o target/e2e_runner

# Execute all test tiers
./target/e2e_runner

# Execute specific tier
./target/e2e_runner --tier 1
./target/e2e_runner --tier 2
./target/e2e_runner --tier 3
./target/e2e_runner --tier 4

# Run with custom binary location
./target/e2e_runner --dft-bin /path/to/dft

# Run in verification / dry-run mode (validates all test definitions)
./target/e2e_runner --verify
```

#### Option B: Cargo Test Integration
```bash
# Run tests via Cargo test harness
cargo test --test runner
```

### Environment Variables
- `DFT_BIN`: Path to the compiled `dft` executable. If unset, the runner searches `target/debug/dft`, `target/release/dft`, and system `$PATH`.
- `DFT_TEST_VERBOSE`: Enables detailed stdout/stderr logging for each test case (`1` or `true`).
- `DFT_TEST_FILTER`: String filter to execute a matching subset of tests.

### Output Report Format
The runner outputs a structured console summary:
```
================================================================================
                      DAFT E2E TEST EXECUTION SUMMARY
================================================================================
  Tier 1: Feature Coverage ................ 78 passed, 0 failed, 0 skipped
  Tier 2: Boundary & Corner Cases ......... 16 passed, 0 failed, 0 skipped
  Tier 3: Pairwise Cross-Feature .......... 12 passed, 0 failed, 0 skipped
  Tier 4: Multi-Agent Workload ............  4 passed, 0 failed, 0 skipped
--------------------------------------------------------------------------------
  TOTAL: 110 / 110 Passed (100.0% Success Rate)
  DURATION: 4.82s
  STATUS: READY FOR MILESTONE ACCEPTANCE
================================================================================
```
Exit code `0` is returned if and only if all executed tests pass.
