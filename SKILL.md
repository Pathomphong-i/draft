---
name: darf-vcs
description: Operational guide and multi-agent protocol for Darf ('drf' / 'dft'), the high-performance parallel version control system designed for AI agent swarms and human-agent collaboration.
---

# Darf (`drf`) Multi-Agent VCS Skill

## 1. Conceptual Model: From Git 1D to Darf Multiverse

Traditional Git enforces a **single universe, single timeline**:
- Only one branch can be checked out at a time in a working tree.
- If multiple AI agents work concurrently, they overwrite `HEAD`, collide on files, corrupt uncommitted work, and block each other.

**Darf (`drf`)** is built for **Parallel Swarm Concurrency**:
- **Dimensions (`drf dimension`)**: Isolated parallel workspaces existing simultaneously. Each dimension is an independent working tree with its own index and branch, powered by OS-native Copy-on-Write (macOS APFS `clonefile` / Linux `FICLONE`) sharing a lock-free SHA-256 Content-Addressable Object Store (`.dft/objects/`).
- **Radar (`drf radar`)**: Cross-dimensional awareness showing which files are being modified by other agents across all dimensions in real time.
- **Foresee (`drf foresee`)**: Predictive conflict detection that simulates 3-way merges in memory *before* merging by analyzing overlapping edits.
- **Territory (`drf claim`, `drf fence`)**: Claim-based advisory file leases and boundary fences so agents do not overwrite shared files.
- **Entanglement (`drf entangle`)**: Continuous bi-directional synchronization rules propagating file changes between selected dimensions.
- **Cronos (`drf cronos`)**: Background daemon providing autonomous 3-way tree reconciliation.
- **Convergence (`drf collapse`, `drf converge`)**: Reconciliation engine merging parallel dimension timelines back into mainline.
- **Agent Fleet (`drf agent`)**: Worker registry and Maildir-style asynchronous mailbox queue for direct agent-to-agent coordination.

---

## 2. Standard Autonomous Agent Protocol

Every AI agent working on a Darf repository **MUST** follow this 8-step lifecycle:

```
┌────────────────────────────────────────────────────────┐
│ 1. Register:   drf agent register agent-alpha --type ai│
│ 2. Dimension:  drf dimension create agent-alpha/task-1 │
│ 3. Radar:      drf radar --hot                         │
│ 4. Claim:      drf claim src/core/engine.rs            │
│ 5. Code & Save:drf add . && drf commit -m "feat: ..."  │
│ 6. Foresee:    drf foresee agent-alpha/task-1 mainline │
│ 7. Converge:   drf converge agent-alpha/task-1 mainline│
│ 8. Yield:      drf yield src/core/engine.rs            │
└────────────────────────────────────────────────────────┘
```

### Step 1: Register Your Agent Identity & Heartbeat
Before executing tasks, register yourself in the repository:
```bash
drf agent register <agent_id> --type ai
drf heartbeat <agent_id>
```

### Step 2: Spawn Your Own Dimension
Never work directly in `mainline` alongside other agents. Spawn an isolated parallel dimension:
```bash
# Creates an isolated CoW workspace instantly (< 0.06s)
drf dimension create <agent_id>/<task_name>
drf dimension enter <agent_id>/<task_name>
```

### Step 3: Inspect Radar Before Modifying Files
Always scan the radar to see if other dimensions are touching the same files:
```bash
# View active modifications across all dimensions
drf radar

# Check for collision hot-zones (files touched in 2+ dimensions)
drf radar --hot
```

### Step 4: Claim Your File Territory
Declare advisory ownership of the files you will modify to prevent collisions:
```bash
# Claim exclusive advisory ownership
drf claim src/module_a.rs

# Or place a boundary fence across all dimensions
drf fence src/critical_config.toml

# Audit active claims
drf territory
```

### Step 5: Implement, Stage, and Commit
Write code normally. Commits in your dimension are isolated and do not disrupt any other agents:
```bash
drf status
drf add src/module_a.rs
drf commit -m "feat(module_a): implement vector optimization"
```

### Step 6: Predictive Conflict Detection (Foresee)
Before merging back, verify that your timeline has no conflicting hunks:
```bash
# Predict conflicts between your dimension and mainline
drf foresee <your_dimension> mainline

# Calculate timeline divergence metric (0.0 = identical, 1.0 = high divergence)
drf entropy
```

### Step 7: Converge Timelines & Release Territory
When your feature is complete and verified:
```bash
# Return to mainline
drf dimension enter mainline

# Option A: Converge your specific dimension into mainline
drf converge <your_dimension> mainline

# Option B: Multi-branch collapse (collapse all active work into mainline)
drf collapse

# Release your file claims
drf yield src/module_a.rs

# Clean up your ephemeral dimension
drf dimension destroy <your_dimension>
```

---

## 3. Multi-Agent Swarm Communication & Recipes

### Recipe A: Agent-to-Agent Direct Mailbox
AI agents can send structured messages and coordination payloads directly through Darf without external network dependencies:
```bash
# Send a message to another agent's mailbox
drf agent send agent-beta "API types updated in shared/types/api.rs"

# Read messages in your own mailbox
drf agent read agent-alpha
```

### Recipe B: Continuous Entanglement (Pair Programming)
When two agents are working on tightly coupled features (e.g. Frontend Agent & API Agent):
```bash
# Entangle dimension-api and dimension-frontend on shared API types
drf entangle dimension-api dimension-frontend --paths "shared/types/*"
```
Whenever Agent 1 updates `shared/types/api.rs`, Darf automatically propagates the change into Agent 2's workspace in real time.

### Recipe C: Autonomous Background Synchronization (Cronos)
For swarms of 3+ agents working concurrently, activate Cronos:
```bash
# Start background sync daemon with 3-way tree convergence
drf cronos start

# Inspect sync history and auto-resolutions
drf cronos log
```

### Recipe D: Non-Destructive Dimension Inspection
Peek into another agent's dimension without switching branches or disrupting your working tree:
```bash
# View files in another dimension directly
drf observe dimension-beta src/engine.rs

# Diff two other dimensions from your current vantage point
drf diff dimension-alpha..dimension-beta
```

### Recipe E: Deploying to DaftUniverse Remote
Push your commits and dimensions to the shared DaftUniverse origin:
```bash
# Add origin remote
drf remote add origin /path/to/DaftUniverse

# Push dimension or branch
drf push origin main
drf push origin feature-dimension
```

---

## 4. Safety Invariants for AI Agents

1. **NEVER touch fenced paths**: If `drf territory` shows a path has a hard fence owned by another dimension, DO NOT edit that file until the fence is lifted.
2. **Always check `drf radar --hot`**: If a file is in a hot zone, coordinate with the other agent using `drf agent send <id> "<msg>"` or check `drf foresee`.
3. **Keep dimensions ephemeral**: Create dimensions for discrete tasks, converge them when done, and destroy them to preserve workspace hygiene.
4. **Use CoW Workspaces**: Darf automatically uses kernel-level copy-on-write. Never manually copy entire repositories into subfolders.

