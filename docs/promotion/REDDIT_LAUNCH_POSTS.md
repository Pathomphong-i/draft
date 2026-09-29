# Reddit Launch Posts (Tailored per Subreddit)

---

## 1. r/rust

* **Title**:
  ```text
  Draft: A Multiverse Version Control System written in Rust for concurrent AI agent swarms
  ```
* **Post Body**:
  ```markdown
  Hey r/rust,

  Over the past several months, I've been building **Draft (`dft`)**, an open-source version control system engineered in Rust for concurrent multi-agent and human developer workflows.

  - GitHub: https://github.com/Pathomphong-i/draft
  - Live Web Demo: https://pathomphong-i.github.io/draft/

  ### The Problem
  When orchestrating multiple concurrent AI coding agents on a single repo, Git's single-working-tree model breaks down. Running multiple checkouts via `git worktree` or full clones duplicates gigabytes of build artifacts (`target/`) and causes frequent lock contention on index writes.

  ### Rust Architecture & Subsystems
  Draft is split into a modular Cargo workspace:
  - `daft-core`: Cryptographic SHA-256 Content-Addressable Storage (CAS), memory-mapped I/O (`memmap2`) for zero-copy inspection, zlib streaming compression, and Eugene Myers' $O(ND)$ greedy difference algorithm.
  - `daft-dimension`: Sub-second Copy-on-Write workspace provisioning using OS kernel reflinks (`clonefile` on macOS APFS, `ioctl(FICLONE)` on Linux Btrfs/XFS). Spawning an isolated 10,000-file dimension takes 0.06s with 0 KB initial block allocation.
  - `daft-awareness`: Telemetry engine with real-time file access radar and advisory territory leases (`dft claim` / `dft fence`).
  - `daft-convergence`: In-memory 3-way tree merge simulations (`dft foresee`) and atomic multi-branch collapse (`dft collapse`).
  - `daft-agent`: Built-in agent identity, heartbeats, and asynchronous file-based mailboxes.
  - `daft-cli`: Terminal interface built with `clap` and `tokio`.

  ### Benchmarks
  We benchmarked 10 concurrent autonomous workers against Git (2.50) and Jujutsu (0.45). Draft achieved 3.5x faster workspace provisioning and zero lock errors under concurrent commit pressure thanks to lock-free CAS writes.

  The project is licensed under GPL v2.0. I'd love code reviews, architectural feedback, and thoughts from the Rust community!
  ```

---

## 2. r/LocalLLaMA & r/ClaudeAI

* **Title**:
  ```text
  We built a version control system (Draft) so Claude Code & local agent swarms can code concurrently without lock collisions
  ```
* **Post Body**:
  ```markdown
  Hey everyone,

  If you've experimented with running multiple autonomous coding agents simultaneously (e.g. Claude Code, Cursor, AutoGen, CrewAI, LangGraph), you've probably hit the "Git Concurrency Wall":
  - Switching branches trampling uncommitted files.
  - Disk bloat from duplicating repos and dependencies.
  - Agents blindly editing the same files and causing massive merge conflicts at the end.

  To solve this, we open-sourced **Draft (`dft`)**:
  - GitHub: https://github.com/Pathomphong-i/draft
  - Web Demo: https://pathomphong-i.github.io/draft/

  ### What Draft does for AI Agents:
  1. **Instant 0.06s CoW Dimensions**: Each agent gets its own isolated working tree using APFS/Btrfs copy-on-write without copying files or bloating your SSD.
  2. **Cross-Agent Radar (`dft radar`)**: Agents can see what files other running agents are modifying in real-time.
  3. **Territory Leases (`dft claim`)**: Agents claim files before editing so two agents don't accidentally write to the same module.
  4. **Predictive Merge (`dft foresee`)**: Simulates 3-way reconciliation in RAM in ~70ms before touching your main branch.
  5. **Native `SKILL.md` Manifest**: An open-standard prompt file you can pass to Claude Code or local agents that guides them through an 8-step collision-free workflow.

  You can install it on macOS/Linux with:
  ```bash
  curl -fsSL https://raw.githubusercontent.com/Pathomphong-i/draft/main/install.sh | sh
  ```

  Check out the repo and let us know what you think!
  ```

---

## 3. r/programming

* **Title**:
  ```text
  From Chronos to Kairos: Why AI agent swarms require rethinking Git's single-working-tree model
  ```
* **Post Body**:
  ```markdown
  When Linus Torvalds engineered Git in 2005, version control was designed for a single developer sitting at a single terminal, committing linearly through time.

  In 2026, software development is increasingly parallel: teams of autonomous AI agents write database layers, API endpoints, and frontend components simultaneously.

  Under this load, traditional single-worktree Git workflows hit severe friction:
  - Linear branch checkouts cause unstaged file trampling.
  - Clones duplicate gigabytes of build artifacts.
  - Branch integration is purely reactive — conflicts are discovered only after hours of diverged coding.

  We built **Draft (`dft`)**, an open-source Rust version control system that replaces linear time (Chronos) with parallel working drafts (Kairos) using kernel-level Copy-on-Write reflinks, real-time collision radar, and in-memory predictive merge simulations.

  - Deep Technical Architecture & Benchmarks: https://github.com/Pathomphong-i/draft
  - Interactive Browser Visualizer: https://pathomphong-i.github.io/draft/

  Read our benchmark comparing Draft vs Git vs Jujutsu across 10 concurrent workers in the repo!
  ```
