---
name: daft-vcs
description: Operational guide and multi-agent protocol for Daft ('dft'), the high-performance parallel version control system designed for AI agent swarms and human-agent collaboration.
---

# Daft (`dft`) Multi-Agent VCS Skill

## 1. Conceptual Model: From Git 1D to Daft Multiverse

Traditional Git enforces a **single universe, single timeline**:
- Only one branch can be checked out at a time in a working tree.
- If multiple AI agents work concurrently, they overwrite `HEAD`, collide on files, corrupt uncommitted work, and block each other.

**Daft (`dft`)** is built for **Parallel Swarm Concurrency**:
- **Dimensions (`dft dimension`)**: Isolated parallel workspaces existing simultaneously. Each dimension is an independent working tree with its own index and branch, powered by OS-native Copy-on-Write (macOS APFS `clonefile` / Linux `FICLONE`) sharing a lock-free SHA-256 Content-Addressable Object Store (`.dft/objects/`).
- **Radar (`dft radar`)**: Cross-dimensional awareness showing which files are being modified by other agents across all dimensions in real time.
- **Foresee (`dft foresee`)**: Predictive conflict detection that simulates 3-way merges in memory *before* merging by analyzing overlapping edits.
- **Territory (`dft claim`, `dft fence`)**: Claim-based advisory file leases and boundary fences so agents do not overwrite shared files.
- **Entanglement (`dft entangle`)**: Continuous bi-directional synchronization rules propagating file changes between selected dimensions.
- **Cronos (`dft cronos`)**: Background daemon providing autonomous 3-way tree reconciliation.
- **Convergence (`dft collapse`, `dft converge`)**: Reconciliation engine merging parallel dimension timelines back into mainline.
- **Agent Fleet (`dft agent`)**: Worker registry and Maildir-style asynchronous mailbox queue for direct agent-to-agent coordination.

---

## 2. Standard Autonomous Agent Protocol

Every AI agent working on a Daft repository **MUST** follow this 8-step lifecycle:

```
┌────────────────────────────────────────────────────────┐
│ 1. Register:   dft agent register agent-alpha --type ai│
│ 2. Dimension:  dft dimension create agent-alpha/task-1 │
│ 3. Radar:      dft radar --hot                         │
│ 4. Claim:      dft claim src/core/engine.rs            │
│ 5. Code & Save:dft add . && dft commit -m "feat: ..."  │
│ 6. Foresee:    dft foresee agent-alpha/task-1 mainline │
│ 7. Converge:   dft converge agent-alpha/task-1 mainline│
│ 8. Yield:      dft yield src/core/engine.rs            │
└────────────────────────────────────────────────────────┘
```

### Step 1: Register Your Agent Identity & Heartbeat
Before executing tasks, register yourself in the repository:
```bash
dft agent register <agent_id> --type ai
dft heartbeat <agent_id>
```

### Step 2: Spawn Your Own Dimension
Never work directly in `mainline` alongside other agents. Spawn an isolated parallel dimension:
```bash
# Creates an isolated CoW workspace instantly (< 0.06s)
dft dimension create <agent_id>/<task_name>
dft dimension enter <agent_id>/<task_name>
```

### Step 3: Inspect Radar Before Modifying Files
Always scan the radar to see if other dimensions are touching the same files:
```bash
# View active modifications across all dimensions
dft radar

# Check for collision hot-zones (files touched in 2+ dimensions)
dft radar --hot
```

### Step 4: Claim Your File Territory
Declare advisory ownership of the files you will modify to prevent collisions:
```bash
# Claim exclusive advisory ownership
dft claim src/module_a.rs

# Or place a boundary fence across all dimensions
dft fence src/critical_config.toml

# Audit active claims
dft territory
```

### Step 5: Implement, Stage, and Commit
Write code normally. Commits in your dimension are isolated and do not disrupt any other agents:
```bash
dft status
dft add src/module_a.rs
dft commit -m "feat(module_a): implement vector optimization"
```

### Step 6: Predictive Conflict Detection (Foresee)
Before merging back, verify that your timeline has no conflicting hunks:
```bash
# Predict conflicts between your dimension and mainline
dft foresee <your_dimension> mainline

# Calculate timeline divergence metric (0.0 = identical, 1.0 = high divergence)
dft entropy
```

### Step 7: Converge Timelines & Release Territory
When your feature is complete and verified:
```bash
# Return to mainline
dft dimension enter mainline

# Option A: Converge your specific dimension into mainline
dft converge <your_dimension> mainline

# Option B: Multi-branch collapse (collapse all active work into mainline)
dft collapse

# Release your file claims
dft yield src/module_a.rs

# Clean up your ephemeral dimension
dft dimension destroy <your_dimension>
```

---

## 3. Multi-Agent Swarm Communication & Recipes

### Recipe A: Agent-to-Agent Direct Mailbox
AI agents can send structured messages and coordination payloads directly through Daft without external network dependencies:
```bash
# Send a message to another agent's mailbox
dft agent send agent-beta "API types updated in shared/types/api.rs"

# Read messages in your own mailbox
dft agent read agent-alpha
```

### Recipe B: Continuous Entanglement (Pair Programming)
When two agents are working on tightly coupled features (e.g. Frontend Agent & API Agent):
```bash
# Entangle dimension-api and dimension-frontend on shared API types
dft entangle dimension-api dimension-frontend --paths "shared/types/*"
```
Whenever Agent 1 updates `shared/types/api.rs`, Daft automatically propagates the change into Agent 2's workspace in real time.

### Recipe C: Autonomous Background Synchronization (Cronos)
For swarms of 3+ agents working concurrently, activate Cronos:
```bash
# Start background sync daemon with 3-way tree convergence
dft cronos start

# Inspect sync history and auto-resolutions
dft cronos log
```

### Recipe D: Non-Destructive Dimension Inspection
Peek into another agent's dimension without switching branches or disrupting your working tree:
```bash
# View files in another dimension directly
dft observe dimension-beta src/engine.rs

# Diff two other dimensions from your current vantage point
dft diff dimension-alpha..dimension-beta
```

### Recipe E: Deploying to DaftUniverse Remote
Push your commits and dimensions to the shared DaftUniverse origin:
```bash
# Add origin remote
dft remote add origin /path/to/DaftUniverse

# Push dimension or branch
dft push origin main
dft push origin feature-dimension
```

---

## 4. Safety Invariants for AI Agents

1. **NEVER touch fenced paths**: If `dft territory` shows a path has a hard fence owned by another dimension, DO NOT edit that file until the fence is lifted.
2. **Always check `dft radar --hot`**: If a file is in a hot zone, coordinate with the other agent using `dft agent send <id> "<msg>"` or check `dft foresee`.
3. **Keep dimensions ephemeral**: Create dimensions for discrete tasks, converge them when done, and destroy them to preserve workspace hygiene.
4. **Use CoW Workspaces**: Daft automatically uses kernel-level copy-on-write. Never manually copy entire repositories into subfolders.

