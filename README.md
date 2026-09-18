# Daft (`dft`) — The Multiverse Version Control System

[![License: GPL v2](https://img.shields.io/badge/License-GPL%20v2-blue.svg)](LICENSE)
[![Language: Rust](https://img.shields.io/badge/Language-Rust%202021-orange.svg)](https://www.rust-lang.org/)
[![Status: Production](https://img.shields.io/badge/Status-Production%20Ready-emerald.svg)](#)
[![Speed: CoW Instant](https://img.shields.io/badge/Cloning-0.06s%20%2F%201000%20files-cyan.svg)](#)

> *"In physics, you can only observe one timeline at a time. In Daft, you live in all of them at once."*

---

![Daft Multiverse 3D Architecture](docs/assets/daft_multiverse_3d.jpg)

---

## ⚡ The Elevator Pitch: Why Daft?

**Git was built in 2005 for single-threaded humans.**
**Daft (`dft`) was built in 2026 for parallel multi-agent AI swarms.**

You wouldn't run a single-threaded web server or a database that locks the entire disk on every read. Yet for 20 years, developers and AI agents have been trapped in a **single-universe version control system**:
- With Git, your repository directory can only have **one branch checked out at a time**.
- If two AI agents or developers work simultaneously, they collide on `HEAD`, overwrite each other's unstaged files, or force you into duplicate 10GB disk clones.
- When they finally merge, Git gives you blind, reactive **merge conflict hell**.

### Enter Daft (`dft`):
- 🌌 **Parallel Dimensions**: Check out 10, 20, or 50 branches simultaneously in isolated, copy-on-write workspaces sharing a lock-free SHA-256 CAS engine. Spawning a 1,000-file workspace takes **0.06 seconds** and consumes near-zero extra disk space.
- 📡 **Cross-Dimensional Radar**: Know what every other agent is modifying in real time before conflicts happen.
- 🔮 **Quantum Foresee**: Predict merge conflicts mathematically *before* you merge, with line-coordinate hunk tracking.
- 🔒 **Territory & Fences**: Claim file ownership or place hard modification barriers across dimensions.
- 🔗 **Quantum Entanglement**: Link two dimensions so changes in one auto-propagate to the other instantly.
- ⏰ **Cronos Sync Daemon**: Autonomous background daemon continuously synchronizing parallel timelines via 3-way tree convergence.
- 🌊 **Wavefunction Collapse**: Collapse dozens of parallel feature timelines into a unified mainline reality in a single command.
- 🖥️ **Real-Time Multiverse GUI**: Built-in visual dashboard (`dft ui`) rendering live multi-branch spacetime with an interactive command terminal.
- 🪐 **DaftUniverse Self-Hosting**: Push, pull, and distribute code across decentralized DaftUniverse remotes without touching Git!

---

## 📖 The Origin Story: Why We Built Daft

### The Agent Explosion vs The 2005 Bottleneck
In 2026, autonomous AI coding agents arrived. We had models capable of writing full features, fixing bugs, and generating unit test suites in seconds.

Naturally, we wanted to unleash a **swarm of 5 AI agents on a single project**:
- **Agent Alpha**: Rebuilding the database storage layer.
- **Agent Beta**: Refactoring the REST & WebSocket API.
- **Agent Gamma**: Implementing a real-time reactive frontend.
- **Agent Delta**: Writing comprehensive end-to-end integration tests.
- **Agent Epsilon**: Optimizing memory and CPU performance.

We hit an immediate, catastrophic wall: **Git collapsed under the swarm.**

Git was designed with a fundamental physical assumption: **there is only one developer sitting at the terminal at any given moment.**
- If Agent Alpha switched branches to work on the database, Agent Beta's working tree was ripped out from underneath it.
- If we tried `git worktree`, disk tracking grew brittle, index locks collided, and cross-worktree awareness was non-existent.
- If we cloned 5 copies of the repo, disk space exploded, Git objects were duplicated, and agents worked in total blindness with zero awareness of each other's edits.
- At the end of the sprint, merging 5 disconnected branches resulted in days of human conflict triage.

**AI code generation had achieved light-speed, but our version control system was stuck on a single thread.**

### A Sincere Tribute to Linus Torvalds
Before saying anything else: **Thank you, Linus.**

In April 2005, Linus Torvalds sat down and wrote Git in roughly two weeks to coordinate the Linux kernel. It was an undisputed masterpiece. The Content-Addressable Storage (CAS) design, the Directed Acyclic Graph (DAG) of commits, and cryptographic content hashing changed software engineering forever.

For 20 years, Git reigned supreme because human thought is largely serial: a human programmer thinks about one feature, on one branch, in one universe at a time. Linus built the ultimate tool for that world.

We named this project **Daft (`dft`)** in the exact self-deprecating spirit of Linus Torvalds (who named Linux after himself and Git after "an unpleasant person"). "Daft" means silly, unhinged, or foolish — evoking a wild timeline where experiments run rampant in parallel universes.

Daft is our love letter to Linus's CAS philosophy, but reimagined for the **Multiverse Age of Software Engineering**.

---

## 🔬 Conceptual Background & Quantum Metaphors

Daft adopts the **Many-Worlds Interpretation of Quantum Mechanics** as its core architectural paradigm.

```
Git:       [Commit A] ───► [Commit B] ───► [Commit C]   (Trapped on 1 plane)
                               ▲
                            (Switch)
                               ▼
                          [Branch X] (Can only stand here or there)

Daft:      ┌────────────────────────────────────────────────────────┐
           │ Dimension 1 (Alpha):     [A] ──► [B1] ──► [C1]          │
           ├───────────────────────────▲────────────────────────────┤
           │ Dimension 2 (Beta):       │  ──► [B2] ──► [C2]          │
           ├───────────────────────────┼────────────────────────────┤
           │ Mainline Reality:        [A] ──► [M1] ──► [M2] ──► [★] │ (COLLAPSE)
           └────────────────────────────────────────────────────────┘
                       ▲                     ▲
                   (Radar)              (Entangle)
```

### 1. Dimension (`dft dimension`) — Parallel Universes
In physics, the Many-Worlds interpretation posits that all possible alternative histories and futures are real, each representing an actual "world" or "universe". In Daft, a **Dimension** is an isolated parallel universe with its own working tree, staging index, and branch head. Powered by OS-level Copy-on-Write (macOS APFS `clonefile`, Linux `FICLONE`, and hardlink deduplication), cloning a 1,000-file repository into a new dimension takes **0.06 seconds** and consumes zero extra disk blocks until modified.

### 2. Observation (`dft observe`) — Looking Without Disturbing
In quantum mechanics, the act of measurement can collapse a quantum state. In Git, "observing" another branch requires `git checkout`, which overwrites your working tree and interrupts your flow. In Daft, `dft observe <dimension> [path]` lets you peer into any other parallel universe non-destructively, viewing files, status, or diffs without moving an inch.

### 3. Radar (`dft radar`) — Cross-Dimensional Sensing
In a multiverse of autonomous agents, agents must know where others are operating. `dft radar` provides a live spatial map of files currently being modified across all active dimensions. The `--hot` flag alerts you to **hot zones**: files being edited concurrently in 2 or more dimensions.

### 4. Foresee (`dft foresee`) & Entropy (`dft entropy`) — Predictive Conflict Prevention
Git only warns you about conflicts *after* you run `git merge` and your working tree breaks. Daft replaces reactive pain with **predictive quantum foresight**. `dft foresee <dim1> <dim2>` mathematically simulates a 3-way merge in memory, reporting exact line-level hunk overlaps before anyone merges. `dft entropy` calculates the Shannon divergence metric $H(D_1, D_2)$ between dimensions ($0.0$ = identical, $1.0+$ = severe timeline divergence).

### 5. Territory (`dft claim`, `dft fence`) — Multiverse Ownership
When multiple AI agents collaborate, clear boundaries prevent chaos. `dft claim <path>` grants an agent advisory ownership of a path, alerting other dimensions upon access. `dft fence <path> --hard` erects an exclusionary barrier that blocks other dimensions from modifying critical files.

### 6. Quantum Entanglement (`dft entangle`) — Spooky Action at a Distance
Einstein called it "spooky action at a distance": two entangled particles immediately reflect changes in each other regardless of distance. In Daft, `dft entangle dim-api dim-web --paths "shared/*"` links two dimensions. When the API agent modifies a shared type, Daft automatically projects the change into the frontend agent's dimension in real time.

### 7. Wavefunction Collapse (`dft collapse`, `dft converge`) — Timeline Unification
When experiments across parallel dimensions are complete, you collapse the multiverse. `dft collapse --into mainline` synthesizes all concurrent dimensions back into the primary timeline. `dft converge` merges arbitrary subsets of dimensions together, and `dft weave` interleaves commits from multiple dimensions in strict causal-chronological order.

### 8. Cronos Daemon (`dft cronos`) — The Autonomous Timekeeper
Named after the mythological titan of time, Cronos is an autonomous background sync daemon. Using genuine 3-way tree convergence (`merge_trees_3way`), POSIX advisory kernel locking, and crash-resilient write-ahead logging, Cronos continuously keeps parallel dimensions synchronized in the background.

### 9. DaftUniverse (`dft remote`, `dft push`, `dft pull`) — Self-Hosting Origin
Daft does not need GitHub or Git remotes. `dft remote add origin <url_or_path>` allows deploying to a decentralized **DaftUniverse** bare repository. Daft can self-host its own source code, stage objects, and push commits to `origin` natively!

---

## 🚀 Quickstart Guide

### 1. Installation
Ensure you have Rust (1.75+) installed:
```bash
# Clone the repository
git clone https://github.com/daft-vcs/daft.git
cd daft

# Build in release mode
cargo build --release

# Add to your PATH
export PATH="$PWD/target/release:$PATH"

# Verify installation
dft --version
```

### 2. Initialize a Repository & User
```bash
dft init my-project
cd my-project

dft config --set user.name "Alice Quantum"
dft config --set user.email "alice@daft.org"
```

### 3. Normal VCS Workflow (Familiar to Git Users)
```bash
echo "# Hello Daft" > README.md
dft add README.md
dft commit -m "feat: initial commit in mainline"
dft status
dft log
```

### 4. Unleashing the Multiverse: Parallel Dimensions
```bash
# Spawn an AI agent dimension instantly (Copy-on-Write)
dft dimension create agent-ai-1

# Enter the dimension
dft dimension enter agent-ai-1

# Claim your file territory
dft claim src/engine.rs

# Check radar for hot-zone collisions
dft radar --hot

# Code, commit, and predict conflicts
echo "fn quantum_core() {}" >> src/engine.rs
dft add src/engine.rs
dft commit -m "feat(ai): add quantum core"

# Predict conflicts with mainline
dft foresee agent-ai-1 mainline

# Converge back into mainline
dft dimension enter mainline
dft converge agent-ai-1 mainline

# Clean up
dft dimension destroy agent-ai-1
```

### 5. Launch the Real-Time Web GUI (`dft ui`)
```bash
# Launch the interactive Multiverse Dashboard
dft ui
```
Opens an interactive dashboard at `http://127.0.0.1:3333` displaying:
- Real-time 3D/2D Multiverse Timeline Canvas
- Dimension Manager (Create, Switch, Destroy)
- Radar & Territory Claims HUD
- Cronos Autonomous Sync Daemon controls
- Embedded interactive Daft Command Runner Terminal!

### 6. Push to DaftUniverse Remote
```bash
# Set up a DaftUniverse bare repository as origin
dft remote add origin /path/to/DaftUniverse

# Push the mainline dimension
dft push origin main
```

---

## 📊 Complete Command Reference

### Layer 1: Git-Equivalent Commands
| Command | Git Analog | Description |
|---|---|---|
| `dft init` | `git init` | Initialize a new Daft repository (`.dft/`) |
| `dft clone` | `git clone` | Clone a repository into a new directory |
| `dft config` | `git config` | Get or set configuration options |
| `dft add` | `git add` | Stage file contents into the binary index |
| `dft status` | `git status` | Show working tree and staging status |
| `dft commit` | `git commit` | Record staged changes to the repository |
| `dft reset` | `git reset` | Reset current HEAD to a specified state |
| `dft restore` | `git restore` | Restore working tree or staged files |
| `dft rm` / `dft mv` | `git rm` / `git mv` | Remove or rename tracked files |
| `dft stash` | `git stash` | Stash dirty changes away safely |
| `dft branch` | `git branch` | List, create, or delete branches |
| `dft checkout` / `dft switch` | `git checkout` / `switch` | Switch branches or restore files |
| `dft merge` | `git merge` | Join development histories (recursive 3-way) |
| `dft rebase` | `git rebase` | Reapply commits on top of another base |
| `dft cherry-pick` | `git cherry-pick` | Apply changes from existing commits |
| `dft tag` | `git tag` | Create, list, or verify tags |
| `dft log` | `git log` | Show commit history graph |
| `dft diff` | `git diff` | Myers $O(ND)$ diff between commits/worktree |
| `dft show` | `git show` | Inspect commit, tree, blob, or tag objects |
| `dft blame` | `git blame` | Line-by-line revision provenance |
| `dft grep` | `git grep` | Fast pattern search across tracked files |
| `dft bisect` | `git bisect` | Binary search for bug-introducing commits |
| `dft reflog` | `git reflog` | Manage append-only reflogs |
| `dft gc` / `dft fsck` | `git gc` / `git fsck` | Optimize repository / verify CAS integrity |

### Layer 2: Daft Multiverse Commands (No Git Equivalent)
| Command | Quantum Metaphor | Description |
|---|---|---|
| `dft dimension create <name>` | Parallel Universe | Create an isolated CoW workspace & branch (< 0.06s) |
| `dft dimension list` | Universe Roster | List all active dimensions and their statuses |
| `dft dimension enter <name>` | Spacetime Shift | Switch active terminal context to a dimension |
| `dft dimension destroy <name>` | Universe Pruning | Tear down a dimension, preserving shared CAS objects |
| `dft dimension fork <name>` | Live Forking | Fork another dimension including uncommitted edits |
| `dft snapshot` | Quantum Freeze | Immutable point-in-time capture across dimensions |
| `dft observe <dim> [path]` | Non-destructive Peek | Inspect another dimension without checking it out |
| `dft radar [--hot]` | Spacetime Radar | Real-time map of active files and collision hot zones |
| `dft foresee <dim1> <dim2>` | Quantum Foresight | Predict 3-way merge conflicts *before* merging |
| `dft entropy` | Divergence Metric | Calculate Shannon timeline divergence score $H(D_1, D_2)$ |
| `dft claim <path>` | Territory Stake | Claim exclusive advisory ownership of a file |
| `dft fence <path> [--hard]` | Boundary Barrier | Place an exclusionary barrier preventing edits |
| `dft territory` | Territory Map | Audit all claims, fences, and ownership stakes |
| `dft collapse [--into <dim>]` | Wavefunction Collapse | Collapse all parallel dimensions into mainline |
| `dft converge <dim1> <dim2>` | N-way Convergence | Merge multiple selected dimensions together |
| `dft cascade <dim> --to <target>` | Ripple Effect | Propagate changes down a chain of dimensions |
| `dft weave <dim1> <dim2>` | Causal Synthesis | Interleave commits in chronological vector order |
| `dft splice <dim> <range>` | Slice Transplant | Extract and graft specific commit ranges |
| `dft entangle <dim1> <dim2>` | Quantum Entanglement | Real-time live auto-propagation between dimensions |
| `dft cronos start/stop/status` | Time Titan | Background daemon for autonomous 3-way sync |
| `dft agent register/list/assign` | Agent Identity | Multi-agent registry, assignments, and mailboxes |
| `dft heartbeat` | Liveness Ping | Agent liveness tracking for swarm orchestration |
| `dft timeline` | Multiverse Graph | Render multiverse DAG in ASCII, SVG, DOT, or JSON |
| `dft ui [--port 3333]` | Multiverse HUD | Launch the interactive Real-Time Web GUI |
| `dft remote` / `dft push` / `dft pull` | DaftUniverse | Deploy and sync with DaftUniverse remotes |

---

## 🏗️ Architecture & Crate Topology

Daft is structured as an acyclic Cargo workspace of 8 modular, high-cohesion crates:

```
                  ┌──────────────────────┐
                  │      daft-cli        │ (Executable binary `dft` & Web GUI)
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

- **`daft-core`**: SHA-256 CAS object store, zlib compression, canonical binary index (`DIRC` v2), atomic ref manager, Myers diff, 3-way recursive merge, commit graph reachability.
- **`daft-dimension`**: OS-native Copy-on-Write engine (macOS APFS `clonefile`, Linux `FICLONE`, BoW hardlinks fallback), advisory per-dimension POSIX locks, isolated workspaces.
- **`daft-awareness`**: Cross-dimensional radar, hot-zone detection, predictive conflict simulation (`foresee`), Shannon entropy metric, territory claims and fences.
- **`daft-convergence`**: Multiverse convergence algorithms: wavefunction `collapse`, N-way `converge`, sequential `cascade`, chronological `weave`, and `splice`.
- **`daft-sync`**: Quantum entanglement engine (`entangle`) and `cronos` autonomous background sync daemon with write-ahead event logging (WAL).
- **`daft-agent`**: Multi-agent identity registry, workspace assignments, Maildir-style atomic mailboxes, broadcast channels, and liveness heartbeats.
- **`daft-cli`**: Unified CLI entry point (`dft`) with human-friendly and JSON outputs, and embedded real-time Multiverse Web GUI (`dft ui`).

---

## 📜 License & Open Source

Daft is licensed under the **GNU General Public License v2.0 (GPL v2)** — the same license that has preserved the freedom of Git and the Linux kernel for decades.

See the full [LICENSE](LICENSE) file for terms and conditions.
