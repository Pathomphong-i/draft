//! Real-time Multiverse Web GUI Server for Daft ('dft ui').

use crate::cli::UiArgs;
use crate::error::CliError;
use daft_core::Repository;
use std::env;
use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::Command;

pub fn execute(args: UiArgs) -> Result<(), CliError> {
    let cwd = env::current_dir()?;
    let repo = match Repository::discover(&cwd) {
        Ok(r) => r,
        Err(_) => {
            return Err(CliError::General(
                "Fatal: not inside a Daft repository. Run 'dft init' first.".into(),
            ));
        }
    };

    let repo_root = repo
        .workdir()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| repo.dft_dir().to_path_buf());

    let port = args.port;
    let listener = match TcpListener::bind(format!("127.0.0.1:{}", port)) {
        Ok(l) => l,
        Err(_) => TcpListener::bind("127.0.0.1:0")?,
    };

    let local_addr = listener.local_addr()?;
    let server_url = format!("http://{}", local_addr);

    println!("============================================================");
    println!("🌌 DAFT MULTIVERSE REAL-TIME GUI OPERATIONAL");
    println!("============================================================");
    println!("Dashboard:        {}", server_url);
    println!("Repository:       {}", repo_root.display());
    println!("Active Dimension: {}", get_active_dimension(&repo_root));
    println!("Press Ctrl+C to terminate GUI server.");
    println!("============================================================");

    if !args.no_browser {
        #[cfg(target_os = "macos")]
        let _ = Command::new("open").arg(&server_url).spawn();
        #[cfg(target_os = "linux")]
        let _ = Command::new("xdg-open").arg(&server_url).spawn();
        #[cfg(target_os = "windows")]
        let _ = Command::new("cmd").args(["/C", "start", &server_url]).spawn();
    }

    let exe_path = env::current_exe().unwrap_or_else(|_| PathBuf::from("dft"));

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let repo_root = repo_root.clone();
                let exe_path = exe_path.clone();
                std::thread::spawn(move || {
                    handle_connection(stream, &repo_root, &exe_path);
                });
            }
            Err(e) => {
                eprintln!("Connection error: {}", e);
            }
        }
    }

    Ok(())
}

fn get_active_dimension(repo_root: &Path) -> String {
    let cur_file = repo_root.join(".dft").join("current_dimension");
    if cur_file.exists() {
        fs::read_to_string(cur_file).unwrap_or_else(|_| "mainline".into()).trim().to_string()
    } else {
        "mainline".into()
    }
}

fn handle_connection(mut stream: TcpStream, repo_root: &Path, exe_path: &Path) {
    let mut buffer = [0u8; 8192];
    let n = match stream.read(&mut buffer) {
        Ok(n) if n > 0 => n,
        _ => return,
    };

    let request_str = String::from_utf8_lossy(&buffer[..n]);
    let mut lines = request_str.lines();
    let request_line = match lines.next() {
        Some(l) => l,
        None => return,
    };

    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or("");
    let uri = parts.next().unwrap_or("/");

    if method == "GET" && (uri == "/" || uri == "/index.html") {
        let html = render_dashboard_html(repo_root);
        send_response(&mut stream, 200, "text/html; charset=utf-8", html.as_bytes());
    } else if method == "GET" && uri == "/api/state" {
        let state_json = get_repository_state_json(repo_root);
        send_response(&mut stream, 200, "application/json", state_json.as_bytes());
    } else if method == "GET" && uri == "/assets/daft_multiverse_3d.jpg" {
        let img_path = repo_root.join("docs").join("assets").join("daft_multiverse_3d.jpg");
        if img_path.exists() {
            if let Ok(bytes) = fs::read(&img_path) {
                send_response(&mut stream, 200, "image/jpeg", &bytes);
                return;
            }
        }
        send_response(&mut stream, 404, "text/plain", b"Asset Not Found");
    } else if method == "POST" && uri == "/api/exec" {
        let body = match request_str.split("\r\n\r\n").nth(1) {
            Some(b) => b,
            None => request_str.split("\n\n").nth(1).unwrap_or(""),
        };

        let cmd_to_run: String = if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(body) {
            parsed.get("command").and_then(|v| v.as_str()).unwrap_or("status").to_string()
        } else {
            "status".into()
        };

        let result = execute_dft_command(exe_path, repo_root, &cmd_to_run);
        let resp_json = serde_json::to_string(&result).unwrap_or_else(|_| "{}".into());
        send_response(&mut stream, 200, "application/json", resp_json.as_bytes());
    } else {
        send_response(&mut stream, 404, "text/plain", b"Not Found");
    }
}

fn send_response(stream: &mut TcpStream, status_code: u16, content_type: &str, body: &[u8]) {
    let status_text = match status_code {
        200 => "OK",
        404 => "Not Found",
        500 => "Internal Server Error",
        _ => "Status",
    };
    let header = format!(
        "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nAccess-Control-Allow-Origin: *\r\nConnection: close\r\n\r\n",
        status_code,
        status_text,
        content_type,
        body.len()
    );
    let _ = stream.write_all(header.as_bytes());
    let _ = stream.write_all(body);
    let _ = stream.flush();
}

#[derive(serde::Serialize)]
struct CommandResult {
    command: String,
    stdout: String,
    stderr: String,
    exit_code: i32,
    success: bool,
}

fn execute_dft_command(exe_path: &Path, repo_root: &Path, cmd_str: &str) -> CommandResult {
    let mut parts: Vec<&str> = cmd_str.split_whitespace().collect();
    if parts.first().map(|s| *s == "dft").unwrap_or(false) {
        parts.remove(0);
    }

    let output = Command::new(exe_path)
        .args(&parts)
        .current_dir(repo_root)
        .output();

    match output {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout).to_string();
            let stderr = String::from_utf8_lossy(&out.stderr).to_string();
            let exit_code = out.status.code().unwrap_or(1);
            CommandResult {
                command: cmd_str.to_string(),
                stdout,
                stderr,
                exit_code,
                success: out.status.success(),
            }
        }
        Err(e) => CommandResult {
            command: cmd_str.to_string(),
            stdout: String::new(),
            stderr: format!("Failed to spawn dft: {}", e),
            exit_code: 127,
            success: false,
        },
    }
}

fn get_repository_state_json(repo_root: &Path) -> String {
    let dft_dir = repo_root.join(".dft");
    let cur_dim = get_active_dimension(repo_root);

    let mut dims: Vec<serde_json::Value> = Vec::new();
    let dim_dir = dft_dir.join("dimensions");
    dims.push(serde_json::json!({
        "name": "mainline",
        "branch": "main",
        "is_active": cur_dim == "mainline",
        "type": "root_universe"
    }));

    if dim_dir.is_dir() {
        if let Ok(entries) = fs::read_dir(&dim_dir) {
            for entry in entries.flatten() {
                if entry.path().is_dir() {
                    let name = entry.file_name().to_string_lossy().to_string();
                    let meta_file = entry.path().join("meta.json");
                    let branch = if meta_file.exists() {
                        fs::read_to_string(meta_file)
                            .ok()
                            .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
                            .and_then(|v| v.get("branch").and_then(|b| b.as_str()).map(|s| s.to_string()))
                            .unwrap_or_else(|| name.clone())
                    } else {
                        name.clone()
                    };

                    dims.push(serde_json::json!({
                        "name": name,
                        "branch": branch,
                        "is_active": cur_dim == name,
                        "type": "parallel_dimension"
                    }));
                }
            }
        }
    }

    let mut agents: Vec<serde_json::Value> = Vec::new();
    let agents_dir = dft_dir.join("agents");
    if agents_dir.is_dir() {
        if let Ok(entries) = fs::read_dir(&agents_dir) {
            for entry in entries.flatten() {
                if entry.path().extension().map(|e| e == "json").unwrap_or(false) {
                    if let Ok(content) = fs::read_to_string(entry.path()) {
                        if let Ok(val) = serde_json::from_str::<serde_json::Value>(&content) {
                            agents.push(val);
                        }
                    }
                }
            }
        }
    }

    let claims: serde_json::Value = fs::read_to_string(dft_dir.join("territory").join("claims.json"))
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_else(|| serde_json::json!({}));
    let fences: serde_json::Value = fs::read_to_string(dft_dir.join("territory").join("fences.json"))
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_else(|| serde_json::json!([]));

    let remotes: serde_json::Value = fs::read_to_string(dft_dir.join("remotes.json"))
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_else(|| serde_json::json!({}));

    let cronos_running = dft_dir.join("cronos").join("daemon.pid").exists();

    serde_json::json!({
        "repository": repo_root.to_string_lossy(),
        "active_dimension": cur_dim,
        "dimensions": dims,
        "agents": agents,
        "territory": {
            "claims": claims,
            "fences": fences
        },
        "remotes": remotes,
        "cronos": {
            "running": cronos_running,
            "strategy": "3-way recursive"
        }
    }).to_string()
}

fn render_dashboard_html(repo_root: &Path) -> String {
    let cur_dim = get_active_dimension(repo_root);
    format!(r##"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Daft Multiverse — Real-Time VCS Hub</title>
    <style>
        :root {{
            --bg: #0b0f19;
            --card-bg: rgba(18, 24, 38, 0.85);
            --card-border: #1e293b;
            --accent: #38bdf8;
            --accent-glow: rgba(56, 189, 248, 0.3);
            --quantum-violet: #a855f7;
            --quantum-glow: rgba(168, 85, 247, 0.35);
            --text: #f1f5f9;
            --text-dim: #94a3b8;
            --success: #10b981;
            --danger: #ef4444;
            --warning: #f59e0b;
            --code-bg: #050811;
        }}
        * {{ box-sizing: border-box; margin: 0; padding: 0; font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, "Helvetica Neue", sans-serif; }}
        body {{ background: var(--bg); color: var(--text); min-height: 100vh; overflow-x: hidden; }}
        
        header {{
            background: rgba(11, 15, 25, 0.95);
            backdrop-filter: blur(12px);
            border-bottom: 1px solid var(--card-border);
            padding: 1rem 2rem;
            display: flex;
            justify-content: space-between;
            align-items: center;
            position: sticky;
            top: 0;
            z-index: 100;
        }}
        .brand {{ display: flex; align-items: center; gap: 0.75rem; }}
        .brand-logo {{
            width: 36px; height: 36px;
            background: linear-gradient(135deg, var(--accent), var(--quantum-violet));
            border-radius: 8px;
            display: flex; align-items: center; justify-content: center;
            font-weight: 900; font-size: 1.1rem; color: #fff;
            box-shadow: 0 0 15px var(--accent-glow);
        }}
        .brand h1 {{ font-size: 1.25rem; font-weight: 700; letter-spacing: -0.025em; }}
        .brand span {{ color: var(--accent); font-size: 0.85rem; font-weight: 500; margin-left: 0.5rem; border: 1px solid var(--accent); padding: 0.15rem 0.5rem; border-radius: 9999px; }}

        .active-dim-badge {{
            display: flex; align-items: center; gap: 0.5rem;
            background: rgba(56, 189, 248, 0.1);
            border: 1px solid var(--accent);
            padding: 0.4rem 1rem;
            border-radius: 9999px;
            font-size: 0.9rem;
            color: var(--accent);
            font-weight: 600;
        }}
        .live-dot {{ width: 8px; height: 8px; background: var(--accent); border-radius: 50%; box-shadow: 0 0 8px var(--accent); animation: pulse 2s infinite; }}
        @keyframes pulse {{ 0%, 100% {{ opacity: 1; }} 50% {{ opacity: 0.3; }} }}

        main {{ padding: 2rem; max-width: 1600px; margin: 0 auto; display: grid; grid-template-columns: 1fr 420px; gap: 1.5rem; }}

        .card {{
            background: var(--card-bg);
            border: 1px solid var(--card-border);
            border-radius: 12px;
            padding: 1.25rem;
            backdrop-filter: blur(10px);
            box-shadow: 0 8px 32px rgba(0, 0, 0, 0.3);
            margin-bottom: 1.5rem;
        }}
        .card-header {{ display: flex; justify-content: space-between; align-items: center; margin-bottom: 1rem; }}
        .card-title {{ font-size: 1rem; font-weight: 700; color: #fff; display: flex; align-items: center; gap: 0.5rem; }}

        /* Multiverse Timeline Visualizer */
        #timeline-canvas-container {{
            position: relative;
            width: 100%;
            height: 380px;
            background: radial-gradient(circle at 50% 50%, rgba(30, 41, 59, 0.5) 0%, rgba(5, 8, 17, 0.9) 100%);
            border-radius: 8px;
            overflow: hidden;
            border: 1px solid #1e293b;
        }}
        canvas {{ width: 100%; height: 100%; display: block; }}

        /* Quick Action Bar */
        .quick-actions {{ display: flex; flex-wrap: wrap; gap: 0.5rem; margin-top: 1rem; }}
        .btn {{
            background: rgba(30, 41, 59, 0.8);
            border: 1px solid var(--card-border);
            color: var(--text);
            padding: 0.45rem 0.85rem;
            border-radius: 6px;
            font-size: 0.85rem;
            font-weight: 600;
            cursor: pointer;
            transition: all 0.2s;
            display: inline-flex; align-items: center; gap: 0.35rem;
        }}
        .btn:hover {{ background: var(--accent); color: #000; box-shadow: 0 0 12px var(--accent-glow); transform: translateY(-1px); }}
        .btn-violet:hover {{ background: var(--quantum-violet); color: #fff; box-shadow: 0 0 12px var(--quantum-glow); }}
        .btn-success {{ border-color: var(--success); color: var(--success); }}
        .btn-success:hover {{ background: var(--success); color: #000; }}

        /* Interactive Terminal */
        .terminal-container {{
            background: var(--code-bg);
            border: 1px solid var(--card-border);
            border-radius: 8px;
            overflow: hidden;
            display: flex; flex-direction: column;
            height: 480px;
        }}
        .terminal-header {{
            background: #0f172a;
            padding: 0.5rem 1rem;
            display: flex; justify-content: space-between; align-items: center;
            font-size: 0.8rem; color: var(--text-dim);
            border-bottom: 1px solid var(--card-border);
        }}
        .terminal-output {{
            flex: 1;
            padding: 1rem;
            overflow-y: auto;
            font-family: "JetBrains Mono", "Fira Code", monospace;
            font-size: 0.85rem;
            color: #38bdf8;
            white-space: pre-wrap;
            line-height: 1.5;
        }}
        .terminal-input-bar {{
            display: flex;
            background: #090d16;
            border-top: 1px solid var(--card-border);
            padding: 0.5rem;
        }}
        .prompt-prefix {{
            padding: 0.5rem 0.75rem;
            font-family: monospace;
            color: var(--quantum-violet);
            font-weight: 700;
        }}
        .terminal-input {{
            flex: 1;
            background: transparent;
            border: none;
            color: #fff;
            font-family: monospace;
            font-size: 0.9rem;
            outline: none;
        }}

        /* Dimensions Grid */
        .dimension-list {{ display: flex; flex-direction: column; gap: 0.5rem; }}
        .dim-item {{
            background: rgba(15, 23, 42, 0.6);
            border: 1px solid var(--card-border);
            padding: 0.75rem;
            border-radius: 8px;
            display: flex; justify-content: space-between; align-items: center;
            transition: all 0.2s;
        }}
        .dim-item.active {{
            border-color: var(--accent);
            background: rgba(56, 189, 248, 0.08);
        }}
        .dim-info h4 {{ font-size: 0.9rem; margin-bottom: 0.2rem; }}
        .dim-info p {{ font-size: 0.75rem; color: var(--text-dim); }}

        /* 3D Concept preview */
        .concept-banner {{
            width: 100%;
            height: 140px;
            border-radius: 8px;
            object-fit: cover;
            border: 1px solid var(--card-border);
            margin-bottom: 1rem;
        }}
    </style>
</head>
<body>
    <header>
        <div class="brand">
            <div class="brand-logo">dft</div>
            <h1>Daft Multiverse Hub</h1>
            <span>v0.1.0 Quantum</span>
        </div>
        <div class="active-dim-badge">
            <div class="live-dot"></div>
            <span>Current Dimension: <strong id="active-dim-label">{}</strong></span>
        </div>
    </header>

    <main>
        <!-- Left Column: Multiverse Visualizer & Interactive Terminal -->
        <div class="left-col">
            <div class="card">
                <div class="card-header">
                    <div class="card-title">🌌 Real-Time Multiverse Timeline</div>
                    <div style="font-size: 0.8rem; color: var(--text-dim);">Live Multi-Branch Spacetime</div>
                </div>
                <div id="timeline-canvas-container">
                    <canvas id="timelineCanvas"></canvas>
                </div>
                <div class="quick-actions">
                    <button class="btn" onclick="runCmd('timeline')">📊 dft timeline</button>
                    <button class="btn" onclick="runCmd('radar --hot')">📡 dft radar --hot</button>
                    <button class="btn" onclick="runCmd('foresee')">🔮 dft foresee</button>
                    <button class="btn btn-violet" onclick="runCmd('entropy')">🌊 dft entropy</button>
                    <button class="btn" onclick="runCmd('territory')">🔒 dft territory</button>
                    <button class="btn btn-success" onclick="runCmd('status')">⚡ dft status</button>
                    <button class="btn" onclick="runCmd('push origin main')">🚀 dft push origin</button>
                </div>
            </div>

            <!-- Terminal -->
            <div class="card" style="padding: 0; overflow: hidden;">
                <div class="terminal-container">
                    <div class="terminal-header">
                        <span>⚡ Interactive Daft Terminal</span>
                        <span id="term-status" style="color: var(--success);">READY</span>
                    </div>
                    <div class="terminal-output" id="terminalOutput">Daft VCS v0.1.0 — Ready. Type commands below or click quick action buttons.</div>
                    <div class="terminal-input-bar">
                        <span class="prompt-prefix">dft$</span>
                        <input type="text" class="terminal-input" id="cmdInput" placeholder="Try: status, radar, foresee, dimension list, claim src/lib.rs..." autofocus>
                    </div>
                </div>
            </div>
        </div>

        <!-- Right Column: Control Panels -->
        <div class="right-col">
            <!-- 3D Multiverse Concept Art -->
            <div class="card">
                <div class="card-header">
                    <div class="card-title">🪐 3D Multiverse Architecture</div>
                </div>
                <img src="/assets/daft_multiverse_3d.jpg" alt="Daft 3D Architecture" class="concept-banner" onerror="this.style.display='none'">
                <p style="font-size: 0.8rem; color: var(--text-dim); line-height: 1.4;">
                    In traditional Git, developers and agents are trapped in a single 1D/2D timeline. In <strong>Daft</strong>, parallel dimensions exist across 3D spacetime with quantum entanglements and proactive radar.
                </p>
            </div>

            <!-- Dimensions Manager -->
            <div class="card">
                <div class="card-header">
                    <div class="card-title">🌌 Parallel Dimensions</div>
                    <button class="btn" style="padding: 0.2rem 0.5rem; font-size: 0.75rem;" onclick="promptNewDim()">+ New Dim</button>
                </div>
                <div class="dimension-list" id="dimList">
                    <div style="font-size: 0.8rem; color: var(--text-dim);">Loading dimensions...</div>
                </div>
            </div>

            <!-- Territory & Radar Map -->
            <div class="card">
                <div class="card-header">
                    <div class="card-title">📡 Radar & Territory Claims</div>
                </div>
                <div id="territoryInfo" style="font-size: 0.85rem; color: var(--text-dim); line-height: 1.6;">
                    Loading territory status...
                </div>
            </div>

            <!-- Cronos Sync Daemon -->
            <div class="card">
                <div class="card-header">
                    <div class="card-title">⏰ Cronos Autonomous Daemon</div>
                    <span id="cronos-badge" style="font-size: 0.75rem; padding: 0.2rem 0.5rem; border-radius: 4px; background: #1e293b;">STANDBY</span>
                </div>
                <p style="font-size: 0.8rem; color: var(--text-dim); margin-bottom: 0.75rem;">
                    Background agent that continuously synchronizes dimensions with 3-way tree convergence.
                </p>
                <div style="display: flex; gap: 0.5rem;">
                    <button class="btn btn-success" style="flex: 1;" onclick="runCmd('cronos start')">Start Daemon</button>
                    <button class="btn" style="flex: 1;" onclick="runCmd('cronos stop')">Stop Daemon</button>
                </div>
            </div>
        </div>
    </main>

    <script>
        const terminalOutput = document.getElementById('terminalOutput');
        const cmdInput = document.getElementById('cmdInput');
        const dimList = document.getElementById('dimList');
        const territoryInfo = document.getElementById('territoryInfo');
        const activeDimLabel = document.getElementById('active-dim-label');

        async function fetchState() {{
            try {{
                const res = await fetch('/api/state');
                const data = await res.json();
                renderState(data);
            }} catch (e) {{
                console.error("State poll failed:", e);
            }}
        }}

        function renderState(state) {{
            activeDimLabel.textContent = state.active_dimension;

            // Render Dimensions List
            dimList.innerHTML = '';
            state.dimensions.forEach(d => {{
                const div = document.createElement('div');
                div.className = `dim-item ${{d.is_active ? 'active' : ''}}`;
                div.innerHTML = `
                    <div class="dim-info">
                        <h4>${{d.name}} ${{d.is_active ? '★' : ''}}</h4>
                        <p>Branch: ${{d.branch}} (${{d.type}})</p>
                    </div>
                    <div>
                        ${{!d.is_active ? `<button class="btn" style="padding: 0.2rem 0.5rem; font-size: 0.75rem;" onclick="runCmd('dimension enter ${{d.name}}')">Enter</button>` : '<span style="color: var(--accent); font-size: 0.8rem;">Current</span>'}}
                    </div>
                `;
                dimList.appendChild(div);
            }});

            // Render Territory
            const claims = Object.keys(state.territory.claims || {{}});
            const fences = state.territory.fences || [];
            territoryInfo.innerHTML = `
                <div><strong>Claimed Paths:</strong> ${{claims.length > 0 ? claims.map(c => `<code>${{c}}</code>`).join(', ') : 'None (Zero conflicts)'}}</div>
                <div style="margin-top: 0.4rem;"><strong>Hard Fences:</strong> ${{fences.length > 0 ? fences.join(', ') : 'None'}}</div>
                <div style="margin-top: 0.4rem;"><strong>DaftUniverse Remotes:</strong> ${{Object.keys(state.remotes || {{}}).length > 0 ? Object.entries(state.remotes).map(([k,v]) => `${{k}} &rarr; ${{v}}`).join('<br>') : 'Local universe'}}</div>
            `;

            drawTimeline(state.dimensions);
        }}

        function drawTimeline(dimensions) {{
            const canvas = document.getElementById('timelineCanvas');
            if (!canvas) return;
            const ctx = canvas.getContext('2d');
            const rect = canvas.parentElement.getBoundingClientRect();
            canvas.width = rect.width;
            canvas.height = rect.height;

            ctx.clearRect(0, 0, canvas.width, canvas.height);

            const startX = 60;
            const endX = canvas.width - 60;
            const count = Math.max(dimensions.length, 2);
            const gapY = (canvas.height - 80) / count;

            dimensions.forEach((d, idx) => {{
                const y = 50 + idx * gapY;

                // Dimension background glow plane
                const grad = ctx.createLinearGradient(startX, y, endX, y);
                if (d.is_active) {{
                    grad.addColorStop(0, 'rgba(56, 189, 248, 0.8)');
                    grad.addColorStop(1, 'rgba(168, 85, 247, 0.8)');
                }} else {{
                    grad.addColorStop(0, 'rgba(30, 41, 59, 0.7)');
                    grad.addColorStop(1, 'rgba(51, 65, 85, 0.7)');
                }}

                ctx.strokeStyle = grad;
                ctx.lineWidth = d.is_active ? 4 : 2;
                ctx.beginPath();
                ctx.moveTo(startX, y);
                ctx.lineTo(endX, y);
                ctx.stroke();

                // Branching curve from mainline if not root
                if (idx > 0) {{
                    ctx.strokeStyle = 'rgba(56, 189, 248, 0.35)';
                    ctx.setLineDash([4, 4]);
                    ctx.beginPath();
                    ctx.moveTo(startX + 40, 50);
                    ctx.bezierCurveTo(startX + 80, 50, startX + 60, y, startX + 120, y);
                    ctx.stroke();
                    ctx.setLineDash([]);
                }}

                // Draw commit nodes
                const nodePositions = [startX + 40, startX + 160, startX + 280, startX + 420, endX - 40];
                nodePositions.forEach((nx, nidx) => {{
                    if (nx > endX) return;
                    ctx.fillStyle = d.is_active ? '#38bdf8' : '#64748b';
                    ctx.beginPath();
                    ctx.arc(nx, y, d.is_active ? 6 : 4, 0, Math.PI * 2);
                    ctx.fill();

                    if (d.is_active && nidx === nodePositions.length - 1) {{
                        ctx.strokeStyle = '#fff';
                        ctx.lineWidth = 2;
                        ctx.beginPath();
                        ctx.arc(nx, y, 9, 0, Math.PI * 2);
                        ctx.stroke();
                    }}
                }});

                ctx.fillStyle = d.is_active ? '#38bdf8' : '#94a3b8';
                ctx.font = 'bold 12px monospace';
                ctx.fillText(d.name + (d.is_active ? ' [ACTIVE]' : ''), startX, y - 10);
            }});
        }}

        async function runCmd(cmd) {{
            terminalOutput.textContent += `\n$ dft ${{cmd}}\n`;
            terminalOutput.scrollTop = terminalOutput.scrollHeight;
            document.getElementById('term-status').textContent = 'EXECUTING...';

            try {{
                const res = await fetch('/api/exec', {{
                    method: 'POST',
                    headers: {{ 'Content-Type': 'application/json' }},
                    body: JSON.stringify({{ command: cmd }})
                }});
                const data = await res.json();
                if (data.stdout) terminalOutput.textContent += data.stdout;
                if (data.stderr) terminalOutput.textContent += `[ERR] ${{data.stderr}}`;
                terminalOutput.scrollTop = terminalOutput.scrollHeight;
                fetchState();
            }} catch (e) {{
                terminalOutput.textContent += `[ERROR] Failed to run command: ${{e}}\n`;
            }} finally {{
                document.getElementById('term-status').textContent = 'READY';
            }}
        }}

        cmdInput.addEventListener('keydown', (e) => {{
            if (e.key === 'Enter') {{
                const cmd = cmdInput.value.trim();
                if (cmd) {{
                    runCmd(cmd);
                    cmdInput.value = '';
                }}
            }}
        }});

        function promptNewDim() {{
            const name = prompt("Enter new dimension name (e.g., experiment-ai-1):");
            if (name) {{
                runCmd(`dimension create ${{name}}`);
            }}
        }}

        fetchState();
        setInterval(fetchState, 2000);
        window.addEventListener('resize', () => fetchState());
    </script>
</body>
</html>"##, cur_dim)
}
