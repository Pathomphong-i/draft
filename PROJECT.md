# Project: Daft (`dft`) Version Control System

## Architecture
Daft is a production-quality, open-source, Rust-based version control system designed for parallel AI agent workflows. It combines a full suite of Git-equivalent Layer 1 commands with an advanced Layer 2 multi-agent multiverse engine (parallel dimensions, copy-on-write workspaces, predictive conflict prevention, real-time radar, convergence operations, autonomous sync daemons, agent identities/mailboxes, and multiverse graph visualization).

### Crate Topology (Acyclic Dependency DAG)
```
                  ┌──────────────────────┐
                  │      daft-cli        │ (Executable binary `dft`)
                  └──────────┬───────────┘
         ┌────────────┬──────┴───────┬────────────┐
         │            │              │            │
         ▼            ▼              ▼            ▼
┌────────────────┐ ┌────────────────┐ ┌────────────────┐ ┌──────────────┐
│daft-convergence│ │   daft-sync    │ │   daft-agent   │ │   daft-git   │
└────────┬───────┘ └────────┬───────┘ └────────┬───────┘ └──────┬───────┘
         │                  │                  │                │
         └────────────┬─────┴──────────────────┘                │
                      ▼                                         │
            ┌──────────────────┐                                │
            │  daft-awareness  │                                │
            └─────────┬────────┘                                │
                      ▼                                         │
            ┌──────────────────┐                                │
            │  daft-dimension  │                                │
            └─────────┬────────┘                                │
                      ▼                                         │
            ┌──────────────────┐                                │
            │    daft-core     │◄───────────────────────────────┘
            └──────────────────┘
```

- **`daft-core`**: SHA-256 CAS object store, loose & packfile formats, binary index (`.dft/index`), refs, commit graph, reflogs, config, staging, diff/merge engines, and Layer 1 command implementations.
- **`daft-dimension`**: Dimension lifecycle (`create`, `list`, `enter`, `destroy`, `fork`, `snapshot`, `rename`, `info`), isolated dimension workspaces, OS-native CoW (macOS APFS `clonefile`, Linux `ioctl(FICLONE)`, hardlinks fallback), fine-grained per-dimension locking.
- **`daft-awareness`**: Cross-dimension radar, hot zones, watch mode, predictive conflict detection (`foresee`), overlap detection, divergence entropy mathematical scoring, territory claims/locks/audits.
- **`daft-convergence`**: Multi-dimension convergence operations: `converge` (multi-way merge), `collapse` (squash/distill multiple dimensions into mainline commit), `cascade` (sequential propagation), `weave` (topological causal synthesis via vector clocks), `splice` (slice transplant).
- **`daft-sync`**: `entangle` (linked dimension live sync rules) and `cronos` (autonomous background sync daemon, WAL crash recovery).
- **`daft-agent`**: Multi-agent system (identity registration, dimension assignment, status, broadcast, Maildir-style inbox, heartbeats).
- **`daft-git`**: Bidirectional pure-Rust Git interoperability bridge (`git-import`, `git-export`, `.dft/git_map` translation table).
- **`daft-cli`**: Unified CLI entry point (`dft`) using `clap` v4 with comprehensive subcommands, human-friendly and JSON outputs, and multiverse graph visualization (`timeline`).

### On-Disk Filesystem Layout (`.dft/`)
```
.dft/
├── dft.toml                 # Repository configuration
├── HEAD                     # Current symbolic ref or detached commit OID
├── current_dimension        # Name of active dimension (default: "mainline")
├── index                    # Staging index (DIRC format, SHA-256)
├── objects/                 # Content-Addressable Storage (CAS)
│   ├── [0-9a-f]{2}/[0-9a-f]{62} # Loose objects (blobs, trees, commits, tags)
│   ├── pack/                # Packfiles (.pack) and indices (.idx)
│   └── tmp/                 # Atomic write staging area
├── refs/
│   ├── heads/               # Branch references
│   ├── tags/                # Tag references
│   └── dimensions/          # Dimension head references
├── logs/                    # Reflog history
├── dimensions/              # Dimension isolated workspaces
│   └── <dim_name>/
│       ├── meta.json        # Dimension metadata (creator, branch, parent, CoW mode)
│       ├── index            # Dimension-specific index
│       ├── HEAD             # Dimension-specific HEAD ref
│       └── workspace/       # Isolated working tree (CoW cloned)
├── territory/               # Territory claims and fences
│   ├── claims.json          # File path claims
│   └── fences.json          # Hard edit barriers
├── entangle/                # Entanglement rules
│   └── rules.json           # Live sync definitions
├── cronos/                  # Cronos daemon state
│   ├── daemon.pid           # Active process ID
│   ├── daemon.sock          # Unix domain socket for IPC
│   ├── wal.jsonl            # Write-ahead event log
│   └── config.json          # Sync intervals and modes
├── agents/                  # Registered agent identities
│   └── <agent_id>.json
├── messages/                # Maildir-style agent communication
│   └── <agent_id>/
│       ├── new/
│       └── cur/
└── git_map                  # Git OID <-> Daft OID bidirectional mapping
```

---

## Feature Inventory
Every feature from the Survey phase is mapped to an assigned milestone.

| # | Feature | Description | Milestone | Source |
|---|---------|-------------|-----------|--------|
| 1 | `dft init` | Initialize new Daft repository with `.dft/` structure | M1 | Survey 1 (Core) |
| 2 | CAS Object Store | SHA-256 CAS store for blobs, trees, commits, tags with zlib | M1 | Survey 1 (Core) |
| 3 | Loose & Pack Format | Loose object format and packfile compression / indexing | M1 | Survey 1 (Core) |
| 4 | Binary Index | Staging index (`.dft/index`) with DIRC header & multi-stage entries | M1 | Survey 1 (Core) |
| 5 | References & HEAD | Branch/tag references, symbolic & detached HEAD, atomic `.lock` | M1 | Survey 1 (Core) |
| 6 | Reflog Engine | Append-only reflog history in `.dft/logs/` | M1 | Survey 1 (Core) |
| 7 | Config System | Repository & global configuration (`dft.toml`) | M1 | Survey 1 (Core) |
| 8 | `dft clone` | Clone local or remote repository | M2 | Survey 1 (Layer 1) |
| 9 | `dft config` | Inspect and mutate configuration values | M2 | Survey 1 (Layer 1) |
| 10 | `dft help` & `version` | Built-in command help, manpages, and versioning | M2 | Survey 1 (Layer 1) |
| 11 | `dft add` | Stage modified/untracked files into index | M2 | Survey 1 (Layer 1) |
| 12 | `dft status` | Working tree, staged, unstaged, and untracked file status | M2 | Survey 1 (Layer 1) |
| 13 | `dft commit` | Record staged changes as commit with author/committer & timestamp | M2 | Survey 1 (Layer 1) |
| 14 | `dft reset` | Soft, mixed, hard reset of HEAD and staging index | M2 | Survey 1 (Layer 1) |
| 15 | `dft restore` | Restore working tree or index from commit/tree | M2 | Survey 1 (Layer 1) |
| 16 | `dft rm` | Remove files from working tree and staging index | M2 | Survey 1 (Layer 1) |
| 17 | `dft mv` | Move or rename tracked files | M2 | Survey 1 (Layer 1) |
| 18 | `dft stash` | Push, pop, list, apply, drop working directory stashes | M2 | Survey 1 (Layer 1) |
| 19 | `dft clean` | Remove untracked files and directories (`-f`, `-d`) | M2 | Survey 1 (Layer 1) |
| 20 | `dft branch` | Create, list, delete (`-d`, `-D`), rename branches | M2 | Survey 1 (Layer 1) |
| 21 | `dft checkout` | Checkout branch, commit, or files | M2 | Survey 1 (Layer 1) |
| 22 | `dft switch` | Switch active branches | M2 | Survey 1 (Layer 1) |
| 23 | `dft merge` | Fast-forward, 3-way recursive merge, conflict markers | M2 | Survey 1 (Layer 1) |
| 24 | `dft rebase` | Rebase commit history onto target branch | M2 | Survey 1 (Layer 1) |
| 25 | `dft cherry-pick` | Apply specific commit diff onto current branch | M2 | Survey 1 (Layer 1) |
| 26 | `dft tag` | Create lightweight and annotated tags | M2 | Survey 1 (Layer 1) |
| 27 | `dft log` | Commit history graph, format options, range filtering | M2 | Survey 1 (Layer 1) |
| 28 | `dft diff` | Working tree vs index vs commit diff generation | M2 | Survey 1 (Layer 1) |
| 29 | `dft show` | Display commit, tree, tag, or blob contents | M2 | Survey 1 (Layer 1) |
| 30 | `dft blame` | Line-by-line commit authorship attribution | M2 | Survey 1 (Layer 1) |
| 31 | `dft shortlog` | Summarize log output by author | M2 | Survey 1 (Layer 1) |
| 32 | `dft describe` | Find nearest reachable tag for commit | M2 | Survey 1 (Layer 1) |
| 33 | `dft grep` | Search tracked files for patterns | M2 | Survey 1 (Layer 1) |
| 34 | `dft bisect` | Binary search commit history for regressions | M2 | Survey 1 (Layer 1) |
| 35 | `dft reflog` | Manage and inspect reference update logs | M2 | Survey 1 (Layer 1) |
| 36 | `dft plumbing` | Low-level CAS inspection/mutation: hash-object, cat-file, write-tree, commit-tree, ls-tree, ls-files, rev-parse, update-ref, symbolic-ref | M2 | Survey 1 (Layer 1) |
| 37 | `dft maintenance` | Maintenance operations: gc, fsck, prune, repack, count-objects | M2 | Survey 1 (Layer 1) |
| 38 | Dimension Lifecycle | `dimension create`, `list`, `enter`, `destroy`, `rename`, `info` | M3 | Survey 2 (Dimensions) |
| 39 | Dimension Workspaces | Isolated directories under `.dft/dimensions/<name>/` sharing CAS | M3 | Survey 2 (Dimensions) |
| 40 | Dimension CoW Engine | macOS APFS `clonefile`, Linux `FICLONE`, hardlink BoW fallback | M3 | Survey 2 & 3 (CoW) |
| 41 | Dimension Live Fork | Fork uncommitted live workspace state into new dimension | M3 | Survey 2 (Dimensions) |
| 42 | Dimension Snapshots | Immutable point-in-time dimension state snapshots | M3 | Survey 2 (Dimensions) |
| 43 | Dimension Concurrency | Fine-grained per-dimension locks, lock-free CAS, 5+ parallel agents | M3 | Survey 2 & 3 (Concurrency) |
| 44 | `observe` | Non-destructive inspection of other dimensions (status, diff, log) | M4 | Survey 2 (Observe) |
| 45 | `radar` & Hot Zones | Cross-dimension awareness, active dimensions, hot zones, watch mode | M4 | Survey 2 (Radar) |
| 46 | `foresee` & Overlap | Predictive 3-way virtual conflict detection before merging | M4 | Survey 2 (Foresee) |
| 47 | Divergence Entropy | Quantitative mathematical conflict risk scoring $H(D_1, D_2)$ | M4 | Survey 2 (Entropy) |
| 48 | `territory` Subsystem | File path claims (`claim`), release (`yield`), locks (`fence`), `audit` | M4 | Survey 2 (Territory) |
| 49 | `converge` | Multi-dimension convergence merge into active dimension | M4 | Survey 2 (Convergence) |
| 50 | `collapse` | Selective squash/distill of multiple dimensions into mainline commit | M4 | Survey 2 (Convergence) |
| 51 | `cascade` | Sequential propagation across ordered chain of dimensions | M4 | Survey 2 (Convergence) |
| 52 | `weave` | Topological causal synthesis of interleaved commits via vector clocks | M4 | Survey 2 (Convergence) |
| 53 | `splice` | Transplant specific commit slice or diff across dimensions | M4 | Survey 2 (Convergence) |
| 54 | `entangle` | Autonomous live sync rules between dimensions with file filters | M5 | Survey 2 (Entangle) |
| 55 | `cronos` Daemon | Background sync daemon (`start`, `stop`, `status`, `log`, `sync`, WAL) | M5 | Survey 2 (Cronos) |
| 56 | Agent Subsystem | Agent identity register, assign, status, broadcast, Maildir inbox | M5 | Survey 2 (Agent) |
| 57 | Agent Heartbeat | Liveness tracking and agent status monitoring | M5 | Survey 2 (Agent) |
| 58 | `timeline` Multiverse | Visual branch/dimension graph (ASCII/Unicode, JSON, DOT, SVG) | M5 | Survey 2 (Timeline) |
| 59 | Git Import Bridge | Fast import of Git repositories into Daft format | M6 | Survey 3 (Git Bridge) |
| 60 | Git Export Bridge | Export Daft repository/dimension to standard Git repository | M6 | Survey 3 (Git Bridge) |
| 61 | `git_map` CAS Table | Append-only bidirectional Git OID <-> Daft OID translation table | M6 | Survey 3 (Git Bridge) |
| 62 | Remote Wire Protocol | Remote fetch/push/pull, want/have negotiation, `.dpack` streaming | M6 | Survey 1 & 3 (Remote) |
| 63 | Performance Validation | Sub-5s dimension creation for 1,000 files, sub-linear disk usage | M6 | Survey 3 (Performance) |
| 64 | Open Source Packaging | GPL v2 LICENSE, README.md, CONTRIBUTING.md, CI workflow | M6 | Survey 3 (Packaging) |

---

## Milestones

| # | Name | Scope | Dependencies | Status |
|---|------|-------|-------------|--------|
| M1 | Core VCS Engine & Workspace Foundation | Cargo workspace (`daft-core`), SHA-256 CAS object store, blobs/trees/commits/tags, zlib, binary index, refs/HEAD, atomic locks, config, `dft init` | none | DONE |
| M2 | Layer 1 Git-Equivalent Operations & CLI | All 71 Git-equivalent commands (porcelain & plumbing): add, status, commit, branch, checkout, switch, merge 3-way, rebase, diff, log, reset, stash, blame, tag, gc, fsck, etc. | M1 | DONE |
| M3 | Parallel Dimension Engine & CoW Workspaces | `daft-dimension`: dimension lifecycle, CoW storage engine (macOS `clonefile`, Linux `FICLONE`, hardlinks), isolated workspaces, live state fork, snapshots, 5+ concurrency | M1, M2 | DONE |
| M4 | Awareness, Conflict Prevention & Convergence Operations | `daft-awareness` & `daft-convergence`: `observe`, `radar` & hot zones, `foresee`, divergence entropy scoring, `territory` claims/fences, `converge`, `collapse`, `cascade`, `weave`, `splice` | M3 | DONE |
| M5 | Autonomous Entanglement, Cronos, Agent & Timeline | `daft-sync` & `daft-agent`: `entangle` live sync rules, `cronos` daemon (WAL recovery), agent identity/inbox/broadcast/heartbeat, `timeline` multiverse graph (ASCII/SVG/JSON) | M3, M4 | DONE |
| M6 | Git Bridge, Remote Operations, Packaging & CI | `daft-git` & `daft-cli`: bidirectional Git import/export, `git_map`, remote wire transfer, GPL v2 LICENSE, README.md, CONTRIBUTING.md, GitHub Actions CI workflow, Clippy clean | M1–M5 | IN_PROGRESS |
| E2E | Opaque-Box E2E Testing Track | Independent test harness, test runner, Tiers 1-4 comprehensive test suite (Feature coverage, Boundaries, Combinations, Real-world workloads), publishes `TEST_READY.md` | none (Parallel) | DONE |
| FINAL | Final Acceptance & Adversarial Hardening | Phase 1: 100% E2E test suite pass; Phase 2: Tier 5 adversarial white-box coverage hardening (Challenger loop); Forensic Integrity Audit signoff | M1–M6, E2E | PLANNED |

---

## Code Layout
```
/Users/pathomphongphiphatsuriyawong/Workspace/Daft/
├── Cargo.toml               # Virtual workspace manifest
├── Cargo.lock
├── LICENSE                  # GNU General Public License v2.0
├── README.md                # Comprehensive documentation & architecture guide
├── CONTRIBUTING.md          # Contribution guidelines & coding standards
├── .github/
│   └── workflows/
│       └── ci.yml           # CI: cargo test, clippy, fmt, coverage
├── crates/
│   ├── daft-core/           # CAS, Index, Refs, Commit Graph, Objects, Diff, Merge
│   ├── daft-dimension/      # Dimensions, CoW (macOS/Linux), Workspaces, Snapshots
│   ├── daft-awareness/      # Radar, Foresee, Entropy, Territory
│   ├── daft-convergence/    # Converge, Collapse, Cascade, Weave, Splice
│   ├── daft-sync/           # Entangle, Cronos Daemon, WAL
│   ├── daft-agent/          # Agent Identities, Mailbox, Heartbeat
│   ├── daft-git/            # Pure Rust Git import/export & hash translation
│   └── daft-cli/            # Main binary `dft` CLI
└── tests/
    └── e2e/                 # Independent Opaque-Box E2E Test Suite
        ├── runner.rs        # Test runner harness
        ├── tier1_features/  # Feature coverage (>=5 tests per feature)
        ├── tier2_bounds/    # Boundary & corner cases
        ├── tier3_pairwise/  # Cross-feature combinations
        └── tier4_workload/  # Real-world multi-agent workload scenarios
```

---

## Interface Contracts

### `daft-core` ↔ `daft-dimension`
- **CAS Store Access**:
  `cas::ObjectStore::write_object(&self, obj: &Object) -> Result<ObjectId, CasError>` (thread-safe, atomic tempfile+rename).
  `cas::ObjectStore::read_object(&self, id: &ObjectId) -> Result<Object, CasError>` (lock-free read).
- **Index Management**:
  `index::Index::read_from(path: &Path) -> Result<Index, IndexError>`
  `index::Index::write_to(&self, path: &Path) -> Result<(), IndexError>`
- **Working Tree Checkout**:
  `worktree::checkout_tree(cas: &ObjectStore, tree_id: &ObjectId, target_dir: &Path, cow_mode: CowMode) -> Result<(), CheckoutError>`

### `daft-dimension` ↔ `daft-awareness`
- **Dimension State Inspection**:
  `dimension::DimensionManager::list_dimensions(&self) -> Result<Vec<DimensionInfo>, DimensionError>`
  `dimension::DimensionManager::get_dirty_files(&self, dim: &str) -> Result<Vec<PathBuf>, DimensionError>`
- **Lock Acquisition**:
  `dimension::DimensionManager::acquire_dimension_lock(&self, dim: &str) -> Result<DimensionLockGuard, LockError>`

### `daft-awareness` ↔ `daft-convergence`
- **Foresee Virtual Merge**:
  `foresee::predict_conflicts(base_tree: &ObjectId, ours_tree: &ObjectId, theirs_tree: &ObjectId) -> VirtualMergeResult`
- **Entropy Calculation**:
  `entropy::calculate_entropy(dim_a: &DimensionInfo, dim_b: &DimensionInfo, ast_weight: f64) -> f64`

### `daft-convergence` ↔ `daft-sync`
- **Trigger Synchronous / Asynchronous Convergence**:
  `convergence::execute_converge(target_dim: &str, source_dims: &[&str], strategy: MergeStrategy) -> Result<CommitId, ConvergenceError>`
