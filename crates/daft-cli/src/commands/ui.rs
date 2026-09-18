//! Gitea-inspired DarftMultiverse Web Platform ('drf ui' / 'dft ui').
//! Self-hosted, lightweight, high-performance web interface for parallel version control.

use crate::cli::UiArgs;
use crate::error::CliError;
use daft_core::{DaftIgnore, Repository};
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
                "Fatal: not inside a Darf repository. Run 'drf init' first.".into(),
            ));
        }
    };

    let repo_root = repo
        .workdir()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| repo.dft_dir().to_path_buf());

    let host = if args.host.is_empty() { "127.0.0.1" } else { &args.host };
    let port = args.port;
    let listener = match TcpListener::bind(format!("{}:{}", host, port)) {
        Ok(l) => l,
        Err(_) => TcpListener::bind(format!("{}:0", host))?,
    };

    let local_addr = listener.local_addr()?;
    let server_url = if host == "0.0.0.0" {
        format!("http://127.0.0.1:{}", local_addr.port())
    } else {
        format!("http://{}", local_addr)
    };

    println!("============================================================");
    println!("🌌 DAFTMULTIVERSE WEB PLATFORM OPERATIONAL");
    println!("============================================================");
    println!("Dashboard:        {}", server_url);
    println!("Core Product:     Daft (command: 'dft')");
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

    let exe_path = env::current_exe().unwrap_or_else(|_| PathBuf::from("drf"));

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
    let mut buffer = [0u8; 32768];
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
    let raw_uri = parts.next().unwrap_or("/");

    let uri_parts: Vec<&str> = raw_uri.split('?').collect();
    let path_part = uri_parts.first().copied().unwrap_or("/");
    let query_part = uri_parts.get(1).copied().unwrap_or("");

    if (method == "GET" || method == "HEAD") && (path_part == "/" || path_part == "/index.html") {
        let html = render_dashboard_html(repo_root);
        send_response(&mut stream, 200, "text/html; charset=utf-8", if method == "HEAD" { &[] } else { html.as_bytes() });
    } else if method == "GET" && path_part == "/api/state" {
        let state_json = get_repository_state_json(repo_root);
        send_response(&mut stream, 200, "application/json", state_json.as_bytes());
    } else if method == "GET" && path_part == "/api/tree" {
        let rel_path = extract_query_param(query_part, "path").unwrap_or("");
        let tree_json = get_tree_json(repo_root, rel_path);
        send_response(&mut stream, 200, "application/json", tree_json.as_bytes());
    } else if method == "GET" && path_part == "/api/blob" {
        let rel_path = extract_query_param(query_part, "path").unwrap_or("");
        let blob_json = get_blob_json(repo_root, rel_path);
        send_response(&mut stream, 200, "application/json", blob_json.as_bytes());
    } else if method == "GET" && path_part == "/api/commits" {
        let commits_json = get_commits_json(exe_path, repo_root);
        send_response(&mut stream, 200, "application/json", commits_json.as_bytes());
    } else if method == "GET" && path_part == "/api/commit" {
        let hash = extract_query_param(query_part, "hash").unwrap_or("");
        let commit_json = get_commit_diff_json(exe_path, repo_root, hash);
        send_response(&mut stream, 200, "application/json", commit_json.as_bytes());
    } else if method == "GET" && path_part == "/api/diff" {
        let base = extract_query_param(query_part, "base").unwrap_or("");
        let target = extract_query_param(query_part, "target").unwrap_or("");
        let diff_json = get_diff_json(exe_path, repo_root, base, target);
        send_response(&mut stream, 200, "application/json", diff_json.as_bytes());
    } else if method == "GET" && path_part == "/api/radar" {
        let radar_json = get_radar_json(exe_path, repo_root);
        send_response(&mut stream, 200, "application/json", radar_json.as_bytes());
    } else if (method == "GET" || method == "HEAD") && (
        path_part.ends_with(".jpg") ||
        path_part.ends_with(".jpeg") ||
        path_part.ends_with(".png") ||
        path_part.ends_with(".svg") ||
        path_part.ends_with(".webp") ||
        path_part.ends_with(".gif") ||
        path_part.starts_with("/docs/assets/") ||
        path_part.starts_with("/assets/")
    ) {
        let clean_path = path_part.trim_start_matches('/');
        let mut candidate = repo_root.join(clean_path);
        if !candidate.exists() && clean_path.starts_with("assets/") {
            candidate = repo_root.join("docs").join(clean_path);
        }
        if !candidate.exists() && clean_path.contains("daft_multiverse_3d") {
            candidate = repo_root.join("docs").join("assets").join("daft_multiverse_3d.jpg");
        }
        if candidate.exists() && candidate.is_file() {
            if let Ok(bytes) = fs::read(&candidate) {
                let mime = if candidate.extension().map_or(false, |e| e == "svg") {
                    "image/svg+xml"
                } else if candidate.extension().map_or(false, |e| e == "png") {
                    "image/png"
                } else if candidate.extension().map_or(false, |e| e == "webp") {
                    "image/webp"
                } else if candidate.extension().map_or(false, |e| e == "gif") {
                    "image/gif"
                } else {
                    "image/jpeg"
                };
                send_response(&mut stream, 200, mime, if method == "HEAD" { &[] } else { &bytes });
                return;
            }
        }
        send_response(&mut stream, 404, "text/plain", b"Asset Not Found");
    } else if method == "POST" && path_part == "/api/exec" {
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

fn extract_query_param<'a>(query: &'a str, key: &str) -> Option<&'a str> {
    for pair in query.split('&') {
        let mut kv = pair.split('=');
        if let (Some(k), Some(v)) = (kv.next(), kv.next()) {
            if k == key {
                return Some(v);
            }
        }
    }
    None
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
    if parts.first().map(|s| *s == "dft" || *s == "drf").unwrap_or(false) {
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
            stderr: format!("Failed to spawn command: {}", e),
            exit_code: 127,
            success: false,
        },
    }
}

fn get_tree_json(repo_root: &Path, rel_path: &str) -> String {
    let target_dir = if rel_path.is_empty() || rel_path == "." {
        repo_root.to_path_buf()
    } else {
        repo_root.join(rel_path.replace("%2F", "/"))
    };

    let ignore = DaftIgnore::load_from_workdir(repo_root);
    let mut entries = Vec::new();

    if let Ok(dir_entries) = fs::read_dir(&target_dir) {
        for entry in dir_entries.flatten() {
            let file_name = entry.file_name().to_string_lossy().to_string();
            if ignore.should_prune_dir(&file_name) {
                continue;
            }

            let file_type = entry.file_type();
            let is_dir = file_type.as_ref().map(|t| t.is_dir()).unwrap_or(false);
            let size = entry.metadata().map(|m| m.len()).unwrap_or(0);

            let item_rel = if rel_path.is_empty() {
                file_name.clone()
            } else {
                format!("{}/{}", rel_path.trim_end_matches('/'), file_name)
            };

            entries.push(serde_json::json!({
                "name": file_name,
                "path": item_rel,
                "is_dir": is_dir,
                "size": size,
            }));
        }
    }

    entries.sort_by(|a, b| {
        let a_dir = a.get("is_dir").and_then(|v| v.as_bool()).unwrap_or(false);
        let b_dir = b.get("is_dir").and_then(|v| v.as_bool()).unwrap_or(false);
        if a_dir == b_dir {
            let a_name = a.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let b_name = b.get("name").and_then(|v| v.as_str()).unwrap_or("");
            a_name.cmp(b_name)
        } else if a_dir {
            std::cmp::Ordering::Less
        } else {
            std::cmp::Ordering::Greater
        }
    });

    serde_json::json!({
        "current_path": rel_path,
        "entries": entries,
    }).to_string()
}

fn get_blob_json(repo_root: &Path, rel_path: &str) -> String {
    let clean_path = rel_path.replace("%2F", "/");
    let full_path = repo_root.join(&clean_path);

    if !full_path.exists() || !full_path.is_file() {
        return serde_json::json!({
            "error": "File not found"
        }).to_string();
    }

    match fs::read_to_string(&full_path) {
        Ok(content) => {
            let lines = content.lines().count();
            let size = content.len();
            serde_json::json!({
                "path": clean_path,
                "size": size,
                "lines": lines,
                "content": content,
            }).to_string()
        }
        Err(_) => serde_json::json!({
            "path": clean_path,
            "binary": true,
            "error": "Binary file format not previewable as text"
        }).to_string(),
    }
}

fn get_commits_json(exe_path: &Path, repo_root: &Path) -> String {
    let res = execute_dft_command(exe_path, repo_root, "log");
    let mut commits = Vec::new();

    let mut current_commit: Option<serde_json::Map<String, serde_json::Value>> = None;
    let mut msg_lines = Vec::new();

    for line in res.stdout.lines() {
        if line.starts_with("commit ") {
            if let Some(mut map) = current_commit.take() {
                map.insert("message".into(), msg_lines.join("\n").trim().into());
                commits.push(serde_json::Value::Object(map));
                msg_lines.clear();
            }
            let hash = line.trim_start_matches("commit ").trim().to_string();
            let mut map = serde_json::Map::new();
            map.insert("hash".into(), hash.into());
            current_commit = Some(map);
        } else if let Some(ref mut map) = current_commit {
            if line.starts_with("Author:") {
                let author = line.trim_start_matches("Author:").trim().to_string();
                map.insert("author".into(), author.into());
            } else if line.starts_with("Date:") {
                let date = line.trim_start_matches("Date:").trim().to_string();
                map.insert("date".into(), date.into());
            } else if line.starts_with("    ") {
                msg_lines.push(line.trim().to_string());
            }
        }
    }

    if let Some(mut map) = current_commit.take() {
        map.insert("message".into(), msg_lines.join("\n").trim().into());
        commits.push(serde_json::Value::Object(map));
    }

    serde_json::json!({
        "commits": commits
    }).to_string()
}

fn get_commit_diff_json(exe_path: &Path, repo_root: &Path, hash: &str) -> String {
    let show_res = execute_dft_command(exe_path, repo_root, &format!("show {}", hash));
    serde_json::json!({
        "hash": hash,
        "output": show_res.stdout,
        "error": show_res.stderr,
        "success": show_res.success
    }).to_string()
}

fn get_diff_json(exe_path: &Path, repo_root: &Path, base: &str, target: &str) -> String {
    let cmd = if base.is_empty() && target.is_empty() {
        "diff".to_string()
    } else if target.is_empty() {
        format!("diff {}", base)
    } else {
        format!("diff {} {}", base, target)
    };
    let diff_res = execute_dft_command(exe_path, repo_root, &cmd);
    serde_json::json!({
        "diff": diff_res.stdout,
        "error": diff_res.stderr,
        "success": diff_res.success
    }).to_string()
}

fn get_radar_json(exe_path: &Path, repo_root: &Path) -> String {
    let radar_res = execute_dft_command(exe_path, repo_root, "radar --hot");
    let foresee_res = execute_dft_command(exe_path, repo_root, "foresee");
    let entropy_res = execute_dft_command(exe_path, repo_root, "entropy");
    serde_json::json!({
        "radar": radar_res.stdout,
        "foresee": foresee_res.stdout,
        "entropy": entropy_res.stdout
    }).to_string()
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
            "strategy": "3-way recursive convergence"
        }
    }).to_string()
}

fn render_dashboard_html(repo_root: &Path) -> String {
    let cur_dim = get_active_dimension(repo_root);
    let readme_content = fs::read_to_string(repo_root.join("README.md")).unwrap_or_default();
    let escaped_readme = serde_json::to_string(&readme_content).unwrap_or_else(|_| "\"\"\"".into());
    let template = include_str!("dashboard.html");
    template
        .replace("__ACTIVE_DIM__", &cur_dim)
        .replace("__README_JSON__", &escaped_readme)
}
