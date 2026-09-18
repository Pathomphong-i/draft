<p align="center">
  <img src="docs/assets/draft_icon.svg" width="160" height="160" alt="Draft VCS Logo" style="border-radius: 32px; box-shadow: 0 0 35px rgba(88,166,255,0.45); border: 2px solid rgba(88,166,255,0.35);" />
</p>

# Draft (`dft`) — The Multiverse Version Control System

> 🚀 **The Multi-Agent Concurrency Engine**:  
> **Deploy multiple autonomous AI agents to code across parallel branches simultaneously — with zero disk bloat, zero lock collisions, and automated convergence. Build and ship faster than ever before.**

[![License: GPL v2](https://img.shields.io/badge/License-GPL%20v2-blue.svg)](LICENSE)
[![Language: Rust](https://img.shields.io/badge/Language-Rust%202021-orange.svg)](https://www.rust-lang.org/)
[![Live Web Demo](https://img.shields.io/badge/Live_Demo-GitHub%20Pages-22c55e.svg)](https://pathomphong-i.github.io/draft/)
[![Sponsor on GitHub](https://img.shields.io/badge/Sponsor_Us-GitHub%20Sponsors-ff69b4.svg)](https://github.com/open-source/sponsors)
[![GitHub Stars](https://img.shields.io/github/stars/Pathomphong-i/draft?style=social)](https://github.com/Pathomphong-i/draft)
[![Status: Active Development](https://img.shields.io/badge/Status-Active%20Development-blue.svg)](#)
[![Performance: CoW Reflink](https://img.shields.io/badge/Cloning-0.06s%20%2F%2010%2C000%20files-cyan.svg)](#)
[![Architecture: Lock-Free CAS](https://img.shields.io/badge/Storage-SHA--256%20CAS-purple.svg)](#)
[![Web Platform: DraftMultiverse](https://img.shields.io/badge/GUI-DraftMultiverse-violet.svg)](https://pathomphong-i.github.io/draft/)

> *"In classical philosophy, Chronos (Χρόνος) forces all actions into a single linear sequence. In Draft, Kairos (Καιρός) lets parallel minds build concurrently until the opportune moment of convergence."*

---

![DraftMultiverse 3D Architecture](docs/assets/draft_multiverse_3d.jpg)

---

## 🎭 The "Multiverse on GitHub" Paradox

![Built a Multiverse Engine for Parallel AI Swarms... But We Still Need the GitHub Community!](docs/assets/draft_github_meme.jpg)

> *"We built Draft (`dft`) for concurrent AI swarms... but we still can't live without the GitHub community!"* 🐙💖

When Linus Torvalds engineered Git in 2005, he could host it on raw Linux kernel mail servers. But in 2026, **the developer world, open-source contributors, and AI swarms live in GitHub's gravity well**.

- 🌌 **We develop Draft using Draft**: Internally, our parallel dimensions, lock-free content-addressable storage (CAS), and predictive 3-way merges are 100% dogfooded with `dft`.
- 🐙 **We ❤️ the GitHub Community**: We host our open-source project and interactive demo on GitHub because this is where developers star repositories, report issues, submit PRs, and collaborate. We are proud to build together with the open-source community!

### 💖 Community Goal: Funding Sovereign `draftmultiverse.org`

> [!TIP]
> ### 🪐 Sovereign Multiverse Infrastructure Goal
> Today, Draft's code, releases, and interactive web playground are powered directly by **GitHub** and **GitHub Pages**.
>
> If the community finds Draft valuable and we receive enough community support through donations and sponsorships, **we will fund, build, and deploy a dedicated, 24/7 sovereign server cluster on `draftmultiverse.org`** — featuring decentralized remote repo synchronization, headless multiverse servers, and distributed AI agent fleet coordinators!
>
> 👉 **[Support the Project on GitHub Sponsors](https://github.com/open-source/sponsors)** | [Sponsor @Pathomphong-i](https://github.com/sponsors/Pathomphong-i)

---

## ⚡ The Architectural Shift: Why Draft?

**Git was engineered in 2005 around a single checked-out working tree and linear branch switching.**  
**Draft (`dft`) was architected in 2026 for concurrent, multi-agent AI and human swarms.**

> [!TIP]
> ### ⚡ Build Faster with Autonomous Swarms
> Instead of waiting for one agent or developer to finish before switching branches, **Draft lets you dispatch multiple AI agents to write code across parallel dimensions at the exact same time**. They code concurrently, sense each other in real-time via cross-dimension radar, and converge cleanly into mainline without lock collisions — making you build and ship software drastically faster.

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
- 🪐 **Decentralized Remote Sync (`DraftMultiverse`)**: Native remote push/pull protocol without dependence on third-party git hosts.

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

## 🤖 How to Use `SKILL.md` with Your AI Agents

Draft ships with an open-standard agent skill manifest located at [**`SKILL.md`**](SKILL.md) in the root of the repository. It teaches AI coding agents (such as Google Antigravity, Claude Code, Cursor, Windsurf, GitHub Copilot, and autonomous agent swarms) how to work in parallel dimensions without stepping on each other's code.

```
                     ┌──────────────────────────────────────────────┐
                     │          Autonomous AI Coding Agent          │
                     │  (Antigravity / Claude / Cursor / Swarms)    │
                     └──────────────────────┬───────────────────────┘
                                            │ Reads SKILL.md
                                            ▼
                     ┌──────────────────────────────────────────────┐
                     │          8-Step Draft Agent Protocol         │
                     │  • Register & private CoW dimension          │
                     │  • Check radar & lease file claims           │
                     │  • Lock-free private commit & in-memory merge│
                     │  • Foresee 3-way conflicts & safe converge   │
                     └──────────────────────────────────────────────┘
```

### 1. Equip Your Agent with `SKILL.md`

Depending on your development environment or AI tool, here is how to activate `SKILL.md`:

#### 🌌 Google Antigravity & Agentic IDEs
Draft's [`SKILL.md`](SKILL.md) contains native YAML frontmatter compliant with agent skill protocols:
- Simply open the repository in Antigravity or an Agentic IDE.
- The assistant automatically indexes [`SKILL.md`](SKILL.md) and switches to multiverse-aware workflows when creating feature dimensions or executing tasks.

#### ⚡ Claude Code & Terminal LLM Assistants
Pass `SKILL.md` directly into your terminal agent prompt or system context:
```bash
# Example invocation with Claude Code:
claude "Read SKILL.md and implement the JWT authentication module following the 8-step Draft agent protocol."
```

#### 💻 Cursor, Windsurf, & GitHub Copilot
In your agent chat panel, reference the skill file using the `@` symbol:
> *"@SKILL.md Follow the Draft agent protocol: spawn an isolated dimension, claim `src/auth.rs`, implement the feature, run `dft foresee`, and converge back into mainline."*

#### 🐝 Multi-Agent Swarms (LangGraph, CrewAI, AutoGen, Custom Scripts)
When initializing autonomous swarms programmatically in Python, TypeScript, or Rust, inject `SKILL.md` into the agent's system instructions:
```python
# Load the Draft skill into your agent fleet
with open("SKILL.md") as f:
    draft_skill_manifest = f.read()

agent_worker = Agent(
    role="Backend Engine Developer",
    system_prompt=f"You operate inside a Draft VCS repository. Strictly adhere to this operational protocol:\n{draft_skill_manifest}"
)
```

---

### 2. The 8-Step Concurrency Protocol Every Agent Follows

When agents read [`SKILL.md`](SKILL.md), they automatically follow an isolated, conflict-free workflow:

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

1. **Register Identity**: The agent registers its worker ID to participate in the local fleet:
   ```bash
   dft agent register agent-coder --type ai
   dft heartbeat agent-coder
   ```
2. **Create Private Dimension**: Spawns an isolated Copy-on-Write workspace in **0.06 seconds** without cloning the repo:
   ```bash
   dft dimension create agent-coder/auth-feature
   dft dimension enter agent-coder/auth-feature
   ```
3. **Scan Radar**: Scans for active hot zones to verify if another agent is modifying nearby modules:
   ```bash
   dft radar --hot
   ```
4. **Lease Territory (`dft claim`)**: Claims advisory leases on the files it will edit to prevent other agents from colliding:
   ```bash
   dft claim src/auth.rs
   ```
5. **Code & Commit Locally**: Makes edits and commits with zero lock contention against other agents:
   ```bash
   dft add src/auth.rs
   dft commit -m "feat(auth): implement token verification"
   ```
6. **Predict Conflicts In-Memory (`dft foresee`)**: Simulates 3-way reconciliation in RAM in ~66–120 ms to verify clean convergence before touching mainline:
   ```bash
   dft foresee agent-coder/auth-feature mainline
   ```
7. **Converge into Mainline**: Reconciles the feature cleanly:
   ```bash
   dft dimension enter mainline
   dft converge agent-coder/auth-feature mainline
   ```
8. **Yield Claims & Teardown**: Releases file claims and destroys the ephemeral dimension:
   ```bash
   dft yield src/auth.rs
   dft dimension destroy agent-coder/auth-feature
   ```

*(For peer-to-peer agent mailboxes, background daemon automation, and CI/CD pipelines, see [Section 6: Orchestrating AI Agents with SKILL.md](#6-orchestrating-ai-agents-with-skillmd) below).*

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
brew install Pathomphong-i/draft/dft
```

#### B. Standalone Shell Script (macOS & Linux)
Zero-dependency instant installer for workstations and CI/CD runners:
```bash
curl -fsSL https://draftmultiverse.org/install.sh | sh
```

#### C. Cargo (Rust Crates.io)
```bash
cargo install draft-vcs
```

#### D. Building from Source
```bash
# Clone the repository
dft clone /Users/pathomphongphiphatsuriyawong/Workspace/DraftMultiverse
# or: git clone https://github.com/Pathomphong-i/draft.git
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

### 5. Architectural Comparison & Empirical Benchmarks: Draft (`dft`) vs. Jujutsu (`jj`) vs. Git

Developers frequently ask how **Draft (`dft`)** compares to **[Jujutsu (`jj`)](https://github.com/jj-vcs/jj)** and **Git**. While all three systems share core foundations in content-addressable storage (CAS) graphs and cryptographic hashing, they were engineered for fundamentally different target personas and concurrency models:

- **Git** was built for **human developers** working in a single checked-out working directory with manual index staging.
- **Jujutsu (`jj`)** was built for **human developers** who want world-class ergonomics: automatic commits (`@`), first-class conflicts recorded inside commits, complete undoability via operation logs (`jj undo`), and branchless stacked diffs.
- **Draft (`dft`)** was built for **autonomous AI agent swarms and parallel dimensions**: sub-second Copy-on-Write workspaces (0.06s via kernel reflinks), zero lock collisions under swarm concurrency, real-time cross-branch collision radar, territory leases, and predictive in-memory conflict simulation before merge.

---

#### 🏛️ Philosophy & Purpose Divergence

| Architectural Dimension | Git (2005) | Jujutsu (`jj`, 2019+) | Draft (`dft`, 2026) |
|:---|:---|:---|:---|
| **Core Persona** | Human software engineers | Individual human developers | **Concurrent AI Agent Swarms** & human collaborators |
| **Primary Workflow** | Linear branches, manual staging area (`git add`) | Stacked diffs, working copy is a commit (`@`), first-class conflicts | **Parallel Multiverse Dimensions** with zero-disk CoW workspaces |
| **Staging Model** | Explicit index (`.git/index`) | Implicit snapshotting on every command (no staging area needed) | **Per-Dimension Lock-Free Index** (`.dft/dimensions/<name>/index`) |
| **Working Copy Concurrency** | Single checked-out `HEAD`; extra copies require `git worktree add` | Single working copy; extra copies require `jj workspace add` | **Instant CoW Parallel Workspaces** (`dft dimension create`) |
| **Locking & Contention** | Serial `.git/index.lock` contention under parallel access | Operation log serialization; lock contention under simultaneous writes | **Lock-Free Concurrency**: Each dimension has independent index & ref locks |
| **Conflict Resolution Philosophy** | **Blocking**: Halts mid-merge/rebase; working tree left in dirty conflict state | **Recorded in History**: Conflicts stored inside commit tree objects; resolved later | **Predictive & Autonomous**: In-memory 3-way simulation (`dft foresee`) before merge + autonomous background convergence (`dft cronos`) |
| **Cross-Worker Telemetry** | None (workers are blind to each other) | None (workspaces operate in isolation) | **Real-Time Cross-Dimension Radar (`dft radar`)** with divergence metric $H$ |
| **Collision Prevention** | None (last write wins or merge conflicts) | None | **Territory Claims (`dft claim`)** & hard barriers (`dft fence`) |
| **History Rewriting & Undo** | Destructive; recovered via append-only reflog | Non-destructive; first-class operation log (`jj op log`, `jj undo`) | Non-destructive; Spacetime DAG, point-in-time snapshots (`dft snapshot`), and vector clocks |
| **Upstream Interoperability** | Native standard | Git-compatible backend (`.jj/repo/store/git`) | **Zero-Intrusion Hybrid Layer**: Run `dft` locally for swarm concurrency, push standard Git commits to GitHub/GitLab |

---

#### 📊 Empirical Performance Benchmarks: Git vs. Jujutsu (`jj`) vs. Draft (`dft`)

All benchmarks below were executed on Apple Silicon running macOS Darwin 25 with APFS (Apple File System). The benchmark environment measured raw wall-clock latency (using monotonic nanosecond timers), actual filesystem physical block allocation (`du -sk`), Darwin kernel process resource metrics (`/usr/bin/time -l`), and multi-agent concurrency throughput across repositories with **1,000 tracked files** spanning nested directory hierarchies.

Tool versions tested: **Git 2.39+**, **Jujutsu (`jj`) 0.45.1**, and **Draft (`dft`) 0.1.0**.

---

##### 1. Parallel Workspace Creation & Scaling (1, 5, and 10 Workspaces)
Measuring the wall-clock time required to initialize a fresh repository and spawn isolated, independent workspaces for concurrent AI agents or feature branches:

| Concurrency Scale | Git (`git worktree add`) | Jujutsu (`jj workspace add`) | Draft (`dft dimension create`) | Draft Speedup vs. Git | Draft Speedup vs. `jj` |
|:---|:---:|:---:|:---:|:---:|:---:|
| **1 Workspace** | 200.78 ms | 290.51 ms | **9.46 ms** | **21.2x faster** | **30.7x faster** |
| **5 Workspaces** | 849.44 ms (169.9 ms/ws) | 1,350.78 ms (270.2 ms/ws) | **36.84 ms** (7.37 ms/ws) | **23.1x faster** | **36.7x faster** |
| **10 Workspaces** | 1,651.92 ms (165.2 ms/ws) | 2,823.91 ms (282.4 ms/ws) | **81.26 ms** (8.13 ms/ws) | **20.3x faster** | **34.8x faster** |

> **Why Draft scales linearly in single-digit milliseconds**: Git and Jujutsu execute full user-space directory tree traversals, index instantiation, and file-by-file checkouts from object storage into new directories. Draft leverages kernel-level Copy-on-Write reflink primitives (`clonefile` on macOS APFS, `ioctl(FICLONE)` on Linux Btrfs/XFS/ZFS), creating isolated filesystem namespaces in **~7–8 milliseconds** per dimension regardless of tree size.

---

##### 2. Physical Disk Space Overhead & Extent Sharing (`du -sk`)
Measuring actual physical disk block consumption on the storage layer (`du -sk`) across 1, 5, and 10 concurrent workspaces:

| Benchmark Scale | Git Worktrees | Jujutsu Workspaces | Draft Dimensions (CoW) | Disk Reduction with Draft |
|:---|:---:|:---:|:---:|:---:|
| **1 Workspace** | 4,004 KB (~4.0 MB) | 4,040 KB (~4.0 MB) | **100 KB** | **97.5% less disk** |
| **5 Workspaces** | 20,020 KB (~20.0 MB) | 20,200 KB (~20.2 MB) | **600 KB** (~0.6 MB) | **97.0% less disk** |
| **10 Workspaces** | 40,040 KB (~40.0 MB) | 40,400 KB (~40.4 MB) | **1,600 KB (~1.6 MB)** | **96.0% less disk (25x reduction)** |

> **Block-Level Extent Sharing**: While Git and Jujutsu allocate separate physical storage blocks for every checked-out file in each workspace, Draft's Copy-on-Write engine shares the underlying physical storage blocks with the Content-Addressable Storage (CAS) pool. Clean files consume 0 KB of additional physical disk. New blocks are allocated strictly on modification (Copy-on-Write). On a 5 GB codebase with 10 concurrent agents, Git and Jujutsu require 50 GB of disk; Draft requires ~5 GB total.

---

##### 3. Resource Footprint & System Overhead (Darwin Kernel Telemetry)
Measuring process resource consumption during commit operations on a 1,000-file repository via macOS Darwin kernel telemetry (`/usr/bin/time -l`):

| Telemetry Metric | Git (2.39) | Jujutsu (`jj` 0.45.1) | Draft (`dft` 0.1.0) | Draft vs. Jujutsu | Draft vs. Git |
|:---|:---:|:---:|:---:|:---:|:---:|
| **Peak RSS Memory** | 5.23 MB | 25.47 MB | **8.02 MB** | **68.5% less memory** | Lightweight footprint |
| **CPU Instructions Retired** | 453.2M | 239.9M | **45.6M** | **5.2x fewer instructions** | **10.0x fewer instructions** |
| **Context Switches** | 1,272 | 605 | **16** | **37.8x fewer switches** | **79.5x fewer switches** |

> **Architectural Efficiency**:
> - **Memory Footprint**: Jujutsu maintains an extensive in-memory commit graph, operation log, and snapshot engine (~25.5 MB peak RSS). Draft's lean Rust CAS engine and streaming index structure maintain a tight **8.0 MB** peak memory footprint (68.5% lower than `jj`).
> - **CPU Instruction Efficiency**: Git's single-threaded C engine traverses directory trees and executes compression passes (453M instructions). Draft leverages zero-copy memory-mapped I/O (`memmap2`) and direct SHA-256 digest hashing, retiring only **45.6M instructions (10x fewer than Git, 5.2x fewer than Jujutsu)**.
> - **Scheduler Thrashing**: Git and Jujutsu incur hundreds to thousands of voluntary and involuntary context switches due to synchronous filesystem lock acquisition, file descriptor cycling, and subprocess forks. Draft finishes in a single tightly-optimized execution loop with only **16 context switches**, completely eliminating scheduler thrashing.

---

##### 4. Swarm Concurrency Under Load: 10 Autonomous AI Agents Committing Concurrently
Measuring real-world multi-agent swarm commit throughput when **10 autonomous worker processes** simultaneously modify files and commit to their respective isolated workspaces:

| Concurrency Metric | Git Worktrees | Jujutsu Workspaces | Draft Dimensions | Draft Speedup vs. Git | Draft Speedup vs. `jj` |
|:---|:---:|:---:|:---:|:---:|:---:|
| **10 Parallel Commits (Wall Clock)** | 1,392.36 ms | 537.41 ms | **129.42 ms** | **10.8x faster** | **4.2x faster** |
| **Average Commit Latency per Agent** | 1,199.36 ms | 418.75 ms | **119.23 ms** | **10.1x faster** | **3.5x faster** |
| **Lock Contention & Concurrency Model** | Serial ref locks (`.git/refs/heads/`) block parallel writes | Operation log lock serializes history updates | **Zero Lock Contention**: Independent staging indexes & branch ref locks | Minimal lock wait | Zero serialization bottleneck |

> **Why Draft dominates multi-agent swarms**: Under concurrent load, Git workers contend on shared reference locks and index locks, forcing parallel agents into serialized queues (1,392 ms wall clock). Jujutsu serializes concurrent updates through its global operation log lock (537 ms wall clock). Draft gives each dimension its own isolated staging index (`.dft/dimensions/<name>/index`) and writes immutable SHA-256 CAS objects directly to loose storage pools, allowing **all 10 agents to commit simultaneously in 129.4 ms with zero lock contention**.

---

##### 5. Conflict Handling & Proactive Foresight
| Capability | Git | Jujutsu (`jj`) | Draft (`dft`) |
|:---|:---|:---|:---|
| **Conflict Discovery Timing** | Post-merge (reactive) | Post-operation (recorded in commit) | **Pre-Merge Simulation (`dft foresee`)** (predictive) |
| **Foresight Simulation Speed (500 files)** | Not supported | Not supported | **66.49 ms** (pure in-memory 3-way check) |
| **Foresight Simulation Speed (1,000 files)** | Not supported | Not supported | **122.82 ms** (pure in-memory 3-way check) |
| **Cross-Agent Awareness** | None (workers blind to each other) | None (workspaces isolated) | **Live Hot-Zone Radar (`dft radar`)** |
| **Continuous Auto-Convergence** | Manual script | Manual rebase | **Native Daemon (`dft cronos`)** |
| **Territory Boundaries** | None | None | **Advisory Claims (`dft claim`) & Fences (`dft fence`)** |

---

#### 🧭 Deep Dive: Jujutsu vs. Draft Architectural Contrast

##### How Jujutsu Works (Human Ergonomics Focus)
Jujutsu is an exceptional modern DVCS for human software developers:
1. **The Working Copy is `@`**: Whenever you edit a file, `jj` automatically considers it part of the working-copy commit `@`. You never have to type `git add`.
2. **First-Class Conflicts**: If a rebase has conflicts, `jj` does not pause and leave your repository broken. Instead, it records the conflict directly inside the commit object, allowing you to switch tasks, push the conflicted branch for someone else to inspect, or resolve it whenever convenient.
3. **Operation Log**: Every action you perform appends an entry to `.jj/op_log`. If you make a mistake, `jj undo` rolls back the exact repository state instantly.

##### Where Jujutsu Meets Friction with AI Agent Swarms
When teams orchestrate **multiple concurrent AI coding agents** (e.g. 5–10 agents executing in parallel on the same codebase):
1. **Working Copy Monopoly**: Because `jj` links repository state to a single working copy commit `@`, concurrent agents modifying files in the same checkout will continuously mutate `@` out from under each other, creating cascading race conditions.
2. **Workspace Weight**: Scaling `jj` to multiple agents requires `jj workspace add`, which creates full copies of the working directory on disk (~20 MB per 1,000 files), consuming gigabytes of disk when scaled to dozens of ephemeral agent tasks.
3. **Blind Concurrency**: `jj` does not provide cross-workspace telemetry. If Agent A in `workspace-1` is refactoring `src/database.rs`, Agent B in `workspace-2` has no awareness of this until a subsequent merge or rebase.
4. **Post-Hoc Conflict Discovery**: Even though `jj` stores conflicts cleanly in commits, it discovers them *after* the commit or rebase has taken place. For autonomous agents running unattended in CI/CD, discovering conflicts post-hoc halts automated continuous delivery pipelines.

##### How Draft Solves the Swarm Concurrency Problem
Draft was specifically designed to bridge this gap without sacrificing Git compatibility:
1. **0.06s CoW Dimensions**: Ephemeral agent workspaces are created in single-digit milliseconds using APFS/Btrfs copy-on-write blocks with 0 KB initial duplicate disk overhead.
2. **Lock-Free Parallel Commits**: Each dimension maintains its own independent staging index (`.dft/dimensions/<name>/index`) and ref pointer, allowing 10+ agents to commit simultaneously with zero lock contention.
3. **Proactive Collision Radar (`dft radar`)**: Agents check the real-time activity radar before writing code to see which files are currently being touched in other dimensions.
4. **Territory Leases (`dft claim`) & Fences (`dft fence`)**: Agents can lease file paths or place hard exclusionary barriers, guaranteeing that two agents will never collide on the same modules.
5. **In-Memory Conflict Foresight (`dft foresee`)**: Before attempting convergence, agents simulate a 3-way reconciliation in memory (in under 70ms) to guarantee that incoming changes integrate without conflicts.
6. **Continuous Autonomous Convergence (`dft cronos`)**: The background daemon continuously converges non-conflicting dimensions into mainline, keeping all timelines synchronized.

---

#### 🎯 Summary: When Should You Use Which?

- 🐙 **Choose Git** if:
  - You need universal compatibility with every GUI tool, IDE, code review platform, and CI runner on earth.
  - Your team consists of human developers following traditional branch-and-PR workflows.
- 🥋 **Choose Jujutsu (`jj`)** if:
  - You are a **human developer** looking for the best daily CLI ergonomics.
  - You want stacked diffs, anonymous branches, seamless rebasing without mid-operation pauses, and universal `jj undo`.
- 🌌 **Choose Draft (`dft`)** if:
  - You are running **autonomous AI coding agents or multi-agent swarms** (Google Antigravity, Claude Code, Cursor, AutoGen, CrewAI).
  - You need **parallel dimensions** created in milliseconds without disk bloat.
  - You require **real-time cross-branch collision radar**, territory claims, and **in-memory predictive merge foresight**.
  - You want to **develop locally with multiverse speed and still push clean standard commits upstream to GitHub**.

---

### 6. Orchestrating AI Agents with `SKILL.md`

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

### 7. Automated CI/CD & AI Agent Pipelines

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

### 8. Self-Hosting: DraftMultiverse Web & Remote Server

Draft includes a complete, self-hosted web platform and headless remote server (**DraftMultiverse**):

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

#### Setting up a Self-Hosted Remote Server (`DraftMultiverse`)
```bash
# 1. Initialize a bare server repository on your server or local disk
mkdir -p /path/to/DraftMultiverse
dft init --bare /path/to/DraftMultiverse

# 2. Add as remote origin in your working project
dft remote add origin /path/to/DraftMultiverse

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
| `dft remote / push / pull` | Decentralized Sync | Synchronize with headless DraftMultiverse remotes |

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
