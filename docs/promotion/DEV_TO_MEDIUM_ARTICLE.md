# Why Git Can't Handle AI Agent Swarms (And How We Built Draft in Rust to Fix It)

*Author: Pathomphong Phiphatsuriyawong*  
*Repository: [https://github.com/Pathomphong-i/draft](https://github.com/Pathomphong-i/draft)*  
*Live Demo: [https://pathomphong-i.github.io/draft/](https://pathomphong-i.github.io/draft/)*

---

In 2005, Linus Torvalds changed software engineering forever when he built Git in two weeks. His content-addressable storage model, cryptographic acyclic commit DAG, and fast local branching were revolutionary for human developers working on the Linux kernel.

Fast forward over two decades. Today, software development is entering a radical new paradigm: **concurrent autonomous AI agent swarms**.

Instead of a single human developer methodically writing a feature, switching branches, and submitting a pull request, engineering teams are dispatching fleets of specialized AI agents:
- **Agent 1**: Database schema & migration rewrite
- **Agent 2**: REST API and WebSocket controllers
- **Agent 3**: React/Vue frontend components
- **Agent 4**: End-to-end integration test suites
- **Agent 5**: Performance optimization and memory profiling

The moment you unleash multiple AI agents concurrently on a single repository, **Git's single-working-tree architecture hits a wall**.

---

## The 3 Bottlenecks of Git in the Age of AI Swarms

### 1. The Single Working-Tree Bottleneck
Git limits a working repository to a single checked-out `HEAD` and working directory. If Agent 1 switches branches while Agent 2 is writing code, uncommitted files are trampled.

To bypass this, developers resort to `git worktree` or full repo clones. But cloning a repository 10 times duplicates tens of thousands of build artifacts (`node_modules/`, `target/`, `venv/`), exhausting disk space and I/O bandwidth.

### 2. Blind Collisions
In Git, branches are completely isolated and blind to one another. Agent 1 editing `src/auth.rs` on branch A has no idea that Agent 2 is refactoring `src/auth.rs` on branch B. The collision is only discovered hours later during merge or rebase, requiring complex, expensive conflict triage.

### 3. Reactive, Post-Hoc Merging
Traditional version control operates reactively: you write code first, and discover whether it cleanly merges last. When dealing with autonomous agents, reactive merging leads to cascading rollbacks and wasted compute tokens.

---

## Enter Draft (`dft`): The Multiverse Version Control System

To solve these concurrency bottlenecks, we engineered **Draft (`dft`)** in Rust.

Draft retains the cryptographic content-addressable storage (CAS) principles pioneered by Linus Torvalds, but introduces a **Multiverse Architecture** built specifically for concurrent human and AI agent collaboration.

```
┌────────────────────────────────────────────────────────┐
│                   Draft Multiverse                     │
│  ┌─────────────────┐ ┌─────────────────┐ ┌──────────┐  │
│  │  Dimension A    │ │  Dimension B    │ │Mainline  │  │
│  │ (Agent: Backend)│ │ (Agent: Frontend│ │          │  │
│  └────────┬────────┘ └────────┬────────┘ └────▲─────┘  │
│           │                   │               │         │
│           │       Radar & Territory Claims    │         │
│           └───────────────────┼───────────────┘         │
│                               ▼                         │
│                  Predictive 3-Way Merge                 │
│                     (`dft foresee`)                     │
└────────────────────────────────────────────────────────┘
```

Here is what makes Draft fundamentally different:

### 1. Instant 0.06s Copy-on-Write Dimensions
Instead of copying files, Draft leverages kernel-level reflink primitives:
- **macOS**: Apple APFS `clonefile()`
- **Linux**: `ioctl(FICLONE)` on Btrfs, XFS, and ZFS

Spawning an isolated workspace with 10,000 files completes in **0.06 seconds** and consumes **0 KB** of extra disk space. Each agent gets its own dimension without duplicating disk blocks.

```bash
dft dimension create agent-auth/feature-jwt
dft dimension enter agent-auth/feature-jwt
```

### 2. Cross-Dimensional Radar & Territory Claims
Draft provides real-time situational awareness across all parallel dimensions:
- **`dft radar --hot`**: Detects hot files currently being modified by other agents across all dimensions.
- **`dft claim <file>`**: Leases an advisory lock on a file path, signaling to other agents to stay clear.
- **`dft fence <file>`**: Erects an exclusionary barrier to prevent unauthorized modifications.

### 3. Predictive In-Memory Merge (`dft foresee`)
Before an agent commits code back to the mainline branch, it runs:
```bash
dft foresee agent-auth/feature-jwt mainline
```
Draft executes an in-memory 3-way tree reconciliation (using Eugene Myers' $O(ND)$ difference algorithm) in ~70ms. It predicts whether the integration will cleanly succeed or emit conflict markers *before* modifying any branch refs.

### 4. Built-in `SKILL.md` Agent Standard
Draft ships with an open-standard manifest ([`SKILL.md`](https://github.com/Pathomphong-i/draft/blob/main/SKILL.md)) that any modern AI agent (Claude Code, Google Antigravity, Cursor, Windsurf, AutoGen, CrewAI) can ingest directly.

When agents read `SKILL.md`, they autonomously execute an 8-step safety protocol:
1. `dft agent register`
2. `dft dimension create`
3. `dft radar --hot`
4. `dft claim <file>`
5. Edit & commit locally
6. `dft foresee`
7. `dft converge`
8. `dft yield`

---

## Empirical Benchmarks: Draft vs Git vs Jujutsu (`jj`)

We ran empirical benchmarks with **10 concurrent autonomous agents** executing full task lifecycles on a full-stack web application:

| Metric | Git (2.50) | Jujutsu (0.45) | Draft (`dft` 0.2.1) | Winner |
|---|---|---|---|---|
| **10 Workspace Spawn** | 0.812s | 0.745s | **0.231s** | **Draft (3.5x faster)** |
| **Commit Throughput** | 24.1 ops/s | 28.3 ops/s | **44.2 ops/s** | **Draft (1.8x higher)** |
| **Storage Delta** | +48,200 KB | +46,100 KB | **+120 KB** | **Draft (APFS CoW)** |
| **Lock Contention Errors** | 3 errors (`index.lock`) | Divergent ops | **0 errors** | **Draft (Lock-Free)** |

---

## Try Draft Today

Draft is completely open-source under the GNU General Public License v2.0 (GPL v2).

- **Universal 1-line installer (macOS & Linux)**:
  ```bash
  curl -fsSL https://raw.githubusercontent.com/Pathomphong-i/draft/main/install.sh | sh
  ```
- **Live Interactive Web Playground**: [https://pathomphong-i.github.io/draft/](https://pathomphong-i.github.io/draft/)
- **GitHub Repository**: [https://github.com/Pathomphong-i/draft](https://github.com/Pathomphong-i/draft)

We invite developers, systems engineers, and AI researchers to explore the code, test it with their agent swarms, and contribute!
