<p align="center">
  <img src="docs/assets/draft_icon.svg" width="128" height="128" alt="Draft VCS Logo" />
</p>

# Draft (`dft`) — The Multiverse Version Control System

[![License: GPL v2](https://img.shields.io/badge/License-GPL%20v2-blue.svg)](LICENSE)
[![Language: Rust](https://img.shields.io/badge/Language-Rust%202021-orange.svg)](https://www.rust-lang.org/)
[![Status: Active Development](https://img.shields.io/badge/Status-Active%20Development-blue.svg)](#)
[![Performance: CoW Reflink](https://img.shields.io/badge/Cloning-0.06s%20%2F%2010%2C000%20files-cyan.svg)](#)
[![Architecture: Lock-Free CAS](https://img.shields.io/badge/Storage-SHA--256%20CAS-purple.svg)](#)
[![Web Platform: DraftMultiverse](https://img.shields.io/badge/GUI-DraftMultiverse-violet.svg)](#)

> *"In classical philosophy, Chronos (Χρόνος) forces all actions into a single linear sequence. In Draft, Kairos (Καιρός) lets parallel minds build concurrently until the opportune moment of convergence."*

---

![DraftMultiverse 3D Architecture](docs/assets/draft_multiverse_3d.jpg)

---

## ⚡ The Architectural Shift: Why Draft?

**Git was engineered in 2005 around a single checked-out working tree and linear branch switching.**  
**Draft (`dft`) was architected in 2026 for concurrent, multi-agent AI and human swarms.**

Modern software engineering faces concurrency bottlenecks at the version control layer:
1. **The Single Working-Tree Bottleneck**: Git limits a working repository to a single checked-out `HEAD` and working directory. Concurrently running multiple AI agents or collaborating developers requires brittle `git worktree` setups or multi-gigabyte repository clones that duplicate disk and memory.
2. **Blind Collision**: Agents editing files concurrently have zero cross-branch awareness of each other until a final merge or rebase triggers complex conflict triage.
3. **Reactive Merge Friction**: Traditional VCS operates reactively. Conflicts are discovered *after* code changes are completed, requiring manual 3-way triage.

### What Draft (`dft`) Delivers:
- 🌌 **Parallel Dimensions**: Spawn isolated, Copy-on-Write (CoW) workspaces in **0.06 seconds** sharing a lock-free, content-addressable storage (CAS) engine with zero duplicate disk blocks.
- 📡 **Cross-Dimensional Radar**: Real-time sensing of file access and write operations across all concurrent dimensions.
- 🔮 **Predictive Merge Foresight (`dft foresee`)**: In-memory simulation of 3-way merges *before* branch integration occurs.
- 🔒 **Territory Leasing & Boundary Fences**: Advisory path leases (`dft claim`) and hard exclusionary barriers (`dft fence`) to prevent accidental agent collisions.
- 🔗 **Continuous Entanglement (`dft entangle`)**: Bi-directional live auto-synchronization of specific files across parallel dimensions.
- ⏰ **Autonomous Sync Daemon (`dft cronos`)**: Background continuous 3-way tree convergence engine.
- 🌊 **Multi-Branch Collapse (`dft collapse`)**: Reconcile and merge all active parallel dimensions back into mainline in one atomic command.
- 🖥️ **Self-Hosted Developer Web Platform**: Web interface (`dft ui`) featuring live file tree browsing, syntax-highlighted blob viewing, Myers diff inspections, interactive Spacetime DAG visualization, and an embedded terminal console.
- 🪐 **Decentralized Remote Sync (`DraftUniverse`)**: Native remote push/pull protocol without dependence on third-party git hosts.

---

## 📖 Backstory & Philosophy: From Chronos to Kairos

### The Swarm Concurrency Bottleneck
In 2026, autonomous AI engineering agents transitioned from novelty to necessity. Engineering teams attempted to orchestrate teams of specialized AI agents working concurrently on single repositories:
- **Agent Alpha**: Database engine & migration rewrite.
- **Agent Beta**: HTTP REST & WebSocket API layer.
- **Agent Gamma**: Web frontend UI components.
- **Agent Delta**: Comprehensive integration test suites.
- **Agent Epsilon**: Memory profiling and algorithmic optimizations.

The moment multiple agents executed in parallel, **traditional single-worktree Git workflows hit severe friction**:
- Agents switching branches trampled each other's unstaged files.
- Multiple local clones duplicated tens of thousands of build artifacts, causing disk thrashing and operating system resource exhaustion.
- At the end of every sprint, merging diverged branches created painful manual triage.

In ancient thought, the Greeks distinguished two forms of time: **Chronos (Χρόνος)**, the quantitative, sequential progression of linear events, and **Kairos (Καιρός)**, the qualitative, opportune moment of harmony and action. Git bound version control strictly to *Chronos*—a single linear arrow where only one branch could be active at a time. **Draft was built to bring Kairos (Καιρός) and Metis (Μῆτις, foresight) to version control**, enabling parallel development dimensions to evolve independently and converge into **Harmonia (Ἁρμονία)** without collision.

### Sincere Tribute to Linus Torvalds
Draft stands on the shoulders of Linus Torvalds. In 2005, Linus revolutionized software engineering by creating Git in two weeks. His insights—content-addressable object storage, cryptographically verified acyclic commit graphs, and immutable blob snapshots—remain foundational.

We named this system **Draft (`draft` / `dft`)** because every parallel dimension represents an agile, working *draft* of your project. Where Git forces developers to commit every experiment to a single rigid working-tree sequence, Draft allows agents and developers to iterate on preliminary versions concurrently before bringing them together. Draft retains Linus's CAS principles while unlocking **parallel dimension branching**.

---

## 🔬 Deep Technical Architecture

```
                               ┌───────────────────────────┐
                               │   DraftMultiverse (GUI)    │ (Web Platform on :3333)
                               └─────────────┬─────────────┘
                                             │
                               ┌─────────────▼─────────────┐
                               │   daft-cli (`dft` binary) │
                               └─────────────┬─────────────┘
                ┌────────────────┬───────────┴───────────┬────────────────┐
                │                │                       │                │
                ▼                ▼                       ▼                ▼
       ┌─────────────────┐ ┌───────────┐         ┌──────────────┐ ┌───────────────┐
       │ daft-convergence│ │ daft-sync │         │  daft-agent  │ │ daft-awareness│
       └────────┬────────┘ └─────┬─────┘         └──────┬───────┘ └───────┬───────┘
                │                │                      │                 │
                └───────────┬────┴──────────────────────┘                 │
                            ▼                                             │
                  ┌──────────────────┐                                    │
                  │  daft-dimension  │◄───────────────────────────────────┘
                  └─────────┬────────┘
                            ▼
                  ┌──────────────────┐
                  │    daft-core     │ (CAS, Index, Myers Diff, Refs, Engine)
                  └──────────────────┘
```

### 1. Content-Addressable Storage (CAS) Engine
- **Hashing**: Cryptographic SHA-256 producing 32-byte direct digest identifiers (`ObjectId = [u8; 32]`, formatted as 64-character hex strings).
- **Object Framing**: Canonical git-compatible envelope framing:
  $$\text{Payload} = \langle\text{type}\rangle + \texttt{" "} + \langle\text{length}\rangle + \texttt{"\\0"} + \langle\text{data}\rangle$$
- **Compression**: Deflate/zlib streaming compression (level 6) stored in loose fanout directories: `.dft/objects/xx/yyyy...`.
- **Zero-Copy I/O**: High-throughput file inspection uses read-only memory-mapped I/O (`memmap2`) with automatic fallback to streaming buffers.
- **Primitive Objects**:
  - `Blob`: Raw byte payloads with null-byte detection for binary differentiation.
  - `Tree`: Lexicographically sorted vectors of file mode, object name, and target SHA-256 ID.
  - `Commit`: Immutable snapshot pointer containing tree hash, parent hashes, committer/author signature tuples (name, email, epoch timestamp), and commit message.
  - `Tag`: Cryptographic pointer linking an arbitrary object to an annotated signature.

### 2. Copy-on-Write (CoW) Workspace Engine
- **Sub-Second Forking**: When a parallel dimension is spawned, Draft avoids byte copying by leveraging kernel-level reflink primitives:
  - **macOS**: `clonefile()` / `fclonefileat()` via Apple File System (APFS).
  - **Linux**: `ioctl(FICLONE)` / `ioctl(FICLONERANGE)` on Btrfs, XFS, and ZFS.
  - **Fallback**: Hard-link trees with atomic break-on-write mechanisms for standard ext4.
- **Storage Metrics**: Spawning an isolated 10,000-file dimension workspace completes in **0.06 seconds** and consumes **0 KB** of additional disk blocks until modified.
- **Concurrency Isolation**: Each dimension maintains its own independent working directory, index (`.dft/dimensions/<name>/index`), and branch ref pointer, sharing the root CAS object pool without lock contention.

### 3. Differential Analysis & 3-Way Tree Reconciliation
- **Myers Algorithm**: Complete implementation of Eugene Myers' $O(ND)$ greedy difference algorithm, generating the minimal Shortest Edit Script (SES) between sequences.
- **Tree Merge Engine (`merge_trees_3way`)**:
  - Graph traversal locates the Lowest Common Ancestor (LCA) commit.
  - Generates 3-way disjoint hunk sets: *Base* ($O$), *Ours* ($A$), and *Theirs* ($B$).
  - Clean non-overlapping hunks are auto-resolved into the target index.
  - Overlapping edits automatically emit diff3 standard conflict markers:
    ```
    <<<<<<< ours (dimension-a)
    modified code from dimension A
    ||||||| base
    original base code
    =======
    concurrent code from dimension B
    >>>>>>> theirs (dimension-b)
    ```

### 4. Timeline Divergence Metric ($H$)
Draft computes a quantitative divergence score $H(D_A, D_B) \in [0.0, 1.0]$ between any two dimensions across four weighted components:
$$H(D_A, D_B) = 0.50 \cdot C_{\text{commits}} + 0.25 \cdot F_{\text{files}} + 0.15 \cdot T_{\text{trees}} + 0.10 \cdot L_{\text{lines}}$$

- **$C_{\text{commits}}$**: Distance in the commit DAG from their Lowest Common Ancestor (LCA).
- **$F_{\text{files}}$**: Ratio of concurrently modified files over total tracked files.
- **$T_{\text{trees}}$**: Jaccard distance between the dimension tree snapshots: $1 - \frac{|T_A \cap T_B|}{|T_A \cup T_B|}$.
- **$L_{\text{lines}}$**: Overlapping hunk modifications in shared files.

#### Divergence Risk Tiers:
- $H = 0.0$: Identical state (clean fast-forward possible).
- $0.0 < H \le 0.15$: Negligible divergence (clean auto-merge expected; minimal file overlap).
- $0.15 < H \le 0.35$: Low divergence (auto-merge expected).
- $0.35 < H \le 0.65$: Moderate divergence (cross-file edits require awareness).
- $H > 0.65$: High divergence (elevated collision risk; pre-conflict warnings triggered).

---

## 🛠️ Using Draft with Standard Open-Source Workflows

### 1. Installation

#### A. Homebrew (macOS & Linux)
Install directly from our official tap:
```bash
brew install draft-vcs/draft/dft
```

#### B. Standalone Shell Script (macOS & Linux)
Zero-dependency instant installer for workstations and CI/CD runners:
```bash
curl -fsSL https://draftuniverse.org/install.sh | sh
```

#### C. Cargo (Rust Crates.io)
```bash
cargo install draft-vcs
```

#### D. Building from Source
```bash
# Clone the repository
dft clone /Users/pathomphongphiphatsuriyawong/Workspace/DraftUniverse
# or: git clone https://github.com/draft-vcs/draft.git
cd draft

# Build optimized release binaries
cargo build --release

# Install globally into your cargo bin path
cargo install --path crates/daft-cli

# Verify dft command
dft --version
```

---

### 2. Quickstart: Standard Version Control

Draft offers a familiar interface for standard version control commands:

```bash
# Initialize a new Draft repository
dft init my-project
cd my-project

# Configure your developer identity
dft config --set user.name "Your Name"
dft config --set user.email "you@example.org"

# Stage and commit files
echo "fn main() { println!(\"Hello World\"); }" > main.rs
dft add main.rs
dft commit -m "feat: initial commit"

# Inspect status, history, and diffs
dft status
dft log
dft diff
```

---

### 3. Parallel Development: Multi-Agent & Multi-Feature Workflows

Run multiple parallel feature branches simultaneously in the same repository:

```bash
# 1. Create parallel dimensions for concurrent work
dft dimension create feature-auth
dft dimension create feature-db

# 2. List all active dimensions
dft dimension list
# Output:
#   feature-auth  [clean]
#   feature-db    [clean]
# * mainline      [clean]

# 3. Enter a dimension and work in isolation
dft dimension enter feature-auth

# 4. Acquire an advisory lease on files
dft claim src/auth.rs

# 5. Check real-time radar for concurrent hot zones
dft radar --hot

# 6. Check for potential merge conflicts in memory before merging
dft foresee feature-auth mainline

# 7. Converge feature branch back into mainline
dft dimension enter mainline
dft converge feature-auth mainline
```

---

### 4. Seamless Hybrid Workflow: Develop in Parallel with Draft → Push to Git

You do not need to replace your organization's Git infrastructure, GitHub Pull Request workflows, or existing CI/CD pipelines to harness the concurrency power of Draft. You can use **Draft as a local concurrency acceleration layer** on top of any existing Git repository:

> **"Develop in the Multiverse with Draft, Ship to the World with Git."**

```
                     ┌────────────────────────────────────────────────────────┐
                     │              Existing Git Repository (.git)            │
                     │          (GitHub / GitLab / Bitbucket / Upstream)      │
                     └───────────────────────────▲────────────────────────────┘
                                                 │
                                 git push origin main / dft export
                                                 │
                               ┌─────────────────┴──────────────────┐
                               │       Draft Mainline Working Tree   │
                               │        (Converged, Tested, Clean)  │
                               └─────────────────▲──────────────────┘
                                                 │
                                   dft collapse / dft converge
                                                 │
                   ┌─────────────────────────────┼─────────────────────────────┐
                   │                             │                             │
        ┌──────────▼───────────┐      ┌──────────▼───────────┐      ┌──────────▼───────────┐
        │  Dimension: dim-auth │      │  Dimension: dim-api  │      │  Dimension: dim-ui   │
        │  (Agent / Dev 1)     │      │  (Agent / Dev 2)     │      │  (Agent / Dev 3)     │
        │  • CoW Workspace     │      │  • CoW Workspace     │      │  • CoW Workspace     │
        │  • Exclusive Claim   │      │  • Exclusive Claim   │      │  • Exclusive Claim   │
        │  • Hot-Zone Radar    │      │  • Hot-Zone Radar    │      │  • Hot-Zone Radar    │
        └──────────────────────┘      └──────────────────────┘      └──────────────────────┘
                   ▲                             ▲                             ▲
                   └─────────────────────────────┼─────────────────────────────┘
                                                 │
                                Shared Content-Addressable Storage (CAS)
                                      Zero Duplicate Disk Blocks
```

#### Why Combine Draft with Git?

| Concurrency Dimension | Traditional Git / Worktrees | Draft Multiverse Swarm |
|:---|:---|:---|
| **Workspace Creation Speed** | 2.5s – 5.0s (full directory clone / worktree setup) | **0.06s** (instant APFS/Btrfs CoW reflink) |
| **Disk Footprint** | Multiplies linearly per full clone or separate working tree (GBs) | **0 KB** additional blocks until modified (CoW) |
| **Branch Switching Overhead** | Must stash, commit, or clean working tree | Zero overhead: each dimension is an isolated workspace |
| **Multi-Agent Awareness** | Blind: agents overwrite shared files | **Real-Time Radar (`dft radar`)** & Territory Claims |
| **Merge Conflict Triage** | Reactive: conflicts discovered after work | **Predictive: `dft foresee`** flags collisions in advance |
| **Upstream Compatibility** | Native | **Fully Compatible**: push standard Git commits to GitHub/GitLab |

---

#### Step-by-Step Guide: The Parallel Draft → Git Push Flow

##### Step 1: Enable Draft in Your Existing Git Repository
Navigate to your current project. Draft lives harmoniously alongside `.git/` without altering your Git status:
```bash
cd my-existing-git-repo

# Initialize Draft VCS engine (.dft/)
dft init

# Keep Draft internal state untracked in Git
echo ".dft/" >> .gitignore
git add .gitignore && git commit -m "chore: enable Draft parallel multiverse engine"
```

##### Step 2: Spawn Parallel Dimensions for Features or AI Agents
Instead of fighting branch switches or juggling multiple working tree clones, spawn parallel dimensions in milliseconds:
```bash
# Instant CoW branches for concurrent tasks
dft dimension create feat-auth
dft dimension create feat-billing
dft dimension create feat-docs

# Verify your multiverse fleet
dft dimension list
#   feat-auth     [clean]
#   feat-billing  [clean]
#   feat-docs     [clean]
# * mainline      [clean]
```

##### Step 3: Work Concurrently in Parallel Dimensions
Each dimension operates in complete filesystem isolation under `.dft/dimensions/<name>/workspace`:
- **Developer / Agent Alpha** works on Authentication:
  ```bash
  cd .dft/dimensions/feat-auth/workspace
  dft claim src/auth.rs                    # Claim advisory territory
  # Edit, test, and commit locally within dimension
  dft add src/auth.rs
  dft commit -m "feat(auth): implement JWT token verification"
  ```
- **Developer / Agent Beta** works on Billing concurrently:
  ```bash
  cd .dft/dimensions/feat-billing/workspace
  dft claim src/billing.rs                 # Claim advisory territory
  # Edit, test, and commit locally within dimension
  dft add src/billing.rs
  dft commit -m "feat(billing): add Stripe webhook handler"
  ```

##### Step 4: Proactive Collision Check with Predictive Merge Foresight
Before bringing changes together, verify that no conflicting hunks exist:
```bash
# In-memory simulation of 3-way reconciliation without touching working code
dft foresee feat-auth mainline
# Output:
#   [FORESEE] Simulated 3-way merge between 'feat-auth' and 'mainline'
#   [FORESEE] Clean auto-merge predicted: 0 conflicts detected.
#   [FORESEE] Divergence Metric: H = 0.04 (minimal divergence)
```

##### Step 5: Converge & Collapse into Mainline
When parallel work is finished and verified, collapse all dimensions or converge specific features back into `mainline`:
```bash
# Return to the root workspace (mainline)
dft dimension enter mainline

# Converge features into mainline
dft converge feat-auth mainline
dft converge feat-billing mainline
dft converge feat-docs mainline

# Or collapse all active dimensions into mainline in a single command:
dft collapse
```

##### Step 6: Seamlessly Push to Git / GitHub / GitLab
Your root working tree now contains all converged, tested, and conflict-free changes. Git sees standard working tree modifications ready for upstream:
```bash
# Inspect status using standard Git
git status

# Stage the converged changes
git add src/auth.rs src/billing.rs docs/

# Record as standard Git commits
git commit -m "feat: integrate auth, billing, and docs from parallel swarm"

# Push straight to GitHub, GitLab, or your corporate Git remote!
git push origin main
```

##### Optional: Direct Git Bridge & Import/Export
You can also import existing Git history or export Draft dimensions:
```bash
# Import an existing Git repository into Draft
dft import git

# Export a specific dimension to standard Git format
dft export git --dimension mainline

# Verify compatibility bridge status
dft compat git-bridge
```

> **Tip**: Because Draft converges changes directly into standard working tree files on mainline, you can push integrated commits upstream to GitHub or GitLab without altering your team's existing review processes or CI/CD pipelines.

---

### 5. Orchestrating AI Agents with `SKILL.md`

Draft ships with a native, standardized agent skill definition located at [**`SKILL.md`**](SKILL.md). This file equips LLM coding agents (such as Google Antigravity, Claude Code, Cursor, GitHub Copilot, and custom autonomous swarms) with the exact operational protocol, commands, and safety invariants needed to collaborate concurrently in a Draft repository.

#### Why AI Agents Need `SKILL.md`
Standard coding agents assume Git's single-working-tree model: when multiple agents run concurrently, they switch branches under each other, overwrite uncommitted files, and trigger race conditions. 

By reading [`SKILL.md`](SKILL.md), agents understand how to:
1. **Never work directly in `mainline`** during active feature development.
2. **Spawn instant isolated CoW dimensions** (`dft dimension create`) with zero disk bloat.
3. **Lease file paths** using advisory claims (`dft claim`) and respect boundary fences (`dft fence`).
4. **Sense concurrent edits** across the agent fleet using real-time radar (`dft radar --hot`).
5. **Communicate peer-to-peer** with other agents via asynchronous mailboxes (`dft agent send` / `dft agent read`).
6. **Pre-test 3-way merge viability** (`dft foresee`) before converging into mainline.

---

#### How to Equip Your AI Agents with `SKILL.md`

##### A. In Antigravity / Agentic IDEs
Draft's `SKILL.md` adheres to the open-standard agent skill manifest format with YAML frontmatter:
```markdown
---
name: draft-vcs
description: Operational guide and multi-agent protocol for Draft ('dft')...
---
```
- The IDE automatically discovers [`SKILL.md`](SKILL.md) in the workspace root.
- Agents automatically adopt the Draft multi-agent lifecycle when assigned coding tasks.

##### B. In Claude Code, Cursor, or Terminal Agent Prompts
Feed [`SKILL.md`](SKILL.md) directly into your agent's context or system prompt:
```bash
# Example invocation with Claude Code or terminal LLMs:
claude "Read SKILL.md and implement the JWT authentication module following the 8-step Draft agent protocol."
```
Or in Cursor / Copilot Chat:
> *"@SKILL.md Follow the Draft agent protocol: spawn an isolated dimension, claim `src/auth.rs`, implement the feature, run `dft foresee`, and converge back to mainline."*

##### C. Programmatic Multi-Agent Swarms (Python / TypeScript / Rust)
When orchestrating swarms with frameworks like CrewAI, LangGraph, or AutoGen, provide `SKILL.md` as the system instruction or tool reference:
```python
# Pass SKILL.md contents into the agent system prompt
with open("SKILL.md") as f:
    draft_skill_prompt = f.read()

agent_worker = Agent(
    role="Backend Engine Developer",
    system_prompt=f"You operate in a Draft VCS repository. Follow this protocol:\n{draft_skill_prompt}"
)
```

---

#### The 8-Step Autonomous Agent Lifecycle

Every agent interacting with Draft follows a structured lifecycle:

```
┌────────────────────────────────────────────────────────┐
│ 1. Register:   dft agent register <id> --type ai       │
│ 2. Dimension:  dft dimension create <id>/<task_name>   │
│ 3. Radar:      dft radar --hot                         │
│ 4. Claim:      dft claim <file_path>                   │
│ 5. Code & Save:dft add . && dft commit -m "feat: ..."  │
│ 6. Foresee:    dft foresee <dimension> mainline        │
│ 7. Converge:   dft converge <dimension> mainline       │
│ 8. Yield:      dft yield <file_path>                   │
└────────────────────────────────────────────────────────┘
```

1. **Register**: The agent registers its worker identity and sends heartbeat pings:
   ```bash
   dft agent register agent-coder --type ai
   dft heartbeat agent-coder
   ```
2. **Dimension Creation**: Spawns an isolated CoW parallel workspace in 0.06 seconds:
   ```bash
   dft dimension create agent-coder/auth-feature
   dft dimension enter agent-coder/auth-feature
   ```
3. **Radar Inspection**: Checks whether other dimensions or developers are modifying overlapping paths:
   ```bash
   dft radar --hot
   ```
4. **Territory Lease**: Acquires an advisory lock on target files to signal ownership:
   ```bash
   dft claim src/auth.rs
   ```
5. **Implement & Commit**: Makes changes and records isolated commits within its private dimension without affecting other workers:
   ```bash
   dft add src/auth.rs
   dft commit -m "feat(auth): add bearer token verification"
   ```
6. **Predictive Merge Foresight**: Runs an in-memory 3-way simulation to verify conflict-free integration before convergence:
   ```bash
   dft foresee agent-coder/auth-feature mainline
   ```
7. **Convergence**: Reconciles the dimension back into `mainline`:
   ```bash
   dft dimension enter mainline
   dft converge agent-coder/auth-feature mainline
   ```
8. **Yield & Cleanup**: Releases file claims and tears down the ephemeral dimension:
   ```bash
   dft yield src/auth.rs
   dft dimension destroy agent-coder/auth-feature
   ```

---

#### Peer-to-Peer Agent Mailbox Protocol
Agents can coordinate directly without third-party message brokers using Draft's built-in file-backed mailboxes:
```bash
# Agent Alpha notifies Agent Beta about updated API types
dft agent send agent-beta "New user payload types committed to shared/types/user.rs"

# Agent Beta checks its incoming mailbox
dft agent read agent-beta
```

#### Real-World Verification: AetherDB Swarm
This exact protocol was verified in [`demos/agent_team_miniproject`](demos/agent_team_miniproject), where 3 autonomous AI agents concurrently built a persistent Key-Value database (WAL storage, API parser, and REPL CLI) in parallel dimensions and converged cleanly into mainline without merge conflicts.

---

### 6. Automated CI/CD & AI Agent Pipelines

Every Draft command supports structured `--json` output for automated tooling, CI runners (GitHub Actions, GitLab CI), and AI coding assistants:

```bash
# Get machine-readable status
dft status --json

# Run conflict prediction in CI
dft foresee dimension-a mainline --json

# Query radar telemetry in scripts
dft radar --json
```

#### Example GitHub Actions Workflow Step:
```yaml
- name: Verify Cross-Branch Convergence
  run: |
    dft dimension enter mainline
    dft foresee pr-branch mainline --json > conflict_report.json
    if grep -q '"has_conflicts": true' conflict_report.json; then
      echo "Convergence conflict detected before merge!"
      exit 1
    fi
```

---

### 7. Self-Hosting: DraftMultiverse & DraftUniverse

Draft includes a complete, self-hosted web platform (**DraftMultiverse**) and headless remote server (**DraftUniverse**):

#### Launching the Web Platform
```bash
# Launch the DraftMultiverse Web GUI
dft ui --port 3333
```
Open **[http://127.0.0.1:3333](http://127.0.0.1:3333)** to access:
- **`<> Code`**: Interactive file tree, breadcrumbs, text blob inspector with line numbers, and markdown preview.
- **`⏱️ Commits`**: Commit history log with integrated Myers diff viewer (additions `+` green, deletions `-` red).
- **`🌌 Multiverse`**: Spacetime DAG canvas mapping concurrent parallel timelines.
- **`🔀 Convergence`**: PR-style dashboard for dry-run conflict checks, multi-branch collapse, and dimension convergence.
- **`📡 Radar & Territory`**: Heatmap HUD for hot zones, timeline divergence score, and file claims.
- **`🤖 Agent Fleet`**: AI and human agent roster with live heartbeats and direct mailboxes.
- **`⏰ Cronos Daemon`**: Background sync controls and continuous event log streamer.
- **`⚡ Terminal`**: In-browser command prompt supporting execution of any `dft` command.

#### Setting up a Self-Hosted Remote Server (`DraftUniverse`)
```bash
# 1. Initialize a bare server repository on your server or local disk
mkdir -p /path/to/DraftUniverse
dft init --bare /path/to/DraftUniverse

# 2. Add as remote origin in your working project
dft remote add origin /path/to/DraftUniverse

# 3. Deploy and synchronize all dimensions
dft push origin mainline
dft push origin agent-quantum
```

---

## 📋 Comprehensive Command Matrix

### Layer 1: Core VCS Commands (Git Equivalent)
| Command | Git Analog | Description |
|:---|:---|:---|
| `dft init` | `git init` | Initialize a new repository (`.dft/`) |
| `dft clone` | `git clone` | Clone a repository into a new workspace |
| `dft config` | `git config` | Query or modify configuration settings |
| `dft add` | `git add` | Stage file content changes into binary index |
| `dft status` | `git status` | Reconcile HEAD tree, Index, and Working Tree |
| `dft commit` | `git commit` | Record staged changes as an immutable commit |
| `dft reset` | `git reset` | Reset current HEAD pointer (soft, mixed, hard) |
| `dft restore` | `git restore` | Restore working directory or staged index files |
| `dft rm` / `dft mv` | `git rm` / `git mv` | Delete or move/rename tracked files |
| `dft stash` | `git stash` | Safely shelve dirty uncommitted changes |
| `dft branch` | `git branch` | Create, list, or delete branches |
| `dft checkout` / `dft switch` | `git checkout` / `git switch` | Switch active branch or restore file revisions |
| `dft merge` | `git merge` | Join development histories via 3-way tree merge |
| `dft rebase` | `git rebase` | Replay commits onto a new base tip |
| `dft cherry-pick` | `git cherry-pick` | Apply specific commit diffs to the current branch |
| `dft tag` | `git tag` | Create or verify lightweight/annotated tags |
| `dft log` | `git log` | Traverse and format commit DAG history |
| `dft diff` | `git diff` | Myers differential analysis across trees/blobs |
| `dft show` | `git show` | Pretty-print commits, trees, blobs, or tags |
| `dft blame` | `git blame` | Line-by-line commit authorship provenance |
| `dft grep` | `git grep` | Fast regex pattern matching across tracked files |
| `dft bisect` | `git bisect` | Binary search regression finder |
| `dft reflog` | `git reflog` | Audit append-only reference transaction history |
| `dft gc` / `dft fsck` | `git gc` / `git fsck` | Object pruning, repacking, and integrity verification |

### Layer 2: Draft Multiverse Commands (Exclusive Capabilities)
| Command | Subsystem | Description |
|:---|:---|:---|
| `dft dimension create <name>` | Parallel Workspace | Instant CoW workspace creation (< 0.06s) |
| `dft dimension list` | Parallel Workspace | List active parallel dimensions and sync state |
| `dft dimension enter <name>` | Parallel Workspace | Shift active shell context to another dimension |
| `dft dimension destroy <name>`| Parallel Workspace | Tear down dimension while preserving CAS objects |
| `dft snapshot` | Workspace Snapshot | Capture immutable point-in-time state across dimensions |
| `dft observe <dim> [path]` | Non-destructive Inspection | Read another dimension's state without checking it out |
| `dft radar [--hot]` | Real-Time Telemetry | Detect concurrently edited files and collision hot zones |
| `dft foresee <dim1> <dim2>` | Predictive Merge | Simulate 3-way tree reconciliation in memory before merging |
| `dft entropy` | Divergence Metric | Compute weighted divergence score $H(D_1, D_2) \in [0.0, 1.0]$ |
| `dft claim <path>` | Territory Protocol | Acquire advisory file lease with TTL |
| `dft yield <path>` | Territory Protocol | Release an active territory claim |
| `dft fence <path>` | Territory Protocol | Erect hard modification barrier across all dimensions |
| `dft collapse` | Convergence Engine | Reconcile all active parallel dimensions back into mainline |
| `dft converge <d1> <d2>` | Convergence Engine | Multi-head 3-way reconciliation of specific dimensions |
| `dft cascade <d1> --to <d2>`| Convergence Engine | Propagate updates down a chain of dependent dimensions |
| `dft weave <d1> <d2>` | Convergence Engine | Interleave commits in causal-chronological vector order |
| `dft splice <dim> <range>` | Convergence Engine | Transplant a commit slice between dimensions |
| `dft entangle <d1> <d2>` | Entanglement Engine | Continuous bi-directional file auto-synchronization rule |
| `dft cronos start/stop/log` | Background Sync | Autonomous background 3-way convergence daemon |
| `dft agent register/list` | Multi-Agent Swarm | Register AI/human worker identities and roles |
| `dft agent send/read` | Multi-Agent Swarm | Asynchronous Maildir-style agent communication queue |
| `dft heartbeat` | Multi-Agent Swarm | Agent liveness monitor to prevent abandoned locks |
| `dft timeline` | DAG Visualizer | Export spacetime DAG in ASCII, SVG, DOT, or JSON |
| `dft ui [--port 3333]` | Web Platform | Launch the DraftMultiverse self-hosted web platform |
| `dft remote / push / pull` | Decentralized Sync | Synchronize with headless DraftUniverse remotes |

---

## 🤝 Contributing & Community

Draft is an open-source project welcoming developers, systems programmers, and AI researchers:

### Development Workflow
```bash
# Run the complete test suite across all crates
cargo test --workspace

# Run targeted stress and adversarial tests
cargo test -p daft-core
cargo test -p daft-awareness

# Code formatting & linting
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
```

---

## 📜 License

Draft is licensed under the **GNU General Public License v2.0 (GPL v2)** — the same license that has preserved the freedom of Git and the Linux kernel for decades. See [LICENSE](LICENSE) for details.
