# Hacker News ("Show HN") Launch Guide

### 🎯 Submission Details
* **Submission URL**: `https://news.ycombinator.com/submit`
* **Title**:
  ```text
  Show HN: Draft – A Multiverse Version Control System for AI Agent Swarms (Rust)
  ```
* **URL Field**:
  ```text
  https://github.com/Pathomphong-i/draft
  ```
* **Best Timing**: Tuesday, Wednesday, or Thursday between **7:00 AM – 9:00 AM EST** (which is 11:00 AM – 1:00 PM UTC / 7:00 PM – 9:00 PM ICT).

---

### 📝 First Comment (Post immediately after submitting)

```markdown
Hi HN,

I built Draft (dft) because Git's single-working-tree model breaks down the moment you run multiple autonomous AI agents concurrently on one codebase.

### The Problem
When orchestrating teams of specialized AI coding agents (e.g. Agent Alpha on database migrations, Agent Beta on API endpoints, Agent Gamma on frontend UI, Agent Delta on test suites), traditional VCS introduces painful bottlenecks:
1. **Single Working Tree Bottleneck**: Switching branches in a dirty working tree tramples uncommitted files. Running multiple concurrent checkouts requires brittle `git worktree` setups or multi-gigabyte repository clones that duplicate gigabytes of disk and build artifacts (like `target/` or `node_modules`).
2. **Blind Collisions**: Agents modifying files concurrently have zero cross-branch awareness until final merge/rebase triggers complex conflict triage.
3. **Chronos vs. Kairos**: Git binds version control strictly to linear chronological time (one active branch per working tree), whereas autonomous swarms require parallel working drafts that converge opportunistically.

### How Draft Works (Rust)
Draft retains content-addressable storage (SHA-256 CAS) while introducing a multiverse architecture tailored for concurrent agents:

1. **Sub-Second Copy-on-Write Dimensions**: Leveraging kernel-level reflinks (`clonefile` on macOS APFS, `FICLONE` on Linux Btrfs/XFS), creating a parallel dimension with 10,000 files takes ~0.06 seconds and consumes 0 additional disk blocks until modified.
2. **Cross-Dimensional Radar (`dft radar`)**: Telemetry that detects dirty files and active write operations across all concurrent dimensions in real-time.
3. **Territory Leasing (`dft claim` / `dft fence`)**: Agents acquire advisory leases or hard exclusionary barriers on file paths to coordinate safely without locking the global repo.
4. **Predictive In-Memory 3-Way Merge (`dft foresee`)**: Simulates 3-way tree reconciliation (Myers diff algorithm) directly in RAM in ~70ms before touching mainline, catching collisions before they happen.
5. **Native Agent Protocol (`SKILL.md`)**: Ships with an open standard YAML/Markdown manifest compliant with agent protocols (Claude Code, Antigravity, Cursor, LangGraph, AutoGen) that guides agents through an 8-step concurrency workflow.

We also built an interactive web visualizer (Myers diff, DAG timeline, file tree) running entirely on GitHub Pages:
https://pathomphong-i.github.io/draft/

GitHub: https://github.com/Pathomphong-i/draft

I’d love feedback on the architecture, the CAS engine, the divergence metric formula, and how you see version control evolving for autonomous software development!
```

---

### 💡 Hacker News Etiquette & Advice
1. **Be responsive in the comments**: When people ask questions about how Draft compares to Jujutsu (`jj`), Git worktrees, or Pijul, answer thoughtfully and respectfully.
   - *Vs Jujutsu*: Jujutsu models changes as an operation log and supports working-copy commits, but does not offer sub-second APFS/Btrfs CoW dimensions, real-time cross-workspace collision radar, territory leases (`claim`/`fence`), or in-memory predictive merge simulations.
   - *Vs Git Worktrees*: Git worktrees duplicate the index, serialize operations through index locks, and lack cross-worktree awareness.
2. **Don't ask friends for upvotes on the same network/IP**: HN’s voting ring detector will shadowban the post. Let organic interest take it up!
