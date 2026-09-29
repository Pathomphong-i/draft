# X (Twitter) & Bluesky Launch Thread

Ready-to-post thread for Twitter / X, Bluesky, and Threads.

---

### Tweet 1 [Hook + Media]
Why does Git break when you run multiple AI agents at once?

Git was built in 2005 for 1 human switching 1 branch at a time.

Today, I'm open-sourcing **Draft (`dft`)**: The Multiverse Version Control System for Concurrent AI Agent Swarms, written in Rust 🦀⚡

🧵👇 [Attach image: docs/assets/draft_multiverse_3d.jpg or screenshot of web UI]

---

### Tweet 2 [The Concurrency Problem]
When you run 5 autonomous AI coding agents concurrently on a Git repo:
❌ Multiple clones duplicate gigabytes of disk and build artifacts (`target/`, `node_modules`).
❌ Agents blindly edit overlapping files without knowing it.
❌ Painful merge conflicts at the end of every run.

We need version control built for agent concurrency, not linear human commits.

---

### Tweet 3 [0.06s Parallel Dimensions]
1️⃣ **Instant 0.06s Parallel Dimensions**

Instead of slow clones or brittle worktrees, Draft uses kernel-level Copy-on-Write reflinks:
- Apple APFS `clonefile` on macOS
- `ioctl(FICLONE)` on Linux (Btrfs / XFS)

A 10,000-file dimension spawns in 0.06s with **0 KB** extra disk storage.

---

### Tweet 4 [Cross-Agent Radar]
2️⃣ **Cross-Dimensional Radar (`dft radar`)**

Agents don't have to code blind anymore.
- `dft radar --hot` scans all active dimensions in real-time to detect hot files.
- `dft claim <path>` leases an advisory territory lock so agents avoid trampling on each other's code.

---

### Tweet 5 [Predictive In-Memory Merge]
3️⃣ **Predictive Conflict Foresight (`dft foresee`)**

Why wait until `git merge` to discover your agent broke the build?

`dft foresee <dim> mainline` runs an in-memory 3-way tree simulation in ~70ms, forecasting conflicts before anything touches the mainline branch.

---

### Tweet 6 [Native Agent SKILL.md]
4️⃣ **Native `SKILL.md` Agent Protocol**

Draft includes an open-standard agent skill manifest.

Just point Claude Code, Cursor, Windsurf, or custom LangGraph/CrewAI agents to `SKILL.md`, and they automatically follow an 8-step safe concurrency protocol:
Register → Dimension → Radar → Claim → Commit → Foresee → Converge → Yield.

---

### Tweet 7 [Benchmarks]
In our empirical 10-agent benchmarks against Git and Jujutsu (`jj`):
⚡ 10 Workspace Provisioning: **Draft is 3.5x faster**
🚀 Concurrent Commit Throughput: **Draft is 1.8x higher**
🔒 Concurrency Lock Contention: **Draft has 0 errors**

Full benchmark data:
https://github.com/Pathomphong-i/draft/blob/main/docs/BENCHMARK_RESULTS.md

---

### Tweet 8 [Links & Call to Action]
Explore Draft today:
🌐 Interactive Web Platform & DAG Demo: https://pathomphong-i.github.io/draft/
🐙 GitHub Repository: https://github.com/Pathomphong-i/draft
⚡ Universal Install: `curl -fsSL https://raw.githubusercontent.com/Pathomphong-i/draft/main/install.sh | sh`

If you're building with AI agents or love Rust systems programming, check it out and drop a ⭐️!
