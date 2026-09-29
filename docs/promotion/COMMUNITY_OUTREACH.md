# Developer Communities & Newsletters Outreach Kit

This document provides exact copy-paste blurbs and submission links to submit Draft to major newsletters, podcasts, and developer discords.

---

## 1. Newsletter Submissions

### A. Console.dev (Top Weekly Open-Source Dev Tools Newsletter)
* **Submission Form**: [https://console.dev/submit/](https://console.dev/submit/)
* **Project Name**: Draft (`dft`)
* **URL**: `https://github.com/Pathomphong-i/draft`
* **Elevator Pitch**:
  > Draft is a version control system written in Rust engineered for concurrent AI agent swarms. While Git limits workflows to a single working tree and sequential branch checkouts, Draft introduces 0.06s kernel-level Copy-on-Write (CoW) dimensions, real-time cross-agent collision radar, territory leases (`claim`/`fence`), and predictive in-memory 3-way merge simulations (`dft foresee`) before branch convergence.

---

### B. TLDR Tech & TLDR AI (Read by 1M+ tech professionals)
* **Submission Form**: [https://tldr.tech/](https://tldr.tech/) (or submit a link in their contact form)
* **Headline**: Draft: A Multiverse Version Control System for AI Agent Swarms
* **Blurb**:
  > An open-source Rust project that addresses Git's single-working-tree limitation when running multiple AI coding agents concurrently. Draft uses APFS/Btrfs CoW reflinks to spawn isolated workspaces in 0.06s with zero disk bloat, adds real-time collision radar to detect hot files, and features an open SKILL.md specification for agents like Claude Code and Cursor.

---

### C. This Week in Rust (TWiR)
* **Submit via GitHub PR**: [https://github.com/rust-lang/this-week-in-rust](https://github.com/rust-lang/this-week-in-rust)
* Add Draft to `Crate of the Week` or `Project Updates`:
  > [Draft (dft)](https://github.com/Pathomphong-i/draft) — An open-source multiverse version control system built with Rust (using SHA-256 CAS, `memmap2`, kernel reflinks, and Myers diff) designed for concurrent AI agent swarms.

---

### D. The Changelog (Weekly OSS Podcast & News)
* **Submit News**: [https://changelog.com/submit](https://changelog.com/submit) or ping @adamstac and @jerodsanto on X
* **Pitch**:
  > What happens to Git when code is written by swarms of autonomous AI agents rather than single humans? Check out Draft, a Rust VCS with instant CoW workspaces, collision radar, and in-memory predictive 3-way merges.

---

## 2. Discord & Community Chat Messages

### A. CrewAI Discord (`#showcase` or `#tools`)
```markdown
Hey everyone! 👋 If you've tried running multiple concurrent coding agents on a single repo, you've probably experienced git worktree bloat and painful merge collisions.

We open-sourced **Draft (`dft`)**, a Rust version control system with instant 0.06s Copy-on-Write dimensions, cross-agent collision radar, and a built-in `SKILL.md` manifest designed for agent fleets.

Check out the interactive web visualizer and repo:
- GitHub: https://github.com/Pathomphong-i/draft
- Web Demo: https://pathomphong-i.github.io/draft/
```

### B. LangChain & LangGraph Discord (`#share-your-work`)
```markdown
Hi all! Just launched an open-source tool that might interest folks building multi-agent software engineering swarms: **Draft (`dft`)**.

It provides isolated parallel dimensions with zero disk duplication via kernel CoW reflinks, advisory file leases so agents don't trample on each other, and in-memory pre-merge conflict prediction (`dft foresee`).

GitHub: https://github.com/Pathomphong-i/draft
Demo: https://pathomphong-i.github.io/draft/
```

### C. Cursor Community Forum (`#showcase`)
```markdown
### Using Draft with Cursor Composer for Parallel Features

Hey Cursor community! We built Draft (`dft`), a version control system in Rust designed to run multiple parallel drafts concurrently with zero disk bloat (0.06s CoW dimensions).

It includes a native `SKILL.md` you can reference directly with `@SKILL.md` in Composer to have Cursor follow safe multi-dimension workflows (claim files, simulate 3-way merges with `dft foresee`, and cleanly converge).

Repo: https://github.com/Pathomphong-i/draft
Live Web Playground: https://pathomphong-i.github.io/draft/
```
