//! Gitea-inspired Self-Hosted Draft Server & Multi-Repository Web Platform ('dft server').
//!
//! Provides multi-user/org repository hosting, smart HTTP CAS wire protocol
//! ('dft clone', 'dft push', 'dft fetch', 'dft pull'), web code browsing & editing
//! directly on bare CAS stores, Issues, Convergence/Pull Requests with live 3-way
//! predictive conflict foresight ('merge_trees_3way'), Spacetime DAG visualization,
//! and AI Agent Swarm telemetry.

use crate::cli::ServerArgs;
use crate::commands::http_wire::{
    base64_decode, read_http_request, FetchPackRequest, FetchPackResponse, HasObjectsRequest,
    HasObjectsResponse, HttpRequest, PushPackRequest, PushPackResponse, WireObject,
};
use crate::commands::layer2::timeline::{
    graph::{
        MultiverseTimelineGraph, TimelineDimension, TimelineEdge, TimelineEdgeType, TimelineNode,
        TimelineNodeType,
    },
    render_ancestry_tree, render_dot, render_json, render_single_dimension, render_svg,
    render_terminal_graph,
};
use crate::error::CliError;
use chrono::Utc;
use daft_core::cas::{ObjectId, ObjectType, RawObject};
use daft_core::diff::{diff_trees, flatten_tree, format_unified_diff, EditOpKind};
use daft_core::graph::{all_merge_bases, is_ancestor, merge_base, walk_commits, StoreCommitGraph};
use daft_core::init::{init, InitOptions};
use daft_core::merge::{build_hierarchical_tree, merge_trees_3way};
use daft_core::object::{Commit, FileMode, Signature, Tree};
use daft_core::Repository;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::env;
use std::fs;
use std::io::Write;
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, RwLock};
use std::time::Instant;

const LOGO_SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 120 120" width="120" height="120">
  <defs>
    <linearGradient id="dftGrad" x1="0%" y1="0%" x2="100%" y2="100%">
      <stop offset="0%" stop-color="#58a6ff"/>
      <stop offset="55%" stop-color="#bc8cff"/>
      <stop offset="100%" stop-color="#3fb950"/>
    </linearGradient>
  </defs>
  <rect x="6" y="6" width="108" height="108" rx="26" fill="#0d1117" stroke="url(#dftGrad)" stroke-width="5"/>
  <path d="M36 86 V34 M36 58 C54 58, 56 38, 78 38 M36 62 C54 62, 56 82, 78 82" fill="none" stroke="url(#dftGrad)" stroke-width="7" stroke-linecap="round"/>
  <circle cx="36" cy="34" r="7" fill="#58a6ff"/>
  <circle cx="36" cy="86" r="7" fill="#58a6ff"/>
  <circle cx="80" cy="38" r="8" fill="#bc8cff"/>
  <circle cx="80" cy="82" r="8" fill="#3fb950"/>
  <circle cx="86" cy="60" r="6" fill="#f0f6fc"/>
  <path d="M80 38 L86 60 L80 82" fill="none" stroke="#f0f6fc" stroke-width="3" stroke-dasharray="4 3"/>
</svg>"##;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AccessToken {
    pub id: String,
    pub name: String,
    pub token_hash: String,
    pub prefix: String,
    pub created_at: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct UserRecord {
    pub username: String,
    pub display_name: String,
    pub email: String,
    pub password_hash: String,
    pub salt: String,
    pub is_admin: bool,
    pub avatar_color: String,
    pub created_at: String,
    #[serde(default)]
    pub tokens: Vec<AccessToken>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PublicUser {
    pub username: String,
    pub display_name: String,
    pub email: String,
    pub is_admin: bool,
    pub avatar_color: String,
    pub created_at: String,
}

impl From<&UserRecord> for PublicUser {
    fn from(u: &UserRecord) -> Self {
        Self {
            username: u.username.clone(),
            display_name: u.display_name.clone(),
            email: u.email.clone(),
            is_admin: u.is_admin,
            avatar_color: u.avatar_color.clone(),
            created_at: u.created_at.clone(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SessionRecord {
    pub token: String,
    pub username: String,
    pub created_at: i64,
    pub expires_at: i64,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct OrgRecord {
    pub name: String,
    pub display_name: String,
    pub description: String,
    pub creator: String,
    pub members: Vec<String>,
    pub created_at: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ActivityEvent {
    pub id: String,
    pub event_type: String, // "push", "create_repo", "issue", "pull", "merge", "fork"
    pub actor: String,
    pub owner: String,
    pub repo: String,
    pub ref_name: Option<String>,
    pub commit_id: Option<String>,
    pub summary: String,
    pub created_at: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct RepoMeta {
    pub owner: String,
    pub name: String,
    pub description: String,
    pub private: bool,
    pub default_branch: String,
    #[serde(default)]
    pub topics: Vec<String>,
    #[serde(default)]
    pub stars: Vec<String>,
    #[serde(default)]
    pub forks_count: usize,
    #[serde(default)]
    pub forked_from: Option<String>,
    #[serde(default)]
    pub migrated_from: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CommentItem {
    pub id: u64,
    pub author: String,
    pub body: String,
    pub created_at: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct IssueRecord {
    pub number: u64,
    pub title: String,
    pub body: String,
    pub author: String,
    pub state: String, // "open" | "closed"
    #[serde(default)]
    pub labels: Vec<String>,
    #[serde(default)]
    pub assignee: Option<String>,
    #[serde(default)]
    pub comments: Vec<CommentItem>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PullRecord {
    pub number: u64,
    pub title: String,
    pub body: String,
    pub author: String,
    pub head_ref: String,
    pub base_ref: String,
    pub state: String, // "open" | "merged" | "closed"
    #[serde(default)]
    pub merged_by: Option<String>,
    #[serde(default)]
    pub merge_commit: Option<String>,
    #[serde(default)]
    pub comments: Vec<CommentItem>,
    pub created_at: String,
    pub updated_at: String,
}

pub struct ServerState {
    pub data_dir: PathBuf,
    pub public_url: Option<String>,
    pub port: u16,
    pub started_at: Instant,
    pub users: RwLock<Vec<UserRecord>>,
    pub sessions: RwLock<Vec<SessionRecord>>,
    pub orgs: RwLock<Vec<OrgRecord>>,
    pub activity: RwLock<Vec<ActivityEvent>>,
    pub repo_lock: RwLock<()>,
}

impl ServerState {
    pub fn load_or_init(args: &ServerArgs) -> Result<Arc<Self>, CliError> {
        fs::create_dir_all(args.data_dir.join("repos"))?;
        let data_dir = args
            .data_dir
            .canonicalize()
            .unwrap_or_else(|_| args.data_dir.clone());

        let mut users: Vec<UserRecord> =
            read_json_file(&data_dir.join("users.json")).unwrap_or_default();
        let sessions: Vec<SessionRecord> =
            read_json_file(&data_dir.join("sessions.json")).unwrap_or_default();
        let mut orgs: Vec<OrgRecord> =
            read_json_file(&data_dir.join("orgs.json")).unwrap_or_default();
        let activity: Vec<ActivityEvent> =
            read_json_file(&data_dir.join("activity.json")).unwrap_or_default();

        if users.is_empty() {
            let admin_user = env::var("DFT_ADMIN_USER").unwrap_or_else(|_| "admin".to_string());
            let admin_pass = env::var("DFT_ADMIN_PASS").unwrap_or_else(|_| "draft123".to_string());
            let admin_name =
                env::var("DFT_ADMIN_NAME").unwrap_or_else(|_| "Draft Admin".to_string());
            let admin_email =
                env::var("DFT_ADMIN_EMAIL").unwrap_or_else(|_| "admin@draft-vcs.org".to_string());
            let salt = generate_token_hex(16);
            let password_hash = hash_password(&admin_pass, &salt);
            users.push(UserRecord {
                username: admin_user.clone(),
                display_name: admin_name,
                email: admin_email,
                password_hash,
                salt,
                is_admin: true,
                avatar_color: "#58a6ff".to_string(),
                created_at: Utc::now().to_rfc3339(),
                tokens: Vec::new(),
            });
            write_json_atomic(&data_dir.join("users.json"), &users)?;
        }

        if orgs.is_empty() {
            orgs.push(OrgRecord {
                name: "multiverse".to_string(),
                display_name: "Draft Multiverse Swarm".to_string(),
                description: "Autonomous AI Agent Swarm & Parallel Dimension Repositories"
                    .to_string(),
                creator: users[0].username.clone(),
                members: vec![users[0].username.clone()],
                created_at: Utc::now().to_rfc3339(),
            });
            write_json_atomic(&data_dir.join("orgs.json"), &orgs)?;
        }

        Ok(Arc::new(Self {
            data_dir,
            public_url: args.public_url.clone(),
            port: args.port,
            started_at: Instant::now(),
            users: RwLock::new(users),
            sessions: RwLock::new(sessions),
            orgs: RwLock::new(orgs),
            activity: RwLock::new(activity),
            repo_lock: RwLock::new(()),
        }))
    }

    pub fn repos_root(&self) -> PathBuf {
        self.data_dir.join("repos")
    }

    pub fn repo_dir(&self, owner: &str, repo: &str) -> PathBuf {
        let clean_repo = repo.trim_end_matches(".dft").trim_end_matches(".git");
        self.repos_root()
            .join(sanitize_slug(owner))
            .join(format!("{}.dft", sanitize_slug(clean_repo)))
    }

    pub fn base_url_for(&self, req: &HttpRequest) -> String {
        if let Some(ref pub_url) = self.public_url {
            return pub_url.trim_end_matches('/').to_string();
        }
        if let Some(host) = req
            .headers
            .get("x-forwarded-host")
            .or_else(|| req.headers.get("host"))
        {
            let proto = req
                .headers
                .get("x-forwarded-proto")
                .map(|s| s.as_str())
                .unwrap_or("http");
            return format!("{}://{}", proto, host);
        }
        format!("http://127.0.0.1:{}", self.port)
    }

    pub fn record_activity(&self, ev: ActivityEvent) {
        if let Ok(mut list) = self.activity.write() {
            list.insert(0, ev);
            if list.len() > 250 {
                list.truncate(250);
            }
            let _ = write_json_atomic(&self.data_dir.join("activity.json"), &*list);
        }
    }

    pub fn authenticate(&self, req: &HttpRequest) -> Option<UserRecord> {
        let now = Utc::now().timestamp();

        // 1. Check Authorization header (Bearer session/PAT or Basic user:pass/PAT)
        if let Some(auth_hdr) = req.headers.get("authorization") {
            if let Some(bearer) = auth_hdr
                .strip_prefix("Bearer ")
                .or_else(|| auth_hdr.strip_prefix("token "))
            {
                let tok = bearer.trim();
                if tok.starts_with("dft_pat_") {
                    let thash = sha256_hex(tok.as_bytes());
                    if let Ok(users) = self.users.read() {
                        for u in users.iter() {
                            if u.tokens.iter().any(|t| t.token_hash == thash) {
                                return Some(u.clone());
                            }
                        }
                    }
                } else if let Some(u) = self.lookup_session(tok, now) {
                    return Some(u);
                }
            } else if let Some(b64) = auth_hdr.strip_prefix("Basic ") {
                if let Some(decoded) = base64_decode(b64.trim()) {
                    let creds = String::from_utf8_lossy(&decoded);
                    if let Some((user_part, pass_part)) = creds.split_once(':') {
                        if pass_part.starts_with("dft_pat_") {
                            let thash = sha256_hex(pass_part.as_bytes());
                            if let Ok(users) = self.users.read() {
                                if let Some(u) = users.iter().find(|u| {
                                    u.username.eq_ignore_ascii_case(user_part)
                                        && u.tokens.iter().any(|t| t.token_hash == thash)
                                }) {
                                    return Some(u.clone());
                                }
                            }
                        }
                        if let Ok(users) = self.users.read() {
                            if let Some(u) = users
                                .iter()
                                .find(|u| u.username.eq_ignore_ascii_case(user_part))
                            {
                                if hash_password(pass_part, &u.salt) == u.password_hash {
                                    return Some(u.clone());
                                }
                            }
                        }
                    }
                }
            }
        }

        // 2. Check Cookie 'dft_session'
        if let Some(sess_tok) = req.cookie("dft_session") {
            if let Some(u) = self.lookup_session(&sess_tok, now) {
                return Some(u);
            }
        }

        None
    }

    fn lookup_session(&self, tok: &str, now: i64) -> Option<UserRecord> {
        let username = {
            let sessions = self.sessions.read().ok()?;
            sessions
                .iter()
                .find(|s| s.token == tok && s.expires_at > now)
                .map(|s| s.username.clone())?
        };
        let users = self.users.read().ok()?;
        users.iter().find(|u| u.username == username).cloned()
    }

    pub fn owner_exists(&self, owner: &str) -> bool {
        let slug = sanitize_slug(owner);
        if let Ok(users) = self.users.read() {
            if users.iter().any(|u| sanitize_slug(&u.username) == slug) {
                return true;
            }
        }
        if let Ok(orgs) = self.orgs.read() {
            if orgs.iter().any(|o| sanitize_slug(&o.name) == slug) {
                return true;
            }
        }
        false
    }
}

pub fn execute(args: ServerArgs) -> Result<(), CliError> {
    let state = ServerState::load_or_init(&args)?;
    let bind_addr = format!("{}:{}", args.host, args.port);
    let listener = TcpListener::bind(&bind_addr).map_err(|e| {
        CliError::General(format!(
            "Failed to bind Draft Server on {}: {}",
            bind_addr, e
        ))
    })?;

    let local_addr = listener.local_addr()?;
    let display_url = args.public_url.clone().unwrap_or_else(|| {
        if args.host == "0.0.0.0" {
            format!("http://127.0.0.1:{}", local_addr.port())
        } else {
            format!("http://{}", local_addr)
        }
    });

    println!("============================================================");
    println!("🌌 DRAFT MULTIVERSE — SELF-HOSTED SERVER + UI OPERATIONAL");
    println!("============================================================");
    println!("Web UI & API:     {}", display_url);
    println!("Agent SKILL.md:   {}/SKILL.md", display_url);
    println!("Bind Address:     {}", local_addr);
    println!("Data Directory:   {}", state.data_dir.display());
    println!("Version:          v{}", env!("CARGO_PKG_VERSION"));
    println!("============================================================");

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let st = Arc::clone(&state);
                std::thread::spawn(move || {
                    handle_client(stream, st);
                });
            }
            Err(e) => {
                eprintln!("Accept error: {}", e);
            }
        }
    }

    Ok(())
}

fn handle_client(mut stream: TcpStream, state: Arc<ServerState>) {
    let req = match read_http_request(&stream) {
        Some(r) => r,
        None => return,
    };

    if req.method == "OPTIONS" {
        send_response(&mut stream, 204, "text/plain", &[], None);
        return;
    }

    let path = req.path.as_str();

    // 1. Static Assets, Installer & Health
    if (req.method == "GET" || req.method == "HEAD") && path == "/assets/logo.svg" {
        send_response(
            &mut stream,
            200,
            "image/svg+xml",
            if req.method == "HEAD" {
                &[]
            } else {
                LOGO_SVG.as_bytes()
            },
            None,
        );
        return;
    }

    if (req.method == "GET" || req.method == "HEAD") && (path == "/SKILL.md" || path == "/skill.md")
    {
        let base = state.base_url_for(&req);
        let raw_skill = include_str!("../../../../SKILL.md");
        let customized = format!(
            "{}\n\n---\n\n## 5. Connected DraftMultiverse Server\n\n- **Server Endpoint**: `{}`\n- **Clone Multiverse**: `dft clone {}/<owner>/<multiverse>.dft`\n- **Push Active Dimension**: `dft push -u origin <dimension>`\n- **Push All Parallel Dimensions & Telemetry**: `dft push origin main --all`\n",
            raw_skill.trim(),
            base,
            base
        );
        send_response(
            &mut stream,
            200,
            "text/markdown; charset=utf-8",
            if req.method == "HEAD" {
                &[]
            } else {
                customized.as_bytes()
            },
            None,
        );
        return;
    }

    if (req.method == "GET" || req.method == "HEAD")
        && (path == "/install.sh" || path == "/install")
    {
        let base = state.base_url_for(&req);
        let script = render_dynamic_install_script(&base);
        send_response(
            &mut stream,
            200,
            "text/plain; charset=utf-8",
            if req.method == "HEAD" {
                &[]
            } else {
                script.as_bytes()
            },
            None,
        );
        return;
    }

    if req.method == "GET" && path == "/download/dft" {
        if let Ok(exe) = env::current_exe() {
            if let Ok(bytes) = fs::read(&exe) {
                send_response(&mut stream, 200, "application/octet-stream", &bytes, None);
                return;
            }
        }
        send_json_error(&mut stream, 404, "Binary not available");
        return;
    }

    if req.method == "GET" && (path == "/api/health" || path == "/api/v1/version") {
        let repos = list_all_repo_metas(&state, None);
        let users_cnt = state.users.read().map(|u| u.len()).unwrap_or(0);
        let orgs_cnt = state.orgs.read().map(|o| o.len()).unwrap_or(0);
        let body = serde_json::json!({
            "status": "ok",
            "service": "draft-multiverse-server",
            "paradigm": "kairos-multiverse-l2",
            "version": env!("CARGO_PKG_VERSION"),
            "os": env::consts::OS,
            "arch": env::consts::ARCH,
            "uptime_secs": state.started_at.elapsed().as_secs(),
            "repo_count": repos.len(),
            "user_count": users_cnt,
            "org_count": orgs_cnt,
        });
        send_json(&mut stream, 200, &body, None);
        return;
    }

    // 2. Draft Smart HTTP Wire Protocol: /<owner>/<repo>[.dft]/dft/<action>
    if let Some((owner, repo, wire_action)) = parse_wire_route(path) {
        handle_wire_protocol(&mut stream, &req, &state, &owner, &repo, &wire_action);
        return;
    }

    // 3. Raw Blob Streaming Route: /<owner>/<repo>/raw/<ref>/<filepath...>
    if req.method == "GET" || req.method == "HEAD" {
        if let Some((owner, repo, ref_and_path)) = parse_raw_route(path) {
            handle_raw_blob(&mut stream, &req, &state, &owner, &repo, &ref_and_path);
            return;
        }
    }

    // 4. REST API v1: /api/v1/...
    if path.starts_with("/api/v1/") {
        handle_api_v1(&mut stream, &req, &state);
        return;
    }

    // 5. SPA Fallback for Gitea-like Deep URLs (/, /explore, /<owner>, /<owner>/<repo>, etc.)
    if req.method == "GET" || req.method == "HEAD" {
        let html = render_server_ui_html(&state, &req);
        send_response(
            &mut stream,
            200,
            "text/html; charset=utf-8",
            if req.method == "HEAD" {
                &[]
            } else {
                html.as_bytes()
            },
            None,
        );
        return;
    }

    send_json_error(&mut stream, 404, "Endpoint not found");
}

fn parse_wire_route(path: &str) -> Option<(String, String, String)> {
    let segs: Vec<&str> = path.trim_matches('/').split('/').collect();
    if segs.len() == 4 && segs[2] == "dft" {
        let owner = segs[0].to_string();
        let repo = segs[1]
            .trim_end_matches(".dft")
            .trim_end_matches(".git")
            .to_string();
        let action = segs[3].to_string();
        return Some((owner, repo, action));
    }
    None
}

fn parse_raw_route(path: &str) -> Option<(String, String, String)> {
    let trimmed = path.trim_start_matches('/');
    let mut parts = trimmed.splitn(4, '/');
    let owner = parts.next()?;
    let repo = parts
        .next()?
        .trim_end_matches(".dft")
        .trim_end_matches(".git");
    let marker = parts.next()?;
    let rest = parts.next()?;
    if marker == "raw" && owner != "api" && !rest.is_empty() {
        return Some((owner.to_string(), repo.to_string(), rest.to_string()));
    }
    None
}

// ============================================================================
// Wire Protocol Handlers (dft clone / fetch / pull / push)
// ============================================================================

fn handle_wire_protocol(
    stream: &mut TcpStream,
    req: &HttpRequest,
    state: &Arc<ServerState>,
    owner: &str,
    repo_name: &str,
    action: &str,
) {
    let repo_dir = state.repo_dir(owner, repo_name);
    let user = state.authenticate(req);

    match (req.method.as_str(), action) {
        ("POST", "has-objects") => {
            let payload: HasObjectsRequest = match req.json() {
                Ok(p) => p,
                Err(e) => return send_json_error(stream, 400, &e),
            };
            if !repo_dir.join("HEAD").exists() {
                return send_json(
                    stream,
                    200,
                    &HasObjectsResponse {
                        missing: payload.oids,
                    },
                    None,
                );
            }
            let repo = match Repository::open(&repo_dir) {
                Ok(r) => r,
                Err(e) => return send_json_error(stream, 500, &e.to_string()),
            };
            let missing: Vec<String> = payload
                .oids
                .into_iter()
                .filter(|hex_id| {
                    ObjectId::from_hex(hex_id)
                        .map(|oid| !repo.cas().has_object(&oid))
                        .unwrap_or(true)
                })
                .collect();
            send_json(stream, 200, &HasObjectsResponse { missing }, None);
        }
        ("POST", "fetch-pack") => {
            if !repo_dir.join("HEAD").exists() {
                return send_json_error(
                    stream,
                    404,
                    &format!(
                        "Repository '{}/{}' does not exist on this Draft server",
                        owner, repo_name
                    ),
                );
            }
            let meta = load_repo_meta(&repo_dir, owner, repo_name);
            if meta.private && !can_access_private(&meta, user.as_ref(), state) {
                return send_json_error(
                    stream,
                    401,
                    "Authentication required for private repository",
                );
            }

            let payload: FetchPackRequest = req.json().unwrap_or_default();
            let have_set: HashSet<String> = payload.have.into_iter().collect();

            let repo = match Repository::open(&repo_dir) {
                Ok(r) => r,
                Err(e) => return send_json_error(stream, 500, &e.to_string()),
            };

            let head = fs::read_to_string(repo_dir.join("HEAD"))
                .unwrap_or_else(|_| format!("ref: refs/heads/{}\n", meta.default_branch))
                .trim()
                .to_string();

            let refs = collect_all_repo_refs(&repo);
            let mut objects = Vec::new();
            if let Ok(all_oids) = repo.cas().list_objects() {
                for oid in all_oids {
                    let hex_id = oid.to_hex();
                    if !have_set.contains(&hex_id) {
                        if let Ok(raw) = repo.cas().read_raw(&oid) {
                            objects.push(WireObject {
                                oid: hex_id,
                                object_type: raw.object_type.as_str().to_string(),
                                data_hex: hex::encode(&raw.data),
                            });
                        }
                    }
                }
            }

            let territory = load_territory_json(&repo_dir);
            let agents = load_agents_json(&repo_dir);
            let entanglements = load_entanglements_json(&repo_dir);

            send_json(
                stream,
                200,
                &FetchPackResponse {
                    head,
                    default_branch: meta.default_branch,
                    refs,
                    objects,
                    territory: Some(territory),
                    agents,
                    entanglements,
                },
                None,
            );
        }
        ("POST", "push-pack") => {
            let payload: PushPackRequest = match req.json() {
                Ok(p) => p,
                Err(e) => return send_json_error(stream, 400, &e),
            };

            let _guard = state.repo_lock.write().ok();

            // Auto-initialize bare repository on push if it does not exist yet
            let is_new_repo = !repo_dir.join("HEAD").exists();
            if is_new_repo {
                let default_br = if payload.branch.is_empty() {
                    "main".to_string()
                } else {
                    payload.branch.clone()
                };
                if let Err(e) = create_bare_repository(
                    state,
                    owner,
                    repo_name,
                    "Auto-created via dft push",
                    false,
                    &default_br,
                    false,
                    Vec::new(),
                    user.as_ref()
                        .map(|u| u.username.as_str())
                        .or(payload.pusher.as_deref())
                        .unwrap_or(owner),
                ) {
                    return send_json_error(stream, 500, &e);
                }
            }

            let mut meta = load_repo_meta(&repo_dir, owner, repo_name);
            if meta.private && !can_access_private(&meta, user.as_ref(), state) {
                return send_json_error(
                    stream,
                    401,
                    "Authentication required to push to private repository",
                );
            }

            let repo = match Repository::open(&repo_dir) {
                Ok(r) => r,
                Err(e) => return send_json_error(stream, 500, &e.to_string()),
            };

            let mut received = 0usize;
            for obj in &payload.objects {
                let bytes = match hex::decode(&obj.data_hex) {
                    Ok(b) => b,
                    Err(e) => {
                        return send_json_error(
                            stream,
                            400,
                            &format!("Invalid hex in object {}: {}", obj.oid, e),
                        )
                    }
                };
                let obj_type = match ObjectType::parse(&obj.object_type) {
                    Ok(t) => t,
                    Err(e) => return send_json_error(stream, 400, &e.to_string()),
                };
                let raw = RawObject::new(obj_type, bytes);
                if raw.compute_id().to_hex() != obj.oid {
                    return send_json_error(
                        stream,
                        400,
                        &format!("SHA-256 verification failed for object {}", obj.oid),
                    );
                }
                if let Err(e) = repo.cas().write_raw(&raw) {
                    return send_json_error(stream, 500, &format!("CAS write error: {}", e));
                }
                received += 1;
            }

            let mut updated_refs = Vec::new();
            let mut ref_map = payload.refs.clone();
            if ref_map.is_empty() && !payload.branch.is_empty() && !payload.commit_id.is_empty() {
                ref_map.insert(
                    format!("refs/heads/{}", payload.branch),
                    payload.commit_id.clone(),
                );
            }

            // Enforce server-side Hard Boundary Fences before updating refs
            for (ref_name, commit_hex) in &ref_map {
                let short_dim = ref_name
                    .strip_prefix("refs/dimensions/")
                    .or_else(|| ref_name.strip_prefix("refs/heads/"))
                    .unwrap_or(ref_name);
                if let Ok(new_oid) = ObjectId::from_hex(commit_hex.trim()) {
                    let old_oid = repo.refs().resolve(ref_name).ok();
                    if let Ok(new_raw) = repo.cas().read_raw(&new_oid) {
                        if let Ok(new_c) = Commit::deserialize(&new_raw.data) {
                            let base_oid = old_oid.or_else(|| new_c.parents.first().copied());
                            if let Some(b_oid) = base_oid {
                                if let Ok(b_raw) = repo.cas().read_raw(&b_oid) {
                                    if let Ok(b_c) = Commit::deserialize(&b_raw.data) {
                                        let mut bf = BTreeMap::new();
                                        let mut nf = BTreeMap::new();
                                        let _ = flatten_tree(
                                            repo.cas().as_ref(),
                                            &b_c.tree,
                                            "",
                                            &mut bf,
                                        );
                                        let _ = flatten_tree(
                                            repo.cas().as_ref(),
                                            &new_c.tree,
                                            "",
                                            &mut nf,
                                        );
                                        for p in nf.keys().chain(bf.keys()) {
                                            if bf.get(p).map(|x| x.1) != nf.get(p).map(|x| x.1) {
                                                if let Some(owner_dim) =
                                                    check_fence_violation(&repo_dir, p, short_dim)
                                                {
                                                    return send_json_error(
                                                        stream,
                                                        409,
                                                        &format!(
                                                            "FenceViolation: path '{}' modified on '{}' is protected by fence owned by '{}'",
                                                            p, short_dim, owner_dim
                                                        ),
                                                    );
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            for (ref_name, commit_hex) in &ref_map {
                if !ref_name.starts_with("refs/") || ref_name.contains("..") {
                    continue;
                }
                let ref_file = repo_dir.join(ref_name);
                if let Some(p) = ref_file.parent() {
                    let _ = fs::create_dir_all(p);
                }
                let _ = fs::write(&ref_file, format!("{}\n", commit_hex.trim()));
                updated_refs.push(ref_name.clone());
            }

            // Ensure HEAD points to a valid branch if default_branch doesn't exist yet
            let default_ref_file = repo_dir.join("refs/heads").join(&meta.default_branch);
            if !default_ref_file.exists() && !payload.branch.is_empty() {
                let pushed_head = repo_dir.join("refs/heads").join(&payload.branch);
                if pushed_head.exists() {
                    meta.default_branch = payload.branch.clone();
                    let _ = fs::write(
                        repo_dir.join("HEAD"),
                        format!("ref: refs/heads/{}\n", meta.default_branch),
                    );
                }
            }

            // Sync territory & swarm agents
            if let Some(terr) = payload.territory {
                let terr_dir = repo_dir.join("territory");
                let _ = fs::create_dir_all(&terr_dir);
                if let Some(claims) = terr.get("claims") {
                    let _ = write_json_atomic(&terr_dir.join("claims.json"), claims);
                }
                if let Some(fences) = terr.get("fences") {
                    let _ = write_json_atomic(&terr_dir.join("fences.json"), fences);
                }
            }
            if !payload.agents.is_empty() {
                let agents_dir = repo_dir.join("agents");
                let _ = fs::create_dir_all(&agents_dir);
                for a in &payload.agents {
                    if let Some(id) = a
                        .get("id")
                        .or_else(|| a.get("name"))
                        .and_then(|v| v.as_str())
                    {
                        let _ = write_json_atomic(
                            &agents_dir.join(format!("{}.json", sanitize_slug(id))),
                            a,
                        );
                    }
                }
            }
            if !payload.entanglements.is_empty() {
                let ent_dir = repo_dir.join("entangle");
                let _ = fs::create_dir_all(&ent_dir);
                let _ = write_json_atomic(&ent_dir.join("rules.json"), &payload.entanglements);
            }
            if !payload.branch.is_empty() {
                let _ = propagate_entanglements_from(&repo, &repo_dir, &payload.branch);
            }

            meta.updated_at = Utc::now().to_rfc3339();
            let _ = save_repo_meta(&repo_dir, &meta);

            let actor = user
                .as_ref()
                .map(|u| u.username.clone())
                .or(payload.pusher.clone())
                .unwrap_or_else(|| "anonymous".to_string());
            let short_sha = &payload.commit_id[..payload.commit_id.len().min(7)];

            state.record_activity(ActivityEvent {
                id: generate_token_hex(8),
                event_type: "push".to_string(),
                actor,
                owner: meta.owner.clone(),
                repo: meta.name.clone(),
                ref_name: Some(payload.branch.clone()),
                commit_id: Some(payload.commit_id.clone()),
                summary: format!(
                    "pushed {} ({} new objects) to {}",
                    short_sha, received, payload.branch
                ),
                created_at: Utc::now().to_rfc3339(),
            });

            let base_url = state.base_url_for(req);
            let web_url = format!("{}/{}/{}", base_url, meta.owner, meta.name);
            let compare_url = if !payload.branch.is_empty() && payload.branch != meta.default_branch
            {
                Some(format!(
                    "{}/{}/{}?tab=pulls&new=1&head={}&base={}",
                    base_url, meta.owner, meta.name, payload.branch, meta.default_branch
                ))
            } else {
                None
            };

            send_json(
                stream,
                200,
                &PushPackResponse {
                    ok: true,
                    message: "Push accepted".to_string(),
                    updated_refs,
                    objects_received: received,
                    web_url: Some(web_url),
                    compare_url,
                },
                None,
            );
        }
        _ => send_json_error(stream, 404, "Unknown wire endpoint"),
    }
}

// ============================================================================
// REST API v1 Router
// ============================================================================

fn handle_api_v1(stream: &mut TcpStream, req: &HttpRequest, state: &Arc<ServerState>) {
    let sub = req.path.trim_start_matches("/api/v1").trim_end_matches('/');
    let segs: Vec<&str> = sub
        .trim_start_matches('/')
        .split('/')
        .filter(|s| !s.is_empty())
        .collect();
    let user = state.authenticate(req);

    match (req.method.as_str(), segs.as_slice()) {
        // --- Auth & Users ---
        ("GET", ["auth", "me"]) => {
            let orgs = state.orgs.read().map(|o| o.clone()).unwrap_or_default();
            let tokens = user
                .as_ref()
                .map(|u| {
                    u.tokens
                        .iter()
                        .map(|t| {
                            serde_json::json!({
                                "id": t.id,
                                "name": t.name,
                                "prefix": t.prefix,
                                "created_at": t.created_at,
                            })
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            send_json(
                stream,
                200,
                &serde_json::json!({
                    "authenticated": user.is_some(),
                    "user": user.as_ref().map(PublicUser::from),
                    "tokens": tokens,
                    "orgs": orgs,
                }),
                None,
            );
        }
        ("POST", ["auth", "login"]) => {
            #[derive(Deserialize)]
            struct LoginReq {
                username: String,
                password: String,
            }
            let body: LoginReq = match req.json() {
                Ok(b) => b,
                Err(e) => return send_json_error(stream, 400, &e),
            };
            let found = {
                let users = match state.users.read() {
                    Ok(u) => u,
                    Err(_) => return send_json_error(stream, 500, "Lock poisoned"),
                };
                users
                    .iter()
                    .find(|u| {
                        u.username.eq_ignore_ascii_case(body.username.trim())
                            || u.email.eq_ignore_ascii_case(body.username.trim())
                    })
                    .cloned()
            };
            let Some(u) = found else {
                return send_json_error(stream, 401, "Invalid username or password");
            };
            if hash_password(&body.password, &u.salt) != u.password_hash {
                return send_json_error(stream, 401, "Invalid username or password");
            }
            let sess_tok = create_session(state, &u.username);
            let cookie = format!(
                "dft_session={}; Path=/; Max-Age=2592000; HttpOnly; SameSite=Lax",
                sess_tok
            );
            send_json(
                stream,
                200,
                &serde_json::json!({
                    "token": sess_tok,
                    "user": PublicUser::from(&u),
                }),
                Some(&cookie),
            );
        }
        ("POST", ["auth", "register"]) => {
            #[derive(Deserialize)]
            struct RegReq {
                username: String,
                display_name: Option<String>,
                email: Option<String>,
                password: String,
            }
            let body: RegReq = match req.json() {
                Ok(b) => b,
                Err(e) => return send_json_error(stream, 400, &e),
            };
            let uname = sanitize_slug(&body.username);
            if uname.len() < 2 {
                return send_json_error(stream, 400, "Username must be at least 2 characters");
            }
            if body.password.len() < 4 {
                return send_json_error(stream, 400, "Password must be at least 4 characters");
            }
            if state.owner_exists(&uname) {
                return send_json_error(stream, 409, "Username or organization already exists");
            }

            let colors = [
                "#58a6ff", "#bc8cff", "#3fb950", "#f78166", "#d29922", "#39d353",
            ];
            let salt = generate_token_hex(16);
            let password_hash = hash_password(&body.password, &salt);
            let new_user = {
                let mut users = match state.users.write() {
                    Ok(u) => u,
                    Err(_) => return send_json_error(stream, 500, "Lock error"),
                };
                let color = colors[users.len() % colors.len()].to_string();
                let u = UserRecord {
                    username: uname.clone(),
                    display_name: body
                        .display_name
                        .filter(|s| !s.trim().is_empty())
                        .unwrap_or_else(|| uname.clone()),
                    email: body
                        .email
                        .filter(|s| !s.trim().is_empty())
                        .unwrap_or_else(|| format!("{}@draft.local", uname)),
                    password_hash,
                    salt,
                    is_admin: users.is_empty(),
                    avatar_color: color,
                    created_at: Utc::now().to_rfc3339(),
                    tokens: Vec::new(),
                };
                users.push(u.clone());
                let _ = write_json_atomic(&state.data_dir.join("users.json"), &*users);
                u
            };
            let sess_tok = create_session(state, &uname);
            let cookie = format!(
                "dft_session={}; Path=/; Max-Age=2592000; HttpOnly; SameSite=Lax",
                sess_tok
            );
            send_json(
                stream,
                201,
                &serde_json::json!({
                    "token": sess_tok,
                    "user": PublicUser::from(&new_user),
                }),
                Some(&cookie),
            );
        }
        ("POST", ["auth", "logout"]) => {
            if let Some(tok) = req.cookie("dft_session") {
                if let Ok(mut sessions) = state.sessions.write() {
                    sessions.retain(|s| s.token != tok);
                    let _ = write_json_atomic(&state.data_dir.join("sessions.json"), &*sessions);
                }
            }
            let cookie = "dft_session=; Path=/; Max-Age=0; HttpOnly; SameSite=Lax";
            send_json(stream, 200, &serde_json::json!({"ok": true}), Some(cookie));
        }
        ("POST", ["user", "tokens"]) => {
            let Some(cur) = user else {
                return send_json_error(stream, 401, "Sign in required");
            };
            #[derive(Deserialize)]
            struct TokReq {
                name: String,
            }
            let body: TokReq = match req.json() {
                Ok(b) => b,
                Err(e) => return send_json_error(stream, 400, &e),
            };
            let raw_token = format!("dft_pat_{}", generate_token_hex(20));
            let token_id = generate_token_hex(6);
            let rec = AccessToken {
                id: token_id.clone(),
                name: if body.name.trim().is_empty() {
                    "CLI Token".to_string()
                } else {
                    body.name.trim().to_string()
                },
                token_hash: sha256_hex(raw_token.as_bytes()),
                prefix: raw_token[..14].to_string(),
                created_at: Utc::now().to_rfc3339(),
            };
            if let Ok(mut users) = state.users.write() {
                if let Some(u) = users.iter_mut().find(|u| u.username == cur.username) {
                    u.tokens.push(rec);
                    let _ = write_json_atomic(&state.data_dir.join("users.json"), &*users);
                }
            }
            send_json(
                stream,
                201,
                &serde_json::json!({
                    "id": token_id,
                    "token": raw_token,
                }),
                None,
            );
        }
        ("DELETE", ["user", "tokens", tid]) => {
            let Some(cur) = user else {
                return send_json_error(stream, 401, "Sign in required");
            };
            if let Ok(mut users) = state.users.write() {
                if let Some(u) = users.iter_mut().find(|u| u.username == cur.username) {
                    u.tokens.retain(|t| t.id != *tid);
                    let _ = write_json_atomic(&state.data_dir.join("users.json"), &*users);
                }
            }
            send_json(stream, 200, &serde_json::json!({"ok": true}), None);
        }
        ("POST", ["user", "password"]) => {
            let Some(cur) = user else {
                return send_json_error(stream, 401, "Sign in required");
            };
            #[derive(Deserialize)]
            struct PwReq {
                current_password: String,
                new_password: String,
            }
            let body: PwReq = match req.json() {
                Ok(b) => b,
                Err(e) => return send_json_error(stream, 400, &e),
            };
            if body.new_password.len() < 4 {
                return send_json_error(stream, 400, "New password must be at least 4 characters");
            }
            if hash_password(&body.current_password, &cur.salt) != cur.password_hash {
                return send_json_error(stream, 401, "Current password is incorrect");
            }
            if let Ok(mut users) = state.users.write() {
                if let Some(u) = users.iter_mut().find(|u| u.username == cur.username) {
                    u.salt = generate_token_hex(16);
                    u.password_hash = hash_password(&body.new_password, &u.salt);
                    let _ = write_json_atomic(&state.data_dir.join("users.json"), &*users);
                }
            }
            send_json(stream, 200, &serde_json::json!({"ok": true}), None);
        }
        ("GET", ["users"]) => {
            let all_repos = list_all_repo_metas(state, user.as_ref());
            let users: Vec<serde_json::Value> = state
                .users
                .read()
                .map(|list| {
                    list.iter()
                        .map(|u| {
                            let cnt = all_repos.iter().filter(|r| r.owner == u.username).count();
                            serde_json::json!({
                                "kind": "user",
                                "username": u.username,
                                "display_name": u.display_name,
                                "email": u.email,
                                "is_admin": u.is_admin,
                                "avatar_color": u.avatar_color,
                                "repo_count": cnt,
                                "created_at": u.created_at,
                            })
                        })
                        .collect()
                })
                .unwrap_or_default();
            let orgs: Vec<serde_json::Value> = state
                .orgs
                .read()
                .map(|list| {
                    list.iter()
                        .map(|o| {
                            let cnt = all_repos.iter().filter(|r| r.owner == o.name).count();
                            serde_json::json!({
                                "kind": "org",
                                "username": o.name,
                                "display_name": o.display_name,
                                "description": o.description,
                                "members": o.members,
                                "repo_count": cnt,
                                "created_at": o.created_at,
                            })
                        })
                        .collect()
                })
                .unwrap_or_default();
            send_json(
                stream,
                200,
                &serde_json::json!({ "users": users, "orgs": orgs }),
                None,
            );
        }
        ("POST", ["orgs"]) => {
            let actor = user
                .as_ref()
                .map(|u| u.username.clone())
                .unwrap_or_else(|| "admin".to_string());
            #[derive(Deserialize)]
            struct OrgReq {
                name: String,
                display_name: Option<String>,
                description: Option<String>,
            }
            let body: OrgReq = match req.json() {
                Ok(b) => b,
                Err(e) => return send_json_error(stream, 400, &e),
            };
            let slug = sanitize_slug(&body.name);
            if slug.len() < 2 {
                return send_json_error(
                    stream,
                    400,
                    "Organization name must be at least 2 characters",
                );
            }
            if state.owner_exists(&slug) {
                return send_json_error(stream, 409, "Owner name is already taken");
            }
            let org = OrgRecord {
                name: slug.clone(),
                display_name: body
                    .display_name
                    .filter(|s| !s.trim().is_empty())
                    .unwrap_or_else(|| slug.clone()),
                description: body.description.unwrap_or_default(),
                creator: actor.clone(),
                members: vec![actor],
                created_at: Utc::now().to_rfc3339(),
            };
            if let Ok(mut orgs) = state.orgs.write() {
                orgs.push(org.clone());
                let _ = write_json_atomic(&state.data_dir.join("orgs.json"), &*orgs);
            }
            send_json(stream, 201, &org, None);
        }

        // --- Global Repos, Activity, Issues, Pulls & Admin ---
        ("GET", ["repos"]) => {
            let q = req.query_param("q").unwrap_or_default().to_lowercase();
            let owner_filter = req.query_param("owner").unwrap_or_default();
            let sort = req
                .query_param("sort")
                .unwrap_or_else(|| "updated".to_string());
            let base_url = state.base_url_for(req);

            let mut items: Vec<serde_json::Value> = list_all_repo_metas(state, user.as_ref())
                .into_iter()
                .filter(|m| {
                    if !owner_filter.is_empty() && !m.owner.eq_ignore_ascii_case(&owner_filter) {
                        return false;
                    }
                    if !q.is_empty() {
                        let hay = format!(
                            "{}/{} {} {}",
                            m.owner,
                            m.name,
                            m.description,
                            m.topics.join(" ")
                        )
                        .to_lowercase();
                        return hay.contains(&q);
                    }
                    true
                })
                .map(|m| summarize_repo_card(state, &m, &base_url))
                .collect();

            match sort.as_str() {
                "stars" => items.sort_by(|a, b| {
                    let sa = a.get("stars_count").and_then(|v| v.as_u64()).unwrap_or(0);
                    let sb = b.get("stars_count").and_then(|v| v.as_u64()).unwrap_or(0);
                    sb.cmp(&sa)
                }),
                "name" => items.sort_by(|a, b| {
                    let na = a.get("full_name").and_then(|v| v.as_str()).unwrap_or("");
                    let nb = b.get("full_name").and_then(|v| v.as_str()).unwrap_or("");
                    na.cmp(nb)
                }),
                _ => items.sort_by(|a, b| {
                    let ua = a.get("updated_at").and_then(|v| v.as_str()).unwrap_or("");
                    let ub = b.get("updated_at").and_then(|v| v.as_str()).unwrap_or("");
                    ub.cmp(ua)
                }),
            }

            send_json(stream, 200, &serde_json::json!({ "repos": items }), None);
        }
        ("GET", ["activity"]) => {
            let owner_f = req.query_param("owner");
            let repo_f = req.query_param("repo");
            let limit = req
                .query_param("limit")
                .and_then(|s| s.parse::<usize>().ok())
                .unwrap_or(40);
            let events: Vec<ActivityEvent> = state
                .activity
                .read()
                .map(|list| {
                    list.iter()
                        .filter(|e| {
                            if let Some(ref o) = owner_f {
                                if !e.owner.eq_ignore_ascii_case(o) {
                                    return false;
                                }
                            }
                            if let Some(ref r) = repo_f {
                                if !e.repo.eq_ignore_ascii_case(r) {
                                    return false;
                                }
                            }
                            true
                        })
                        .take(limit)
                        .cloned()
                        .collect()
                })
                .unwrap_or_default();
            send_json(
                stream,
                200,
                &serde_json::json!({ "activity": events }),
                None,
            );
        }
        ("GET", ["issues"]) => {
            let state_filter = req
                .query_param("state")
                .unwrap_or_else(|| "open".to_string());
            let mut out = Vec::new();
            for m in list_all_repo_metas(state, user.as_ref()) {
                let rdir = state.repo_dir(&m.owner, &m.name);
                let issues: Vec<IssueRecord> =
                    read_json_file(&rdir.join("issues.json")).unwrap_or_default();
                for iss in issues {
                    if state_filter == "all" || iss.state == state_filter {
                        out.push(serde_json::json!({
                            "owner": m.owner,
                            "repo": m.name,
                            "issue": iss,
                        }));
                    }
                }
            }
            out.sort_by(|a, b| {
                let ua = a["issue"]["updated_at"].as_str().unwrap_or("");
                let ub = b["issue"]["updated_at"].as_str().unwrap_or("");
                ub.cmp(ua)
            });
            send_json(stream, 200, &serde_json::json!({ "issues": out }), None);
        }
        ("GET", ["pulls"]) => {
            let state_filter = req
                .query_param("state")
                .unwrap_or_else(|| "open".to_string());
            let mut out = Vec::new();
            for m in list_all_repo_metas(state, user.as_ref()) {
                let rdir = state.repo_dir(&m.owner, &m.name);
                let pulls: Vec<PullRecord> =
                    read_json_file(&rdir.join("pulls.json")).unwrap_or_default();
                for pr in pulls {
                    if state_filter == "all" || pr.state == state_filter {
                        out.push(serde_json::json!({
                            "owner": m.owner,
                            "repo": m.name,
                            "pull": pr,
                        }));
                    }
                }
            }
            out.sort_by(|a, b| {
                let ua = a["pull"]["updated_at"].as_str().unwrap_or("");
                let ub = b["pull"]["updated_at"].as_str().unwrap_or("");
                ub.cmp(ua)
            });
            send_json(stream, 200, &serde_json::json!({ "pulls": out }), None);
        }
        ("GET", ["admin", "overview"]) => {
            let metas = list_all_repo_metas(state, user.as_ref());
            let mut total_objects = 0usize;
            let mut total_issues = 0usize;
            let mut total_pulls = 0usize;
            for m in &metas {
                let rdir = state.repo_dir(&m.owner, &m.name);
                if let Ok(r) = Repository::open(&rdir) {
                    total_objects += r.cas().list_objects().map(|v| v.len()).unwrap_or(0);
                }
                let issues: Vec<IssueRecord> =
                    read_json_file(&rdir.join("issues.json")).unwrap_or_default();
                let pulls: Vec<PullRecord> =
                    read_json_file(&rdir.join("pulls.json")).unwrap_or_default();
                total_issues += issues.len();
                total_pulls += pulls.len();
            }
            let disk_bytes = compute_dir_size(&state.data_dir);
            let users_cnt = state.users.read().map(|u| u.len()).unwrap_or(0);
            let orgs_cnt = state.orgs.read().map(|o| o.len()).unwrap_or(0);

            send_json(
                stream,
                200,
                &serde_json::json!({
                    "version": env!("CARGO_PKG_VERSION"),
                    "os": env::consts::OS,
                    "arch": env::consts::ARCH,
                    "data_dir": state.data_dir.to_string_lossy(),
                    "uptime_secs": state.started_at.elapsed().as_secs(),
                    "disk_bytes": disk_bytes,
                    "repo_count": metas.len(),
                    "total_cas_objects": total_objects,
                    "user_count": users_cnt,
                    "org_count": orgs_cnt,
                    "total_issues": total_issues,
                    "total_pulls": total_pulls,
                }),
                None,
            );
        }

        // --- Create & Migrate Repository ---
        ("POST", ["repos"]) => {
            #[derive(Deserialize)]
            struct CreateRepoReq {
                owner: Option<String>,
                name: String,
                description: Option<String>,
                private: Option<bool>,
                default_branch: Option<String>,
                auto_init: Option<bool>,
                topics: Option<Vec<String>>,
            }
            let body: CreateRepoReq = match req.json() {
                Ok(b) => b,
                Err(e) => return send_json_error(stream, 400, &e),
            };
            let actor = user
                .as_ref()
                .map(|u| u.username.clone())
                .unwrap_or_else(|| "admin".to_string());
            let owner = sanitize_slug(body.owner.as_deref().unwrap_or(&actor));
            let name = sanitize_slug(&body.name);
            if name.is_empty() {
                return send_json_error(stream, 400, "Repository name is required");
            }
            let rdir = state.repo_dir(&owner, &name);
            if rdir.join("HEAD").exists() {
                return send_json_error(
                    stream,
                    409,
                    &format!("Repository '{}/{}' already exists", owner, name),
                );
            }
            let default_branch = body
                .default_branch
                .filter(|s| !s.trim().is_empty())
                .unwrap_or_else(|| "main".to_string());

            let _guard = state.repo_lock.write().ok();
            match create_bare_repository(
                state,
                &owner,
                &name,
                body.description.as_deref().unwrap_or(""),
                body.private.unwrap_or(false),
                &default_branch,
                body.auto_init.unwrap_or(true),
                body.topics.unwrap_or_default(),
                &actor,
            ) {
                Ok(meta) => send_json(stream, 201, &meta, None),
                Err(e) => send_json_error(stream, 500, &e),
            }
        }
        ("POST", ["repos", "migrate"]) => {
            #[derive(Deserialize)]
            struct MigrateReq {
                clone_addr: String,
                owner: Option<String>,
                name: String,
                description: Option<String>,
                private: Option<bool>,
            }
            let body: MigrateReq = match req.json() {
                Ok(b) => b,
                Err(e) => return send_json_error(stream, 400, &e),
            };
            let actor = user
                .as_ref()
                .map(|u| u.username.clone())
                .unwrap_or_else(|| "admin".to_string());
            let owner = sanitize_slug(body.owner.as_deref().unwrap_or(&actor));
            let name = sanitize_slug(&body.name);
            if name.is_empty() || body.clone_addr.trim().is_empty() {
                return send_json_error(
                    stream,
                    400,
                    "Source URL/path and target name are required",
                );
            }
            let rdir = state.repo_dir(&owner, &name);
            if rdir.join("HEAD").exists() {
                return send_json_error(stream, 409, "Target repository already exists");
            }

            let _guard = state.repo_lock.write().ok();
            match migrate_external_repository(
                state,
                &body.clone_addr,
                &owner,
                &name,
                body.description.as_deref().unwrap_or(""),
                body.private.unwrap_or(false),
                &actor,
            ) {
                Ok(meta) => send_json(stream, 201, &meta, None),
                Err(e) => send_json_error(stream, 500, &e),
            }
        }

        // --- Per-Repository Routes (/api/v1/repos/<owner>/<repo>/...) ---
        (_, ["repos", owner, repo_raw, rest @ ..]) => {
            let repo_name = repo_raw.trim_end_matches(".dft").trim_end_matches(".git");
            handle_repo_api(stream, req, state, user.as_ref(), owner, repo_name, rest);
        }

        _ => send_json_error(stream, 404, "API route not found"),
    }
}

fn handle_repo_api(
    stream: &mut TcpStream,
    req: &HttpRequest,
    state: &Arc<ServerState>,
    user: Option<&UserRecord>,
    owner: &str,
    repo_name: &str,
    rest: &[&str],
) {
    let repo_dir = state.repo_dir(owner, repo_name);
    if !repo_dir.join("HEAD").exists() {
        return send_json_error(
            stream,
            404,
            &format!("Repository '{}/{}' not found", owner, repo_name),
        );
    }

    let mut meta = load_repo_meta(&repo_dir, owner, repo_name);
    if meta.private && !can_access_private(&meta, user, state) {
        return send_json_error(stream, 401, "Private repository — sign in required");
    }

    let repo = match Repository::open(&repo_dir) {
        Ok(r) => Arc::new(r),
        Err(e) => return send_json_error(stream, 500, &e.to_string()),
    };

    match (req.method.as_str(), rest) {
        // Repo Overview
        ("GET", []) => {
            let base_url = state.base_url_for(req);
            let clone_url = format!("{}/{}/{}.dft", base_url, meta.owner, meta.name);
            let branches = list_ref_items(&repo, "refs/heads", Some(&meta.default_branch));
            let dimensions = list_dimension_items(&repo);
            let tags = list_ref_items(&repo, "refs/tags", None);

            let selected_ref = req
                .query_param("ref")
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| meta.default_branch.clone());
            let head_oid = resolve_any_ref(&repo, &selected_ref);
            let head_commit = head_oid
                .as_ref()
                .and_then(|oid| read_commit_json(&repo, oid));

            let commits_count = head_oid
                .as_ref()
                .map(|oid| {
                    let g = StoreCommitGraph::new(repo.cas().as_ref());
                    walk_commits(&g, &[*oid]).map(|v| v.len()).unwrap_or(0)
                })
                .unwrap_or(0);

            let objects_count = repo.cas().list_objects().map(|v| v.len()).unwrap_or(0);
            let issues: Vec<IssueRecord> =
                read_json_file(&repo_dir.join("issues.json")).unwrap_or_default();
            let pulls: Vec<PullRecord> =
                read_json_file(&repo_dir.join("pulls.json")).unwrap_or_default();
            let open_issues = issues.iter().filter(|i| i.state == "open").count();
            let open_pulls = pulls.iter().filter(|p| p.state == "open").count();

            let starred_by_me = user
                .map(|u| meta.stars.iter().any(|s| s == &u.username))
                .unwrap_or(false);

            send_json(
                stream,
                200,
                &serde_json::json!({
                    "meta": meta,
                    "clone_url": clone_url,
                    "selected_ref": selected_ref,
                    "is_empty": branches.is_empty() && dimensions.is_empty(),
                    "head_commit": head_commit,
                    "branches": branches,
                    "dimensions": dimensions,
                    "tags": tags,
                    "starred_by_me": starred_by_me,
                    "stats": {
                        "commits": commits_count,
                        "branches": branches.len(),
                        "dimensions": dimensions.len(),
                        "tags": tags.len(),
                        "objects": objects_count,
                        "open_issues": open_issues,
                        "open_pulls": open_pulls,
                        "stars": meta.stars.len(),
                        "forks": meta.forks_count,
                    }
                }),
                None,
            );
        }

        // Update Repo Settings
        ("PATCH", []) => {
            #[derive(Deserialize)]
            struct PatchRepo {
                description: Option<String>,
                private: Option<bool>,
                default_branch: Option<String>,
                topics: Option<Vec<String>>,
            }
            let body: PatchRepo = match req.json() {
                Ok(b) => b,
                Err(e) => return send_json_error(stream, 400, &e),
            };
            if let Some(d) = body.description {
                meta.description = d;
            }
            if let Some(p) = body.private {
                meta.private = p;
            }
            if let Some(db) = body.default_branch.filter(|s| !s.trim().is_empty()) {
                meta.default_branch = db.clone();
                let _ = fs::write(repo_dir.join("HEAD"), format!("ref: refs/heads/{}\n", db));
            }
            if let Some(t) = body.topics {
                meta.topics = t;
            }
            meta.updated_at = Utc::now().to_rfc3339();
            let _ = save_repo_meta(&repo_dir, &meta);
            send_json(stream, 200, &meta, None);
        }

        // Delete Repository
        ("DELETE", []) => {
            let _guard = state.repo_lock.write().ok();
            let _ = fs::remove_dir_all(&repo_dir);
            send_json(stream, 200, &serde_json::json!({"ok": true}), None);
        }

        // Star Toggle
        ("POST", ["star"]) => {
            let actor = user
                .map(|u| u.username.clone())
                .unwrap_or_else(|| "guest".to_string());
            if let Some(pos) = meta.stars.iter().position(|s| s == &actor) {
                meta.stars.remove(pos);
            } else {
                meta.stars.push(actor);
            }
            let _ = save_repo_meta(&repo_dir, &meta);
            send_json(
                stream,
                200,
                &serde_json::json!({
                    "stars_count": meta.stars.len(),
                    "stars": meta.stars,
                }),
                None,
            );
        }

        // Fork Repository
        ("POST", ["fork"]) => {
            #[derive(Deserialize)]
            struct ForkReq {
                target_owner: Option<String>,
                target_name: Option<String>,
            }
            let body: ForkReq = req.json().unwrap_or(ForkReq {
                target_owner: None,
                target_name: None,
            });
            let actor = user
                .map(|u| u.username.clone())
                .unwrap_or_else(|| "multiverse".to_string());
            let dst_owner = sanitize_slug(body.target_owner.as_deref().unwrap_or(&actor));
            let mut dst_name = sanitize_slug(body.target_name.as_deref().unwrap_or(repo_name));
            if dst_owner == meta.owner && dst_name == meta.name {
                dst_name = format!("{}-fork", meta.name);
            }
            let dst_dir = state.repo_dir(&dst_owner, &dst_name);
            if dst_dir.join("HEAD").exists() {
                return send_json_error(
                    stream,
                    409,
                    &format!("Fork target '{}/{}' already exists", dst_owner, dst_name),
                );
            }

            let _guard = state.repo_lock.write().ok();
            if let Some(p) = dst_dir.parent() {
                let _ = fs::create_dir_all(p);
            }
            let cow_strat = daft_dimension::cow::detect_best_strategy(&dst_dir);
            if let Err(e) =
                daft_dimension::cow::CowEngine::clone_dir(&repo_dir, &dst_dir, cow_strat)
            {
                return send_json_error(stream, 500, &format!("Fork clone failed: {}", e));
            }
            let _ = fs::remove_file(dst_dir.join("issues.json"));
            let _ = fs::remove_file(dst_dir.join("pulls.json"));

            meta.forks_count += 1;
            let _ = save_repo_meta(&repo_dir, &meta);

            let fork_meta = RepoMeta {
                owner: dst_owner.clone(),
                name: dst_name.clone(),
                description: meta.description.clone(),
                private: meta.private,
                default_branch: meta.default_branch.clone(),
                topics: meta.topics.clone(),
                stars: Vec::new(),
                forks_count: 0,
                forked_from: Some(format!("{}/{}", meta.owner, meta.name)),
                migrated_from: None,
                created_at: Utc::now().to_rfc3339(),
                updated_at: Utc::now().to_rfc3339(),
            };
            let _ = save_repo_meta(&dst_dir, &fork_meta);

            state.record_activity(ActivityEvent {
                id: generate_token_hex(8),
                event_type: "fork".to_string(),
                actor,
                owner: dst_owner,
                repo: dst_name,
                ref_name: None,
                commit_id: None,
                summary: format!("forked from {}/{}", meta.owner, meta.name),
                created_at: Utc::now().to_rfc3339(),
            });

            send_json(stream, 201, &fork_meta, None);
        }

        // CAS Tree Browser
        ("GET", ["tree"]) => {
            let ref_name = req
                .query_param("ref")
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| meta.default_branch.clone());
            let rel_path = req
                .query_param("path")
                .unwrap_or_default()
                .trim_matches('/')
                .to_string();

            let Some(commit_oid) = resolve_any_ref(&repo, &ref_name) else {
                return send_json(
                    stream,
                    200,
                    &serde_json::json!({
                        "ref": ref_name,
                        "path": rel_path,
                        "is_empty": true,
                        "entries": [],
                        "readme": null,
                    }),
                    None,
                );
            };

            let commit_info = read_commit_json(&repo, &commit_oid);
            let raw_commit = match repo.cas().read_raw(&commit_oid) {
                Ok(r) => r,
                Err(e) => return send_json_error(stream, 500, &e.to_string()),
            };
            let commit = match Commit::deserialize(&raw_commit.data) {
                Ok(c) => c,
                Err(e) => return send_json_error(stream, 500, &e.to_string()),
            };

            let target_tree_oid = if rel_path.is_empty() {
                Some(commit.tree)
            } else {
                lookup_path_in_tree(repo.cas().as_ref(), &commit.tree, &rel_path)
                    .and_then(|(mode, oid)| if mode.is_tree() { Some(oid) } else { None })
            };

            let Some(t_oid) = target_tree_oid else {
                return send_json_error(stream, 404, "Directory path not found in tree");
            };

            let raw_tree = match repo.cas().read_raw(&t_oid) {
                Ok(r) => r,
                Err(e) => return send_json_error(stream, 500, &e.to_string()),
            };
            let tree = match Tree::deserialize(&raw_tree.data) {
                Ok(t) => t,
                Err(e) => return send_json_error(stream, 500, &e.to_string()),
            };

            let mut entries = Vec::new();
            let mut readme_content: Option<String> = None;
            let mut readme_name: Option<String> = None;

            for entry in tree.entries() {
                let is_dir = entry.mode.is_tree();
                let item_path = if rel_path.is_empty() {
                    entry.name.clone()
                } else {
                    format!("{}/{}", rel_path, entry.name)
                };
                let size = if is_dir {
                    0
                } else {
                    repo.cas()
                        .read_raw(&entry.oid)
                        .map(|r| r.data.len())
                        .unwrap_or(0)
                };
                if !is_dir
                    && readme_content.is_none()
                    && entry.name.to_lowercase().starts_with("readme.")
                {
                    if let Ok(raw_b) = repo.cas().read_raw(&entry.oid) {
                        if let Ok(txt) = std::str::from_utf8(&raw_b.data) {
                            readme_content = Some(txt.to_string());
                            readme_name = Some(entry.name.clone());
                        }
                    }
                }
                entries.push(serde_json::json!({
                    "name": entry.name,
                    "path": item_path,
                    "is_dir": is_dir,
                    "mode": entry.mode.to_display_octal(),
                    "oid": entry.oid.to_hex(),
                    "size": size,
                }));
            }

            entries.sort_by(|a, b| {
                let ad = a["is_dir"].as_bool().unwrap_or(false);
                let bd = b["is_dir"].as_bool().unwrap_or(false);
                if ad != bd {
                    bd.cmp(&ad)
                } else {
                    let an = a["name"].as_str().unwrap_or("");
                    let bn = b["name"].as_str().unwrap_or("");
                    an.cmp(bn)
                }
            });

            send_json(
                stream,
                200,
                &serde_json::json!({
                    "ref": ref_name,
                    "path": rel_path,
                    "is_empty": false,
                    "latest_commit": commit_info,
                    "entries": entries,
                    "readme_name": readme_name,
                    "readme": readme_content,
                }),
                None,
            );
        }

        // CAS Blob Viewer
        ("GET", ["blob"]) => {
            let ref_name = req
                .query_param("ref")
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| meta.default_branch.clone());
            let file_path = req
                .query_param("path")
                .unwrap_or_default()
                .trim_matches('/')
                .to_string();

            let Some(commit_oid) = resolve_any_ref(&repo, &ref_name) else {
                return send_json_error(stream, 404, "Reference not found");
            };
            let Ok(raw_c) = repo.cas().read_raw(&commit_oid) else {
                return send_json_error(stream, 404, "Commit not found");
            };
            let Ok(commit) = Commit::deserialize(&raw_c.data) else {
                return send_json_error(stream, 500, "Corrupt commit");
            };
            let Some((mode, blob_oid)) =
                lookup_path_in_tree(repo.cas().as_ref(), &commit.tree, &file_path)
            else {
                return send_json_error(stream, 404, "File not found in tree");
            };
            if mode.is_tree() {
                return send_json_error(stream, 400, "Path is a directory");
            }
            let Ok(raw_blob) = repo.cas().read_raw(&blob_oid) else {
                return send_json_error(stream, 404, "Blob missing");
            };

            let is_bin = daft_core::diff::is_binary(&raw_blob.data);
            let raw_url = format!(
                "/{}/{}/raw/{}/{}",
                meta.owner, meta.name, ref_name, file_path
            );
            if is_bin {
                send_json(
                    stream,
                    200,
                    &serde_json::json!({
                        "path": file_path,
                        "oid": blob_oid.to_hex(),
                        "size": raw_blob.data.len(),
                        "is_binary": true,
                        "raw_url": raw_url,
                    }),
                    None,
                );
            } else {
                let text = String::from_utf8_lossy(&raw_blob.data).to_string();
                let lines = text.lines().count();
                send_json(
                    stream,
                    200,
                    &serde_json::json!({
                        "path": file_path,
                        "oid": blob_oid.to_hex(),
                        "size": raw_blob.data.len(),
                        "lines": lines,
                        "is_binary": false,
                        "content": text,
                        "raw_url": raw_url,
                    }),
                    None,
                );
            }
        }

        // Web Create / Edit / Delete File directly in CAS
        ("POST", ["contents"]) => {
            #[derive(Deserialize)]
            struct WriteFileReq {
                branch: Option<String>,
                path: String,
                content: Option<String>,
                message: Option<String>,
                action: Option<String>, // "write" | "delete"
            }
            let body: WriteFileReq = match req.json() {
                Ok(b) => b,
                Err(e) => return send_json_error(stream, 400, &e),
            };
            let clean_path = body.path.trim().trim_matches('/').replace('\\', "/");
            if clean_path.is_empty() || clean_path.contains("..") {
                return send_json_error(stream, 400, "Invalid file path");
            }

            let target_branch = body
                .branch
                .filter(|s| !s.trim().is_empty())
                .unwrap_or_else(|| meta.default_branch.clone());
            let action = body.action.as_deref().unwrap_or("write");
            let actor_name = user
                .map(|u| u.display_name.clone())
                .unwrap_or_else(|| "Draft Admin".to_string());
            let actor_login = user
                .map(|u| u.username.clone())
                .unwrap_or_else(|| "admin".to_string());
            let actor_email = user
                .map(|u| u.email.clone())
                .unwrap_or_else(|| "admin@draft-vcs.org".to_string());

            let _guard = state.repo_lock.write().ok();

            if let Some(owner_dim) = check_fence_violation(&repo_dir, &clean_path, &target_branch) {
                return send_json_error(
                    stream,
                    409,
                    &format!(
                        "Fence violation: path '{}' is protected by a hard boundary fence owned by dimension '{}'",
                        clean_path, owner_dim
                    ),
                );
            }

            let parent_oid = resolve_any_ref(&repo, &target_branch);
            let mut files = BTreeMap::new();
            if let Some(ref p_oid) = parent_oid {
                if let Ok(rc) = repo.cas().read_raw(p_oid) {
                    if let Ok(c) = Commit::deserialize(&rc.data) {
                        let _ = flatten_tree(repo.cas().as_ref(), &c.tree, "", &mut files);
                    }
                }
            }

            if action == "delete" {
                if files.remove(&clean_path).is_none() {
                    return send_json_error(stream, 404, "File does not exist on branch");
                }
            } else {
                let bytes = body.content.unwrap_or_default().into_bytes();
                let blob_oid = match repo.cas().write_blob(&bytes) {
                    Ok(id) => id,
                    Err(e) => return send_json_error(stream, 500, &e.to_string()),
                };
                files.insert(clean_path.clone(), (FileMode::REGULAR, blob_oid));
            }

            let new_tree_oid = match build_hierarchical_tree(repo.cas().as_ref(), &files) {
                Ok(t) => t,
                Err(e) => return send_json_error(stream, 500, &e.to_string()),
            };

            let sig = Signature::now(actor_name, actor_email);
            let default_msg = if action == "delete" {
                format!("Delete {}", clean_path)
            } else {
                format!("Update {}", clean_path)
            };
            let msg = body
                .message
                .filter(|s| !s.trim().is_empty())
                .unwrap_or(default_msg);

            let parents = parent_oid.into_iter().collect();
            let commit = Commit::new(new_tree_oid, parents, sig.clone(), sig, msg.clone());
            let raw_commit = RawObject::new(ObjectType::Commit, commit.serialize());
            let commit_oid = match repo.cas().write_raw(&raw_commit) {
                Ok(id) => id,
                Err(e) => return send_json_error(stream, 500, &e.to_string()),
            };

            // Write ref to dimension if it exists in refs/dimensions, else refs/heads
            write_dimension_or_head_ref(
                &repo_dir,
                &meta.default_branch,
                &target_branch,
                &commit_oid,
            );
            let _ = propagate_entanglements_from(&repo, &repo_dir, &target_branch);

            meta.updated_at = Utc::now().to_rfc3339();
            let _ = save_repo_meta(&repo_dir, &meta);

            state.record_activity(ActivityEvent {
                id: generate_token_hex(8),
                event_type: "push".to_string(),
                actor: actor_login,
                owner: meta.owner.clone(),
                repo: meta.name.clone(),
                ref_name: Some(target_branch.clone()),
                commit_id: Some(commit_oid.to_hex()),
                summary: msg,
                created_at: Utc::now().to_rfc3339(),
            });

            send_json(
                stream,
                200,
                &serde_json::json!({
                    "ok": true,
                    "commit_id": commit_oid.to_hex(),
                    "branch": target_branch,
                }),
                None,
            );
        }

        // Download Portable CAS Bundle
        ("GET", ["bundle"]) => {
            let mut refs = BTreeMap::new();
            for (k, v) in collect_all_repo_refs(&repo) {
                refs.insert(k, v);
            }
            let mut objects = Vec::new();
            if let Ok(oids) = repo.cas().list_objects() {
                for oid in oids {
                    if let Ok(raw) = repo.cas().read_raw(&oid) {
                        objects.push(serde_json::json!({
                            "oid": oid.to_hex(),
                            "object_type": raw.object_type.as_str(),
                            "data_hex": hex::encode(&raw.data),
                        }));
                    }
                }
            }
            let head = fs::read_to_string(repo_dir.join("HEAD"))
                .unwrap_or_else(|_| format!("ref: refs/heads/{}", meta.default_branch))
                .trim()
                .to_string();
            let archive = serde_json::json!({
                "version": 1,
                "head": head,
                "refs": refs,
                "objects": objects,
            });
            let payload = format!(
                "DAFT-BUNDLE-V1\n{}",
                serde_json::to_string_pretty(&archive).unwrap_or_default()
            );
            send_response(
                stream,
                200,
                "application/octet-stream",
                payload.as_bytes(),
                None,
            );
        }

        // Commits History
        ("GET", ["commits"]) => {
            let ref_name = req
                .query_param("ref")
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| meta.default_branch.clone());
            let limit = req
                .query_param("limit")
                .and_then(|s| s.parse::<usize>().ok())
                .unwrap_or(60);

            let mut commits = Vec::new();
            if let Some(start_oid) = resolve_any_ref(&repo, &ref_name) {
                let graph = StoreCommitGraph::new(repo.cas().as_ref());
                if let Ok(oids) = walk_commits(&graph, &[start_oid]) {
                    for oid in oids.into_iter().take(limit) {
                        if let Some(cj) = read_commit_json(&repo, &oid) {
                            commits.push(cj);
                        }
                    }
                }
            }
            send_json(
                stream,
                200,
                &serde_json::json!({ "commits": commits }),
                None,
            );
        }

        // Single Commit Details + Myers Diff
        ("GET", ["commits", sha]) => {
            let Some(commit_oid) = resolve_any_ref(&repo, sha) else {
                return send_json_error(stream, 404, "Commit not found");
            };
            let Ok(raw_c) = repo.cas().read_raw(&commit_oid) else {
                return send_json_error(stream, 404, "Commit missing");
            };
            let Ok(commit) = Commit::deserialize(&raw_c.data) else {
                return send_json_error(stream, 500, "Corrupt commit");
            };

            let parent_tree = commit.parents.first().and_then(|p_oid| {
                repo.cas()
                    .read_raw(p_oid)
                    .ok()
                    .and_then(|r| Commit::deserialize(&r.data).ok())
                    .map(|pc| pc.tree)
            });

            let patches = diff_trees(
                repo.cas().as_ref(),
                parent_tree.as_ref(),
                Some(&commit.tree),
            )
            .unwrap_or_default();
            let diff_text = format_unified_diff(&patches);
            let (files_changed, additions, deletions) = summarize_patches(&patches);

            send_json(
                stream,
                200,
                &serde_json::json!({
                    "commit": read_commit_json(&repo, &commit_oid),
                    "stats": {
                        "files_changed": files_changed,
                        "additions": additions,
                        "deletions": deletions,
                    },
                    "diff": diff_text,
                }),
                None,
            );
        }

        // Compare & 3-Way Foresee Simulation
        ("GET", ["compare"]) => {
            let base = req
                .query_param("base")
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| meta.default_branch.clone());
            let head = req
                .query_param("head")
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| meta.default_branch.clone());
            let cmp = compute_comparison(&repo, &base, &head);
            send_json(stream, 200, &cmp, None);
        }

        // Create / Delete Refs (Branches, Parallel Dimensions, Tags)
        ("POST", ["refs"]) => {
            #[derive(Deserialize)]
            struct CreateRefReq {
                kind: String, // "branch" | "dimension" | "tag"
                name: String,
                from_ref: Option<String>,
            }
            let body: CreateRefReq = match req.json() {
                Ok(b) => b,
                Err(e) => return send_json_error(stream, 400, &e),
            };
            let clean_name = body.name.trim().trim_matches('/');
            if clean_name.is_empty() || clean_name.contains("..") || clean_name.contains(' ') {
                return send_json_error(stream, 400, "Invalid reference name");
            }
            let src = body
                .from_ref
                .filter(|s| !s.trim().is_empty())
                .unwrap_or_else(|| meta.default_branch.clone());
            let Some(src_oid) = resolve_any_ref(&repo, &src) else {
                return send_json_error(stream, 400, "Source revision has no commits yet");
            };
            let prefix = match body.kind.as_str() {
                "dimension" => "refs/dimensions",
                "tag" => "refs/tags",
                _ => "refs/heads",
            };
            let ref_path = repo_dir.join(prefix).join(clean_name);
            if ref_path.exists() {
                return send_json_error(stream, 409, "Reference already exists");
            }
            if let Some(p) = ref_path.parent() {
                let _ = fs::create_dir_all(p);
            }
            let _ = fs::write(&ref_path, format!("{}\n", src_oid.to_hex()));
            send_json(
                stream,
                201,
                &serde_json::json!({
                    "ok": true,
                    "kind": body.kind,
                    "name": clean_name,
                    "commit": src_oid.to_hex(),
                }),
                None,
            );
        }
        ("DELETE", ["refs"]) => {
            let kind = req
                .query_param("kind")
                .unwrap_or_else(|| "branch".to_string());
            let name = req.query_param("name").unwrap_or_default();
            if kind == "branch" && name == meta.default_branch {
                return send_json_error(stream, 400, "Cannot delete the default branch");
            }
            let prefix = match kind.as_str() {
                "dimension" => "refs/dimensions",
                "tag" => "refs/tags",
                _ => "refs/heads",
            };
            let ref_path = repo_dir.join(prefix).join(name.trim_matches('/'));
            if ref_path.is_file() {
                let _ = fs::remove_file(ref_path);
            }
            send_json(stream, 200, &serde_json::json!({"ok": true}), None);
        }

        // --- Issues ---
        ("GET", ["issues"]) => {
            let state_f = req
                .query_param("state")
                .unwrap_or_else(|| "open".to_string());
            let all_issues: Vec<IssueRecord> =
                read_json_file(&repo_dir.join("issues.json")).unwrap_or_default();
            let open_count = all_issues.iter().filter(|i| i.state == "open").count();
            let closed_count = all_issues.iter().filter(|i| i.state == "closed").count();
            let filtered: Vec<IssueRecord> = all_issues
                .into_iter()
                .filter(|i| state_f == "all" || i.state == state_f)
                .rev()
                .collect();
            send_json(
                stream,
                200,
                &serde_json::json!({
                    "open_count": open_count,
                    "closed_count": closed_count,
                    "issues": filtered,
                }),
                None,
            );
        }
        ("POST", ["issues"]) => {
            #[derive(Deserialize)]
            struct NewIssue {
                title: String,
                body: Option<String>,
                labels: Option<Vec<String>>,
                assignee: Option<String>,
            }
            let body: NewIssue = match req.json() {
                Ok(b) => b,
                Err(e) => return send_json_error(stream, 400, &e),
            };
            if body.title.trim().is_empty() {
                return send_json_error(stream, 400, "Issue title is required");
            }
            let actor = user
                .map(|u| u.username.clone())
                .unwrap_or_else(|| "admin".to_string());

            let _guard = state.repo_lock.write().ok();
            let mut issues: Vec<IssueRecord> =
                read_json_file(&repo_dir.join("issues.json")).unwrap_or_default();
            let next_num = issues.iter().map(|i| i.number).max().unwrap_or(0) + 1;
            let now = Utc::now().to_rfc3339();
            let rec = IssueRecord {
                number: next_num,
                title: body.title.trim().to_string(),
                body: body.body.unwrap_or_default(),
                author: actor.clone(),
                state: "open".to_string(),
                labels: body.labels.unwrap_or_default(),
                assignee: body.assignee.filter(|s| !s.trim().is_empty()),
                comments: Vec::new(),
                created_at: now.clone(),
                updated_at: now.clone(),
            };
            issues.push(rec.clone());
            let _ = write_json_atomic(&repo_dir.join("issues.json"), &issues);

            state.record_activity(ActivityEvent {
                id: generate_token_hex(8),
                event_type: "issue".to_string(),
                actor,
                owner: meta.owner,
                repo: meta.name,
                ref_name: None,
                commit_id: None,
                summary: format!("opened issue #{}: {}", rec.number, rec.title),
                created_at: now,
            });
            send_json(stream, 201, &rec, None);
        }
        ("GET", ["issues", num_str]) => {
            let num = num_str.parse::<u64>().unwrap_or(0);
            let issues: Vec<IssueRecord> =
                read_json_file(&repo_dir.join("issues.json")).unwrap_or_default();
            match issues.into_iter().find(|i| i.number == num) {
                Some(iss) => send_json(stream, 200, &iss, None),
                None => send_json_error(stream, 404, "Issue not found"),
            }
        }
        ("PATCH", ["issues", num_str]) => {
            let num = num_str.parse::<u64>().unwrap_or(0);
            #[derive(Deserialize)]
            struct PatchIssue {
                state: Option<String>,
                title: Option<String>,
                body: Option<String>,
                labels: Option<Vec<String>>,
            }
            let patch: PatchIssue = match req.json() {
                Ok(p) => p,
                Err(e) => return send_json_error(stream, 400, &e),
            };
            let _guard = state.repo_lock.write().ok();
            let mut issues: Vec<IssueRecord> =
                read_json_file(&repo_dir.join("issues.json")).unwrap_or_default();
            let Some(iss) = issues.iter_mut().find(|i| i.number == num) else {
                return send_json_error(stream, 404, "Issue not found");
            };
            if let Some(s) = patch.state {
                iss.state = s;
            }
            if let Some(t) = patch.title {
                iss.title = t;
            }
            if let Some(b) = patch.body {
                iss.body = b;
            }
            if let Some(l) = patch.labels {
                iss.labels = l;
            }
            iss.updated_at = Utc::now().to_rfc3339();
            let updated = iss.clone();
            let _ = write_json_atomic(&repo_dir.join("issues.json"), &issues);
            send_json(stream, 200, &updated, None);
        }
        ("POST", ["issues", num_str, "comments"]) => {
            let num = num_str.parse::<u64>().unwrap_or(0);
            #[derive(Deserialize)]
            struct NewCmt {
                body: String,
            }
            let cmt_req: NewCmt = match req.json() {
                Ok(c) => c,
                Err(e) => return send_json_error(stream, 400, &e),
            };
            let actor = user
                .map(|u| u.username.clone())
                .unwrap_or_else(|| "admin".to_string());
            let _guard = state.repo_lock.write().ok();
            let mut issues: Vec<IssueRecord> =
                read_json_file(&repo_dir.join("issues.json")).unwrap_or_default();
            let Some(iss) = issues.iter_mut().find(|i| i.number == num) else {
                return send_json_error(stream, 404, "Issue not found");
            };
            let cid = iss.comments.len() as u64 + 1;
            let now = Utc::now().to_rfc3339();
            iss.comments.push(CommentItem {
                id: cid,
                author: actor,
                body: cmt_req.body,
                created_at: now.clone(),
            });
            iss.updated_at = now;
            let updated = iss.clone();
            let _ = write_json_atomic(&repo_dir.join("issues.json"), &issues);
            send_json(stream, 201, &updated, None);
        }

        // --- Pull / Convergence Requests ---
        ("GET", ["pulls"]) => {
            let state_f = req
                .query_param("state")
                .unwrap_or_else(|| "open".to_string());
            let all_pulls: Vec<PullRecord> =
                read_json_file(&repo_dir.join("pulls.json")).unwrap_or_default();
            let open_count = all_pulls.iter().filter(|p| p.state == "open").count();
            let merged_count = all_pulls.iter().filter(|p| p.state == "merged").count();
            let closed_count = all_pulls.iter().filter(|p| p.state == "closed").count();

            let enriched: Vec<serde_json::Value> = all_pulls
                .into_iter()
                .filter(|p| state_f == "all" || p.state == state_f)
                .rev()
                .map(|p| {
                    let foresee = if p.state == "open" {
                        Some(compute_foresee_badge(&repo, &p.base_ref, &p.head_ref))
                    } else {
                        None
                    };
                    serde_json::json!({
                        "pull": p,
                        "foresee": foresee,
                    })
                })
                .collect();

            send_json(
                stream,
                200,
                &serde_json::json!({
                    "open_count": open_count,
                    "merged_count": merged_count,
                    "closed_count": closed_count,
                    "pulls": enriched,
                }),
                None,
            );
        }
        ("POST", ["pulls"]) => {
            #[derive(Deserialize)]
            struct NewPull {
                title: String,
                body: Option<String>,
                head_ref: String,
                base_ref: String,
            }
            let body: NewPull = match req.json() {
                Ok(b) => b,
                Err(e) => return send_json_error(stream, 400, &e),
            };
            if body.title.trim().is_empty() {
                return send_json_error(stream, 400, "Title is required");
            }
            if body.head_ref == body.base_ref {
                return send_json_error(stream, 400, "Source and target refs must be different");
            }
            if resolve_any_ref(&repo, &body.head_ref).is_none()
                || resolve_any_ref(&repo, &body.base_ref).is_none()
            {
                return send_json_error(stream, 400, "Source or target reference does not exist");
            }
            let actor = user
                .map(|u| u.username.clone())
                .unwrap_or_else(|| "admin".to_string());

            let _guard = state.repo_lock.write().ok();
            let mut pulls: Vec<PullRecord> =
                read_json_file(&repo_dir.join("pulls.json")).unwrap_or_default();
            let next_num = pulls.iter().map(|p| p.number).max().unwrap_or(0) + 1;
            let now = Utc::now().to_rfc3339();
            let rec = PullRecord {
                number: next_num,
                title: body.title.trim().to_string(),
                body: body.body.unwrap_or_default(),
                author: actor.clone(),
                head_ref: body.head_ref,
                base_ref: body.base_ref,
                state: "open".to_string(),
                merged_by: None,
                merge_commit: None,
                comments: Vec::new(),
                created_at: now.clone(),
                updated_at: now.clone(),
            };
            pulls.push(rec.clone());
            let _ = write_json_atomic(&repo_dir.join("pulls.json"), &pulls);

            state.record_activity(ActivityEvent {
                id: generate_token_hex(8),
                event_type: "pull".to_string(),
                actor,
                owner: meta.owner,
                repo: meta.name,
                ref_name: Some(rec.head_ref.clone()),
                commit_id: None,
                summary: format!(
                    "opened convergence request #{}: {} ({} -> {})",
                    rec.number, rec.title, rec.head_ref, rec.base_ref
                ),
                created_at: now,
            });
            send_json(stream, 201, &rec, None);
        }
        ("GET", ["pulls", num_str]) => {
            let num = num_str.parse::<u64>().unwrap_or(0);
            let pulls: Vec<PullRecord> =
                read_json_file(&repo_dir.join("pulls.json")).unwrap_or_default();
            let Some(pr) = pulls.into_iter().find(|p| p.number == num) else {
                return send_json_error(stream, 404, "Convergence request not found");
            };
            let comparison = compute_comparison(&repo, &pr.base_ref, &pr.head_ref);
            send_json(
                stream,
                200,
                &serde_json::json!({
                    "pull": pr,
                    "comparison": comparison,
                }),
                None,
            );
        }
        ("POST", ["pulls", num_str, "merge"]) => {
            let num = num_str.parse::<u64>().unwrap_or(0);
            #[derive(Deserialize, Default)]
            struct MergeReq {
                message: Option<String>,
            }
            let mreq: MergeReq = req.json().unwrap_or_default();
            let actor_name = user
                .map(|u| u.display_name.clone())
                .unwrap_or_else(|| "Draft Admin".to_string());
            let actor_login = user
                .map(|u| u.username.clone())
                .unwrap_or_else(|| "admin".to_string());
            let actor_email = user
                .map(|u| u.email.clone())
                .unwrap_or_else(|| "admin@draft-vcs.org".to_string());

            let _guard = state.repo_lock.write().ok();
            let mut pulls: Vec<PullRecord> =
                read_json_file(&repo_dir.join("pulls.json")).unwrap_or_default();
            let Some(pr) = pulls.iter_mut().find(|p| p.number == num) else {
                return send_json_error(stream, 404, "Convergence request not found");
            };
            if pr.state != "open" {
                return send_json_error(stream, 400, "Convergence request is not open");
            }

            let Some(base_oid) = resolve_any_ref(&repo, &pr.base_ref) else {
                return send_json_error(stream, 400, "Base ref not found");
            };
            let Some(head_oid) = resolve_any_ref(&repo, &pr.head_ref) else {
                return send_json_error(stream, 400, "Head ref not found");
            };

            let cas = repo.cas().as_ref();
            let graph = StoreCommitGraph::new(cas);
            let lcas = all_merge_bases(&graph, &base_oid, &head_oid).unwrap_or_default();
            let lca_tree = lcas
                .first()
                .and_then(|id| cas.read_raw(id).ok())
                .and_then(|r| Commit::deserialize(&r.data).ok())
                .map(|c| c.tree);

            let Ok(base_raw) = cas.read_raw(&base_oid) else {
                return send_json_error(stream, 500, "Missing base commit");
            };
            let Ok(base_commit) = Commit::deserialize(&base_raw.data) else {
                return send_json_error(stream, 500, "Corrupt base commit");
            };
            let Ok(head_raw) = cas.read_raw(&head_oid) else {
                return send_json_error(stream, 500, "Missing head commit");
            };
            let Ok(head_commit) = Commit::deserialize(&head_raw.data) else {
                return send_json_error(stream, 500, "Corrupt head commit");
            };

            let merge_res = match merge_trees_3way(
                cas,
                lca_tree.as_ref(),
                &base_commit.tree,
                &head_commit.tree,
                &pr.base_ref,
                &pr.head_ref,
            ) {
                Ok(r) => r,
                Err(e) => return send_json_error(stream, 500, &e.to_string()),
            };

            if merge_res.has_conflicts() {
                let files: Vec<String> = merge_res.conflicted_files.keys().cloned().collect();
                return send_json_error(
                    stream,
                    409,
                    &format!(
                        "Cannot auto-converge due to conflicts in: {}",
                        files.join(", ")
                    ),
                );
            }

            let merged_tree_oid = match merge_res.write_clean_tree(cas) {
                Ok(id) => id,
                Err(e) => return send_json_error(stream, 500, &e.to_string()),
            };

            let sig = Signature::now(actor_name, actor_email);
            let commit_msg = mreq
                .message
                .filter(|s| !s.trim().is_empty())
                .unwrap_or_else(|| {
                    format!(
                        "[converge] Merge #{} '{}' ({} -> {})",
                        pr.number, pr.title, pr.head_ref, pr.base_ref
                    )
                });

            let merge_commit = Commit::new(
                merged_tree_oid,
                vec![base_oid, head_oid],
                sig.clone(),
                sig,
                commit_msg,
            );
            let raw_mc = RawObject::new(ObjectType::Commit, merge_commit.serialize());
            let merge_oid = match cas.write_raw(&raw_mc) {
                Ok(id) => id,
                Err(e) => return send_json_error(stream, 500, &e.to_string()),
            };

            // Update target ref
            let dim_target = repo_dir.join("refs/dimensions").join(&pr.base_ref);
            let target_file = if dim_target.exists() {
                dim_target
            } else {
                repo_dir.join("refs/heads").join(&pr.base_ref)
            };
            if let Some(p) = target_file.parent() {
                let _ = fs::create_dir_all(p);
            }
            let _ = fs::write(&target_file, format!("{}\n", merge_oid.to_hex()));

            let now = Utc::now().to_rfc3339();
            pr.state = "merged".to_string();
            pr.merged_by = Some(actor_login.clone());
            pr.merge_commit = Some(merge_oid.to_hex());
            pr.updated_at = now.clone();
            let updated_pr = pr.clone();
            let _ = write_json_atomic(&repo_dir.join("pulls.json"), &pulls);

            meta.updated_at = now.clone();
            let _ = save_repo_meta(&repo_dir, &meta);

            state.record_activity(ActivityEvent {
                id: generate_token_hex(8),
                event_type: "merge".to_string(),
                actor: actor_login,
                owner: meta.owner,
                repo: meta.name,
                ref_name: Some(updated_pr.base_ref.clone()),
                commit_id: Some(merge_oid.to_hex()),
                summary: format!(
                    "converged #{} ({} -> {})",
                    updated_pr.number, updated_pr.head_ref, updated_pr.base_ref
                ),
                created_at: now,
            });

            send_json(stream, 200, &updated_pr, None);
        }
        ("PATCH", ["pulls", num_str]) => {
            let num = num_str.parse::<u64>().unwrap_or(0);
            #[derive(Deserialize)]
            struct PatchPull {
                state: Option<String>,
                title: Option<String>,
                body: Option<String>,
            }
            let patch: PatchPull = match req.json() {
                Ok(p) => p,
                Err(e) => return send_json_error(stream, 400, &e),
            };
            let _guard = state.repo_lock.write().ok();
            let mut pulls: Vec<PullRecord> =
                read_json_file(&repo_dir.join("pulls.json")).unwrap_or_default();
            let Some(pr) = pulls.iter_mut().find(|p| p.number == num) else {
                return send_json_error(stream, 404, "Pull request not found");
            };
            if let Some(s) = patch.state {
                pr.state = s;
            }
            if let Some(t) = patch.title {
                pr.title = t;
            }
            if let Some(b) = patch.body {
                pr.body = b;
            }
            pr.updated_at = Utc::now().to_rfc3339();
            let updated = pr.clone();
            let _ = write_json_atomic(&repo_dir.join("pulls.json"), &pulls);
            send_json(stream, 200, &updated, None);
        }
        ("POST", ["pulls", num_str, "comments"]) => {
            let num = num_str.parse::<u64>().unwrap_or(0);
            #[derive(Deserialize)]
            struct NewCmt {
                body: String,
            }
            let cmt_req: NewCmt = match req.json() {
                Ok(c) => c,
                Err(e) => return send_json_error(stream, 400, &e),
            };
            let actor = user
                .map(|u| u.username.clone())
                .unwrap_or_else(|| "admin".to_string());
            let _guard = state.repo_lock.write().ok();
            let mut pulls: Vec<PullRecord> =
                read_json_file(&repo_dir.join("pulls.json")).unwrap_or_default();
            let Some(pr) = pulls.iter_mut().find(|p| p.number == num) else {
                return send_json_error(stream, 404, "Pull request not found");
            };
            let cid = pr.comments.len() as u64 + 1;
            let now = Utc::now().to_rfc3339();
            pr.comments.push(CommentItem {
                id: cid,
                author: actor,
                body: cmt_req.body,
                created_at: now.clone(),
            });
            pr.updated_at = now;
            let updated = pr.clone();
            let _ = write_json_atomic(&repo_dir.join("pulls.json"), &pulls);
            send_json(stream, 201, &updated, None);
        }

        // --- Multiverse Spacetime Timeline (works on bare server repos!) ---
        ("GET", ["timeline"]) => {
            let fmt = req
                .query_param("format")
                .unwrap_or_else(|| "json".to_string());
            let dim_filter = req.query_param("dimension").filter(|s| !s.is_empty());
            let graph = build_server_timeline_graph(
                &repo,
                &format!("{}/{}", meta.owner, meta.name),
                &meta.default_branch,
                dim_filter.as_deref(),
            );
            match fmt.as_str() {
                "svg" => {
                    let svg = render_svg(&graph);
                    send_response(stream, 200, "image/svg+xml", svg.as_bytes(), None);
                }
                "ascii" | "text" => {
                    let txt = if let Some(ref d) = dim_filter {
                        render_single_dimension(&graph, d)
                    } else {
                        render_terminal_graph(&graph)
                    };
                    send_response(
                        stream,
                        200,
                        "text/plain; charset=utf-8",
                        txt.as_bytes(),
                        None,
                    );
                }
                "ancestry" => {
                    let anc = render_ancestry_tree(&graph);
                    send_response(
                        stream,
                        200,
                        "text/plain; charset=utf-8",
                        anc.as_bytes(),
                        None,
                    );
                }
                "dot" => {
                    let dot = render_dot(&graph);
                    send_response(
                        stream,
                        200,
                        "text/plain; charset=utf-8",
                        dot.as_bytes(),
                        None,
                    );
                }
                _ => {
                    let j = render_json(&graph).unwrap_or_else(|_| "{}".to_string());
                    send_response(stream, 200, "application/json", j.as_bytes(), None);
                }
            }
        }

        // --- Radar, Hot Zones, Pairwise Foresight Matrix, Territory & Swarm Telemetry ---
        ("GET", ["radar"]) => {
            let territory = load_territory_json(&repo_dir);
            let agents = load_agents_json(&repo_dir);
            let matrix = compute_radar_matrix(&repo, &meta.default_branch);
            let hot_zones = compute_hot_zones(&repo, &meta.default_branch);
            let entanglements = load_entanglements_json(&repo_dir);
            let cronos_log = load_cronos_log(&repo_dir);
            let snapshots = list_snapshots_json(&repo);
            let messages = load_messages_json(&repo_dir);
            send_json(
                stream,
                200,
                &serde_json::json!({
                    "default_branch": meta.default_branch,
                    "hot_zones": hot_zones,
                    "matrix": matrix,
                    "territory": territory,
                    "agents": agents,
                    "entanglements": entanglements,
                    "cronos_log": cronos_log,
                    "snapshots": snapshots,
                    "messages": messages,
                }),
                None,
            );
        }

        // --- Non-Destructive Cross-Dimension Observe ('dft observe') ---
        ("GET", ["observe"]) => {
            let file_path = req
                .query_param("path")
                .unwrap_or_default()
                .trim_matches('/')
                .to_string();
            if file_path.is_empty() {
                return send_json_error(stream, 400, "Query parameter 'path' is required");
            }
            let observations =
                observe_file_across_dimensions(&repo, &meta.default_branch, &file_path);
            send_json(
                stream,
                200,
                &serde_json::json!({
                    "path": file_path,
                    "observations": observations,
                }),
                None,
            );
        }

        // --- Wavefunction Collapse ('dft collapse') — Atomic Multi-Dimension Collapse ---
        ("POST", ["collapse"]) => {
            #[derive(Deserialize, Default)]
            struct CollapseReq {
                into: Option<String>,
                dimensions: Option<Vec<String>>,
                dry_run: Option<bool>,
                message: Option<String>,
            }
            let body: CollapseReq = req.json().unwrap_or_default();
            let target = body
                .into
                .filter(|s| !s.trim().is_empty())
                .unwrap_or_else(|| meta.default_branch.clone());
            let dry_run = body.dry_run.unwrap_or(false);
            let actor_name = user
                .map(|u| u.display_name.clone())
                .unwrap_or_else(|| "Draft Swarm".to_string());
            let actor_login = user
                .map(|u| u.username.clone())
                .unwrap_or_else(|| "admin".to_string());
            let actor_email = user
                .map(|u| u.email.clone())
                .unwrap_or_else(|| "swarm@draft.local".to_string());

            let _guard = state.repo_lock.write().ok();
            let candidates: Vec<String> = match body.dimensions {
                Some(list) if !list.is_empty() => list,
                _ => list_dimension_items(&repo)
                    .into_iter()
                    .filter_map(|d| d["name"].as_str().map(String::from))
                    .filter(|n| n != &target && n != "mainline")
                    .collect(),
            };

            match execute_server_multi_converge(
                &repo,
                &repo_dir,
                &meta.default_branch,
                &target,
                &candidates,
                dry_run,
                true,
                body.message.as_deref(),
                &actor_name,
                &actor_email,
            ) {
                Ok(res) => {
                    if !dry_run {
                        meta.updated_at = Utc::now().to_rfc3339();
                        let _ = save_repo_meta(&repo_dir, &meta);
                        if let Some(cid) = res.get("commit_id").and_then(|v| v.as_str()) {
                            append_cronos_wal(
                                &repo_dir,
                                "collapse",
                                &format!("Collapsed {:?} into {}", candidates, target),
                                Some(cid),
                            );
                            state.record_activity(ActivityEvent {
                                id: generate_token_hex(8),
                                event_type: "merge".to_string(),
                                actor: actor_login,
                                owner: meta.owner,
                                repo: meta.name,
                                ref_name: Some(target),
                                commit_id: Some(cid.to_string()),
                                summary: format!(
                                    "collapsed {} parallel dimension(s)",
                                    candidates.len()
                                ),
                                created_at: Utc::now().to_rfc3339(),
                            });
                        }
                    }
                    send_json(stream, 200, &res, None);
                }
                Err(e) => send_json_error(stream, 400, &e),
            }
        }

        // --- Multi-Dimension Convergence ('dft converge') ---
        ("POST", ["converge"]) => {
            #[derive(Deserialize)]
            struct ConvergeReq {
                dimensions: Vec<String>,
                into: Option<String>,
                dry_run: Option<bool>,
                message: Option<String>,
            }
            let body: ConvergeReq = match req.json() {
                Ok(b) => b,
                Err(e) => return send_json_error(stream, 400, &e),
            };
            if body.dimensions.is_empty() {
                return send_json_error(
                    stream,
                    400,
                    "Select at least one parallel dimension to converge",
                );
            }
            let target = body
                .into
                .filter(|s| !s.trim().is_empty())
                .unwrap_or_else(|| meta.default_branch.clone());
            let dry_run = body.dry_run.unwrap_or(false);
            let actor_name = user
                .map(|u| u.display_name.clone())
                .unwrap_or_else(|| "Draft Swarm".to_string());
            let actor_login = user
                .map(|u| u.username.clone())
                .unwrap_or_else(|| "admin".to_string());
            let actor_email = user
                .map(|u| u.email.clone())
                .unwrap_or_else(|| "swarm@draft.local".to_string());

            let _guard = state.repo_lock.write().ok();
            match execute_server_multi_converge(
                &repo,
                &repo_dir,
                &meta.default_branch,
                &target,
                &body.dimensions,
                dry_run,
                false,
                body.message.as_deref(),
                &actor_name,
                &actor_email,
            ) {
                Ok(res) => {
                    if !dry_run {
                        meta.updated_at = Utc::now().to_rfc3339();
                        let _ = save_repo_meta(&repo_dir, &meta);
                        if let Some(cid) = res.get("commit_id").and_then(|v| v.as_str()) {
                            append_cronos_wal(
                                &repo_dir,
                                "converge",
                                &format!("Converged {:?} into {}", body.dimensions, target),
                                Some(cid),
                            );
                            state.record_activity(ActivityEvent {
                                id: generate_token_hex(8),
                                event_type: "merge".to_string(),
                                actor: actor_login,
                                owner: meta.owner,
                                repo: meta.name,
                                ref_name: Some(target),
                                commit_id: Some(cid.to_string()),
                                summary: format!("converged {:?}", body.dimensions),
                                created_at: Utc::now().to_rfc3339(),
                            });
                        }
                    }
                    send_json(stream, 200, &res, None);
                }
                Err(e) => send_json_error(stream, 400, &e),
            }
        }

        // --- Domino Cascade Propagation ('dft cascade') ---
        ("POST", ["cascade"]) => {
            #[derive(Deserialize, Default)]
            struct CascadeReq {
                from: Option<String>,
            }
            let body: CascadeReq = req.json().unwrap_or_default();
            let source = body
                .from
                .filter(|s| !s.trim().is_empty())
                .unwrap_or_else(|| meta.default_branch.clone());
            let actor_name = user
                .map(|u| u.display_name.clone())
                .unwrap_or_else(|| "Draft Swarm".to_string());
            let actor_email = user
                .map(|u| u.email.clone())
                .unwrap_or_else(|| "swarm@draft.local".to_string());

            let _guard = state.repo_lock.write().ok();
            match execute_server_cascade(
                &repo,
                &repo_dir,
                &meta.default_branch,
                &source,
                &actor_name,
                &actor_email,
            ) {
                Ok(res) => {
                    append_cronos_wal(
                        &repo_dir,
                        "cascade",
                        &format!("Cascaded '{}' across parallel dimensions", source),
                        None,
                    );
                    send_json(stream, 200, &res, None);
                }
                Err(e) => send_json_error(stream, 400, &e),
            }
        }

        // --- Surgical Commit Splice ('dft splice') ---
        ("POST", ["splice"]) => {
            #[derive(Deserialize)]
            struct SpliceReq {
                commit: String,
                into: String,
            }
            let body: SpliceReq = match req.json() {
                Ok(b) => b,
                Err(e) => return send_json_error(stream, 400, &e),
            };
            let actor_name = user
                .map(|u| u.display_name.clone())
                .unwrap_or_else(|| "Draft Swarm".to_string());
            let actor_email = user
                .map(|u| u.email.clone())
                .unwrap_or_else(|| "swarm@draft.local".to_string());

            let _guard = state.repo_lock.write().ok();
            match execute_server_splice(
                &repo,
                &repo_dir,
                &meta.default_branch,
                &body.commit,
                &body.into,
                &actor_name,
                &actor_email,
            ) {
                Ok(res) => send_json(stream, 200, &res, None),
                Err(e) => send_json_error(stream, 400, &e),
            }
        }

        // --- Territory Claims & Hard Fences ('dft claim' / 'dft yield' / 'dft fence') ---
        ("POST", ["territory", "claim"]) => {
            #[derive(Deserialize)]
            struct ClaimReq {
                path: String,
                dimension: String,
                agent: Option<String>,
                exclusive: Option<bool>,
            }
            let body: ClaimReq = match req.json() {
                Ok(b) => b,
                Err(e) => return send_json_error(stream, 400, &e),
            };
            let p = body.path.trim().trim_start_matches('/').to_string();
            if p.is_empty() {
                return send_json_error(stream, 400, "Path is required");
            }
            let _guard = state.repo_lock.write().ok();
            let terr_dir = repo_dir.join("territory");
            let _ = fs::create_dir_all(&terr_dir);
            let mut claims: serde_json::Map<String, serde_json::Value> =
                read_json_file(&terr_dir.join("claims.json")).unwrap_or_default();
            let actor = body
                .agent
                .filter(|s| !s.trim().is_empty())
                .or_else(|| user.map(|u| u.username.clone()))
                .unwrap_or_else(|| "agent-swarm".to_string());
            claims.insert(
                p.clone(),
                serde_json::json!({
                    "path": p,
                    "dimension": body.dimension.trim(),
                    "agent": actor,
                    "exclusive": body.exclusive.unwrap_or(true),
                    "claimed_at": Utc::now().to_rfc3339(),
                }),
            );
            let _ = write_json_atomic(&terr_dir.join("claims.json"), &claims);
            send_json(
                stream,
                200,
                &serde_json::json!({ "ok": true, "claims": claims }),
                None,
            );
        }
        ("POST", ["territory", "yield"]) => {
            #[derive(Deserialize)]
            struct YieldReq {
                path: String,
            }
            let body: YieldReq = match req.json() {
                Ok(b) => b,
                Err(e) => return send_json_error(stream, 400, &e),
            };
            let _guard = state.repo_lock.write().ok();
            let terr_dir = repo_dir.join("territory");
            let mut claims: serde_json::Map<String, serde_json::Value> =
                read_json_file(&terr_dir.join("claims.json")).unwrap_or_default();
            claims.remove(body.path.trim().trim_start_matches('/'));
            let _ = write_json_atomic(&terr_dir.join("claims.json"), &claims);
            send_json(
                stream,
                200,
                &serde_json::json!({ "ok": true, "claims": claims }),
                None,
            );
        }
        ("POST", ["territory", "fence"]) => {
            #[derive(Deserialize)]
            struct FenceReq {
                path: String,
                dimension: Option<String>,
                reason: Option<String>,
                action: Option<String>, // "add" | "remove"
            }
            let body: FenceReq = match req.json() {
                Ok(b) => b,
                Err(e) => return send_json_error(stream, 400, &e),
            };
            let p = body.path.trim().trim_start_matches('/').to_string();
            let _guard = state.repo_lock.write().ok();
            let terr_dir = repo_dir.join("territory");
            let _ = fs::create_dir_all(&terr_dir);
            let mut fences: Vec<serde_json::Value> =
                read_json_file(&terr_dir.join("fences.json")).unwrap_or_default();
            fences.retain(|f| {
                let fp = f
                    .get("path")
                    .and_then(|v| v.as_str())
                    .or_else(|| f.as_str())
                    .unwrap_or("");
                fp != p
            });
            if body.action.as_deref() != Some("remove") {
                fences.push(serde_json::json!({
                    "path": p,
                    "dimension": body.dimension.unwrap_or_else(|| meta.default_branch.clone()),
                    "reason": body.reason.unwrap_or_else(|| "Protected boundary".to_string()),
                    "created_at": Utc::now().to_rfc3339(),
                }));
            }
            let _ = write_json_atomic(&terr_dir.join("fences.json"), &fences);
            send_json(
                stream,
                200,
                &serde_json::json!({ "ok": true, "fences": fences }),
                None,
            );
        }

        // --- Quantum Entanglement Rules ('dft entangle') ---
        ("GET", ["entangle"]) => {
            send_json(
                stream,
                200,
                &serde_json::json!({ "entanglements": load_entanglements_json(&repo_dir) }),
                None,
            );
        }
        ("POST", ["entangle"]) => {
            #[derive(Deserialize)]
            struct EntReq {
                dim1: String,
                dim2: String,
                paths: Option<String>,
            }
            let body: EntReq = match req.json() {
                Ok(b) => b,
                Err(e) => return send_json_error(stream, 400, &e),
            };
            if body.dim1.trim() == body.dim2.trim() {
                return send_json_error(stream, 400, "Select two distinct dimensions to entangle");
            }
            let _guard = state.repo_lock.write().ok();
            let ent_dir = repo_dir.join("entangle");
            let _ = fs::create_dir_all(&ent_dir);
            let mut rules = load_entanglements_json(&repo_dir);
            let rule = serde_json::json!({
                "id": generate_token_hex(6),
                "dim1": body.dim1.trim(),
                "dim2": body.dim2.trim(),
                "paths": body.paths.filter(|s| !s.trim().is_empty()).unwrap_or_else(|| "*".to_string()),
                "direction": "bidirectional",
                "created_at": Utc::now().to_rfc3339(),
            });
            rules.push(rule.clone());
            let _ = write_json_atomic(&ent_dir.join("rules.json"), &rules);
            let propagated = propagate_entanglements_from(&repo, &repo_dir, body.dim1.trim());
            append_cronos_wal(
                &repo_dir,
                "entangle",
                &format!(
                    "Linked {} <-> {} ({} files synced)",
                    body.dim1.trim(),
                    body.dim2.trim(),
                    propagated
                ),
                None,
            );
            send_json(
                stream,
                201,
                &serde_json::json!({ "ok": true, "rule": rule, "propagated_files": propagated }),
                None,
            );
        }
        ("DELETE", ["entangle", rule_id]) => {
            let _guard = state.repo_lock.write().ok();
            let ent_dir = repo_dir.join("entangle");
            let mut rules = load_entanglements_json(&repo_dir);
            rules.retain(|r| r.get("id").and_then(|v| v.as_str()) != Some(*rule_id));
            let _ = write_json_atomic(&ent_dir.join("rules.json"), &rules);
            send_json(stream, 200, &serde_json::json!({ "ok": true }), None);
        }

        // --- Autonomous Cronos Reconciliation Engine & WAL ('dft cronos') ---
        ("GET", ["cronos"]) => {
            send_json(
                stream,
                200,
                &serde_json::json!({ "wal": load_cronos_log(&repo_dir) }),
                None,
            );
        }
        ("POST", ["cronos"]) => {
            let _guard = state.repo_lock.write().ok();
            let mut synced_files = 0usize;
            for d in list_dimension_items(&repo) {
                if let Some(dn) = d["name"].as_str() {
                    synced_files += propagate_entanglements_from(&repo, &repo_dir, dn);
                }
            }
            synced_files += propagate_entanglements_from(&repo, &repo_dir, &meta.default_branch);
            append_cronos_wal(
                &repo_dir,
                "cronos_sweep",
                &format!(
                    "Autonomous Cronos sweep verified CAS & propagated {} entangled file(s)",
                    synced_files
                ),
                None,
            );
            send_json(
                stream,
                200,
                &serde_json::json!({
                    "ok": true,
                    "synced_files": synced_files,
                    "wal": load_cronos_log(&repo_dir),
                }),
                None,
            );
        }

        // --- AI Agent Swarm Registry, Heartbeats & Maildir Inbox ('dft agent') ---
        ("GET", ["agents"]) => {
            send_json(
                stream,
                200,
                &serde_json::json!({
                    "agents": load_agents_json(&repo_dir),
                    "messages": load_messages_json(&repo_dir),
                }),
                None,
            );
        }
        ("POST", ["agents"]) => {
            #[derive(Deserialize)]
            struct RegAgent {
                name: String,
                agent_type: Option<String>,
                assigned_dimension: Option<String>,
                capabilities: Option<Vec<String>>,
            }
            let body: RegAgent = match req.json() {
                Ok(b) => b,
                Err(e) => return send_json_error(stream, 400, &e),
            };
            let slug = sanitize_slug(&body.name);
            if slug.is_empty() {
                return send_json_error(stream, 400, "Agent name is required");
            }
            let _guard = state.repo_lock.write().ok();
            let agents_dir = repo_dir.join("agents");
            let _ = fs::create_dir_all(&agents_dir);
            let now = Utc::now().to_rfc3339();
            let dim_val = body
                .assigned_dimension
                .filter(|s| !s.trim().is_empty())
                .unwrap_or_else(|| "mainline".to_string());
            let record = serde_json::json!({
                "id": slug,
                "name": body.name.trim(),
                "agent_type": body.agent_type.unwrap_or_else(|| "ai".to_string()),
                "dimension": dim_val,
                "assigned_dimension": dim_val,
                "capabilities": body.capabilities.unwrap_or_default(),
                "status": "active",
                "last_heartbeat": now,
                "registered_at": now,
            });
            let _ = write_json_atomic(&agents_dir.join(format!("{}.json", slug)), &record);
            send_json(stream, 201, &record, None);
        }
        ("POST", ["agents", "heartbeat"]) => {
            #[derive(Deserialize)]
            struct HbReq {
                name: String,
                status: Option<String>,
                dimension: Option<String>,
            }
            let body: HbReq = match req.json() {
                Ok(b) => b,
                Err(e) => return send_json_error(stream, 400, &e),
            };
            let slug = sanitize_slug(&body.name);
            let _guard = state.repo_lock.write().ok();
            let path = repo_dir.join("agents").join(format!("{}.json", slug));
            let mut rec: serde_json::Value = read_json_file(&path).unwrap_or_else(|| {
                serde_json::json!({
                    "id": slug,
                    "name": body.name,
                    "agent_type": "ai",
                    "assigned_dimension": "mainline",
                })
            });
            rec["last_heartbeat"] = serde_json::Value::String(Utc::now().to_rfc3339());
            rec["status"] =
                serde_json::Value::String(body.status.unwrap_or_else(|| "active".to_string()));
            if let Some(dim) = body.dimension.filter(|s| !s.trim().is_empty()) {
                rec["assigned_dimension"] = serde_json::Value::String(dim);
            }
            let _ = write_json_atomic(&path, &rec);
            send_json(stream, 200, &rec, None);
        }
        ("GET", ["messages"]) => {
            send_json(
                stream,
                200,
                &serde_json::json!({ "messages": load_messages_json(&repo_dir) }),
                None,
            );
        }
        ("POST", ["messages"]) => {
            #[derive(Deserialize)]
            struct MsgReq {
                sender: Option<String>,
                recipient: Option<String>, // "*" or "broadcast" for swarm-wide
                subject: Option<String>,
                body: String,
            }
            let body: MsgReq = match req.json() {
                Ok(b) => b,
                Err(e) => return send_json_error(stream, 400, &e),
            };
            if body.body.trim().is_empty() {
                return send_json_error(stream, 400, "Message body is required");
            }
            let _guard = state.repo_lock.write().ok();
            let mut msgs = load_messages_json(&repo_dir);
            let msg = serde_json::json!({
                "id": generate_token_hex(6),
                "sender": body.sender.filter(|s| !s.trim().is_empty()).or_else(|| user.map(|u| u.username.clone())).unwrap_or_else(|| "coordinator".to_string()),
                "recipient": body.recipient.filter(|s| !s.trim().is_empty()).unwrap_or_else(|| "*".to_string()),
                "subject": body.subject.unwrap_or_else(|| "swarm-sync".to_string()),
                "body": body.body.trim(),
                "created_at": Utc::now().to_rfc3339(),
            });
            msgs.insert(0, msg.clone());
            if msgs.len() > 200 {
                msgs.truncate(200);
            }
            let _ = write_json_atomic(&repo_dir.join("messages/swarm_inbox.json"), &msgs);
            send_json(stream, 201, &msg, None);
        }

        // --- Immutable Dimension Snapshots ('dft snapshot') ---
        ("GET", ["snapshots"]) => {
            send_json(
                stream,
                200,
                &serde_json::json!({ "snapshots": list_snapshots_json(&repo) }),
                None,
            );
        }
        ("POST", ["snapshots"]) => {
            #[derive(Deserialize, Default)]
            struct SnapReq {
                dimension: Option<String>,
                all: Option<bool>,
                label: Option<String>,
            }
            let body: SnapReq = req.json().unwrap_or_default();
            let _guard = state.repo_lock.write().ok();
            let ts = Utc::now().format("%Y%m%d-%H%M%S").to_string();
            let suffix = body
                .label
                .as_deref()
                .map(sanitize_slug)
                .filter(|s| !s.is_empty())
                .unwrap_or(ts);

            let targets: Vec<String> = if body.all.unwrap_or(false) {
                let mut t = vec![meta.default_branch.clone()];
                for d in list_dimension_items(&repo) {
                    if let Some(n) = d["name"].as_str() {
                        t.push(n.to_string());
                    }
                }
                t
            } else {
                vec![body
                    .dimension
                    .filter(|s| !s.trim().is_empty())
                    .unwrap_or_else(|| meta.default_branch.clone())]
            };

            let mut created = Vec::new();
            for dim in targets {
                if let Some(oid) = resolve_any_ref(&repo, &dim) {
                    let snap_name = format!("{}/{}", sanitize_slug(&dim), suffix);
                    let p = repo_dir.join("refs/snapshots").join(&snap_name);
                    if let Some(parent) = p.parent() {
                        let _ = fs::create_dir_all(parent);
                    }
                    let _ = fs::write(&p, format!("{}\n", oid.to_hex()));
                    created.push(snap_name);
                }
            }
            send_json(
                stream,
                201,
                &serde_json::json!({ "ok": true, "created": created }),
                None,
            );
        }

        _ => send_json_error(stream, 404, "Repository API endpoint not found"),
    }
}

fn handle_raw_blob(
    stream: &mut TcpStream,
    req: &HttpRequest,
    state: &Arc<ServerState>,
    owner: &str,
    repo_name: &str,
    ref_and_path: &str,
) {
    let repo_dir = state.repo_dir(owner, repo_name);
    if !repo_dir.join("HEAD").exists() {
        send_response(stream, 404, "text/plain", b"Repository not found", None);
        return;
    }
    let meta = load_repo_meta(&repo_dir, owner, repo_name);
    let user = state.authenticate(req);
    if meta.private && !can_access_private(&meta, user.as_ref(), state) {
        send_response(stream, 401, "text/plain", b"Unauthorized", None);
        return;
    }
    let Ok(repo) = Repository::open(&repo_dir) else {
        send_response(stream, 500, "text/plain", b"Open error", None);
        return;
    };

    // Match longest ref prefix or first path segment
    let Some((ref_name, file_path)) = split_ref_and_filepath(&repo, ref_and_path) else {
        send_response(stream, 404, "text/plain", b"Ref/path not found", None);
        return;
    };

    let Some(commit_oid) = resolve_any_ref(&repo, &ref_name) else {
        send_response(stream, 404, "text/plain", b"Ref not found", None);
        return;
    };
    let Ok(rc) = repo.cas().read_raw(&commit_oid) else {
        send_response(stream, 404, "text/plain", b"Commit missing", None);
        return;
    };
    let Ok(commit) = Commit::deserialize(&rc.data) else {
        send_response(stream, 500, "text/plain", b"Corrupt commit", None);
        return;
    };
    let Some((mode, blob_oid)) = lookup_path_in_tree(repo.cas().as_ref(), &commit.tree, &file_path)
    else {
        send_response(stream, 404, "text/plain", b"File not found", None);
        return;
    };
    if mode.is_tree() {
        send_response(stream, 400, "text/plain", b"Path is a directory", None);
        return;
    }
    let Ok(raw_b) = repo.cas().read_raw(&blob_oid) else {
        send_response(stream, 404, "text/plain", b"Blob missing", None);
        return;
    };

    let mime = guess_mime(&file_path);
    send_response(
        stream,
        200,
        mime,
        if req.method == "HEAD" {
            &[]
        } else {
            &raw_b.data
        },
        None,
    );
}

// ============================================================================
// Repository & CAS Helper Functions
// ============================================================================

#[allow(clippy::too_many_arguments)]
fn create_bare_repository(
    state: &ServerState,
    owner: &str,
    name: &str,
    description: &str,
    private: bool,
    default_branch: &str,
    auto_init: bool,
    topics: Vec<String>,
    actor: &str,
) -> Result<RepoMeta, String> {
    let repo_dir = state.repo_dir(owner, name);
    if let Some(p) = repo_dir.parent() {
        fs::create_dir_all(p).map_err(|e| e.to_string())?;
    }

    let repo = init(
        &repo_dir,
        &InitOptions {
            bare: true,
            initial_branch: default_branch.to_string(),
            reinit: false,
        },
    )
    .map_err(|e| e.to_string())?;

    let now = Utc::now().to_rfc3339();
    let meta = RepoMeta {
        owner: sanitize_slug(owner),
        name: sanitize_slug(name),
        description: description.to_string(),
        private,
        default_branch: default_branch.to_string(),
        topics,
        stars: Vec::new(),
        forks_count: 0,
        forked_from: None,
        migrated_from: None,
        created_at: now.clone(),
        updated_at: now.clone(),
    };
    save_repo_meta(&repo_dir, &meta).map_err(|e| e.to_string())?;

    if auto_init {
        let mut files = BTreeMap::new();
        let readme = format!(
            "# {}\n\n{}\n\n## Quick Start with Draft (`dft`)\n\n```bash\ndft clone <SERVER_URL>/{}/{}.dft\ncd {}\ndft dim new agent-alpha\n```\n",
            meta.name,
            if description.is_empty() {
                "Hosted on DraftHub — The Multiverse Version Control Server."
            } else {
                description
            },
            meta.owner,
            meta.name,
            meta.name
        );
        let ignore_body = "target/\nnode_modules/\n.DS_Store\n*.log\n";
        let readme_oid = repo
            .cas()
            .write_blob(readme.as_bytes())
            .map_err(|e| e.to_string())?;
        let ignore_oid = repo
            .cas()
            .write_blob(ignore_body.as_bytes())
            .map_err(|e| e.to_string())?;
        files.insert("README.md".to_string(), (FileMode::REGULAR, readme_oid));
        files.insert(".dftignore".to_string(), (FileMode::REGULAR, ignore_oid));

        let tree_oid =
            build_hierarchical_tree(repo.cas().as_ref(), &files).map_err(|e| e.to_string())?;
        let sig = Signature::now(actor, format!("{}@draft.local", actor));
        let commit = Commit::new(
            tree_oid,
            Vec::new(),
            sig.clone(),
            sig,
            "chore: initial commit",
        );
        let raw_c = RawObject::new(ObjectType::Commit, commit.serialize());
        let c_oid = repo.cas().write_raw(&raw_c).map_err(|e| e.to_string())?;

        let branch_ref = repo_dir.join("refs/heads").join(default_branch);
        if let Some(p) = branch_ref.parent() {
            let _ = fs::create_dir_all(p);
        }
        fs::write(branch_ref, format!("{}\n", c_oid.to_hex())).map_err(|e| e.to_string())?;
    }

    state.record_activity(ActivityEvent {
        id: generate_token_hex(8),
        event_type: "create_repo".to_string(),
        actor: actor.to_string(),
        owner: meta.owner.clone(),
        repo: meta.name.clone(),
        ref_name: Some(default_branch.to_string()),
        commit_id: None,
        summary: format!("created repository {}/{}", meta.owner, meta.name),
        created_at: now,
    });

    Ok(meta)
}

fn migrate_external_repository(
    state: &ServerState,
    clone_addr: &str,
    owner: &str,
    name: &str,
    description: &str,
    private: bool,
    actor: &str,
) -> Result<RepoMeta, String> {
    let mut tmp_clone: Option<PathBuf> = None;
    let src_dir = if clone_addr.starts_with("http://")
        || clone_addr.starts_with("https://")
        || clone_addr.starts_with("git@")
    {
        let tmp = env::temp_dir().join(format!("dft-migrate-{}", generate_token_hex(8)));
        let out = Command::new("git")
            .args(["clone", "--depth", "1", clone_addr, &tmp.to_string_lossy()])
            .output()
            .map_err(|e| format!("Failed to invoke git clone: {}", e))?;
        if !out.status.success() {
            let _ = fs::remove_dir_all(&tmp);
            return Err(format!(
                "git clone failed: {}",
                String::from_utf8_lossy(&out.stderr).trim()
            ));
        }
        tmp_clone = Some(tmp.clone());
        tmp
    } else {
        let p = PathBuf::from(clone_addr);
        if !p.exists() {
            return Err(format!("Local path '{}' does not exist", clone_addr));
        }
        p
    };

    let mut meta = create_bare_repository(
        state,
        owner,
        name,
        description,
        private,
        "main",
        false,
        vec!["migrated".to_string()],
        actor,
    )?;
    let repo_dir = state.repo_dir(owner, name);
    let repo = Repository::open(&repo_dir).map_err(|e| e.to_string())?;

    let mut files = BTreeMap::new();
    let ignore = daft_core::worktree::DaftIgnore::load_from_workdir(&src_dir);
    for entry in walkdir::WalkDir::new(&src_dir)
        .into_iter()
        .filter_entry(|e| {
            let fname = e.file_name().to_string_lossy();
            fname != ".git" && fname != ".dft" && !ignore.should_prune_entry(e, &src_dir)
        })
        .flatten()
    {
        if entry.file_type().is_file() {
            if let Ok(rel) = entry.path().strip_prefix(&src_dir) {
                let rel_str = rel.to_string_lossy().replace('\\', "/");
                if !rel_str.is_empty() && !ignore.is_ignored(&rel_str, false) {
                    if let Ok(bytes) = fs::read(entry.path()) {
                        if let Ok(blob_oid) = repo.cas().write_blob(&bytes) {
                            files.insert(rel_str, (FileMode::REGULAR, blob_oid));
                        }
                    }
                }
            }
        }
    }

    if let Some(tmp) = tmp_clone {
        let _ = fs::remove_dir_all(tmp);
    }

    if !files.is_empty() {
        let tree_oid =
            build_hierarchical_tree(repo.cas().as_ref(), &files).map_err(|e| e.to_string())?;
        let sig = Signature::now(actor, format!("{}@draft.local", actor));
        let commit = Commit::new(
            tree_oid,
            Vec::new(),
            sig.clone(),
            sig,
            format!("chore: migrated from {}", clone_addr),
        );
        let raw_c = RawObject::new(ObjectType::Commit, commit.serialize());
        let c_oid = repo.cas().write_raw(&raw_c).map_err(|e| e.to_string())?;
        let branch_ref = repo_dir.join("refs/heads/main");
        if let Some(p) = branch_ref.parent() {
            let _ = fs::create_dir_all(p);
        }
        let _ = fs::write(branch_ref, format!("{}\n", c_oid.to_hex()));
    }

    meta.migrated_from = Some(clone_addr.to_string());
    meta.updated_at = Utc::now().to_rfc3339();
    let _ = save_repo_meta(&repo_dir, &meta);

    Ok(meta)
}

fn collect_all_repo_refs(repo: &Repository) -> BTreeMap<String, String> {
    let mut refs = BTreeMap::new();
    for prefix in ["refs/heads", "refs/tags", "refs/dimensions"] {
        if let Ok(list) = repo.refs().list_refs(prefix) {
            for r in list {
                if let Ok(oid) = repo.refs().resolve(&r.name) {
                    refs.insert(r.name, oid.to_hex());
                }
            }
        }
    }
    refs
}

fn resolve_any_ref(repo: &Repository, rev: &str) -> Option<ObjectId> {
    let clean = rev.trim();
    if clean.is_empty() {
        return None;
    }
    if clean == "HEAD" {
        if let Ok(oid) = repo.refs().resolve("HEAD") {
            return Some(oid);
        }
    }
    if clean == "mainline" {
        for cand in [
            "refs/dimensions/mainline",
            "refs/heads/mainline",
            "refs/heads/main",
            "HEAD",
        ] {
            if let Ok(oid) = repo.refs().resolve(cand) {
                return Some(oid);
            }
        }
    }
    for candidate in [
        clean.to_string(),
        format!("refs/heads/{}", clean),
        format!("refs/dimensions/{}", clean),
        format!("refs/tags/{}", clean),
        format!("refs/snapshots/{}", clean),
    ] {
        if let Ok(oid) = repo.refs().resolve(&candidate) {
            return Some(oid);
        }
    }
    if clean.len() == 64 {
        if let Ok(oid) = ObjectId::from_hex(clean) {
            if repo.cas().has_object(&oid) {
                return Some(oid);
            }
        }
    } else if clean.len() >= 6 && clean.chars().all(|c| c.is_ascii_hexdigit()) {
        if let Ok(all) = repo.cas().list_objects() {
            for oid in all {
                if oid.to_hex().starts_with(clean) {
                    return Some(oid);
                }
            }
        }
    }
    None
}

fn split_ref_and_filepath(repo: &Repository, ref_and_path: &str) -> Option<(String, String)> {
    let refs = collect_all_repo_refs(repo);
    let mut known_names: Vec<String> = refs
        .keys()
        .map(|k| {
            k.strip_prefix("refs/heads/")
                .or_else(|| k.strip_prefix("refs/dimensions/"))
                .or_else(|| k.strip_prefix("refs/tags/"))
                .unwrap_or(k)
                .to_string()
        })
        .collect();
    // Sort longest first so multi-segment branch/dimension names like 'agent-coder/auth' match cleanly!
    known_names.sort_by_key(|b| std::cmp::Reverse(b.len()));

    for rname in known_names {
        let prefix = format!("{}/", rname);
        if let Some(rest) = ref_and_path.strip_prefix(&prefix) {
            if !rest.is_empty() {
                return Some((rname, rest.to_string()));
            }
        }
    }
    let (first, rest) = ref_and_path.split_once('/')?;
    Some((first.to_string(), rest.to_string()))
}

fn lookup_path_in_tree(
    cas: &daft_core::cas::ObjectStore,
    root_tree_oid: &ObjectId,
    rel_path: &str,
) -> Option<(FileMode, ObjectId)> {
    let parts: Vec<&str> = rel_path.split('/').filter(|s| !s.is_empty()).collect();
    if parts.is_empty() {
        return Some((FileMode::TREE, *root_tree_oid));
    }
    let mut current_tree_oid = *root_tree_oid;
    for (idx, comp) in parts.iter().enumerate() {
        let raw = cas.read_raw(&current_tree_oid).ok()?;
        let tree = Tree::deserialize(&raw.data).ok()?;
        let entry = tree.get(comp)?;
        if idx + 1 == parts.len() {
            return Some((entry.mode, entry.oid));
        }
        if !entry.mode.is_tree() {
            return None;
        }
        current_tree_oid = entry.oid;
    }
    None
}

fn read_commit_json(repo: &Repository, oid: &ObjectId) -> Option<serde_json::Value> {
    let raw = repo.cas().read_raw(oid).ok()?;
    if raw.object_type != ObjectType::Commit {
        return None;
    }
    let c = Commit::deserialize(&raw.data).ok()?;
    let hex_id = oid.to_hex();
    let short_id = hex_id[..hex_id.len().min(8)].to_string();
    let summary = c.message.lines().next().unwrap_or("").trim().to_string();
    let dt = chrono::DateTime::<Utc>::from_timestamp(c.author.time, 0)
        .unwrap_or_else(Utc::now)
        .to_rfc3339();
    Some(serde_json::json!({
        "id": hex_id,
        "hash": hex_id,
        "short_id": short_id,
        "short_hash": short_id,
        "tree": c.tree.to_hex(),
        "parents": c.parents.iter().map(|p| p.to_hex()).collect::<Vec<_>>(),
        "author": c.author.name,
        "author_name": c.author.name,
        "email": c.author.email,
        "author_email": c.author.email,
        "timestamp": c.author.time,
        "date": dt,
        "summary": summary,
        "message": c.message.trim(),
    }))
}

fn list_ref_items(
    repo: &Repository,
    prefix: &str,
    default_branch: Option<&str>,
) -> Vec<serde_json::Value> {
    let mut out = Vec::new();
    let strip_p = format!("{}/", prefix);
    if let Ok(list) = repo.refs().list_refs(prefix) {
        for r in list {
            if let Ok(oid) = repo.refs().resolve(&r.name) {
                let short_name = r.name.strip_prefix(&strip_p).unwrap_or(&r.name).to_string();
                let commit_info = read_commit_json(repo, &oid);
                out.push(serde_json::json!({
                    "name": short_name,
                    "commit": oid.to_hex(),
                    "short_commit": &oid.to_hex()[..8],
                    "is_default": default_branch == Some(short_name.as_str()),
                    "latest_commit": commit_info,
                }));
            }
        }
    }
    out
}

fn list_dimension_items(repo: &Repository) -> Vec<serde_json::Value> {
    let mut map: BTreeMap<String, ObjectId> = BTreeMap::new();
    if let Ok(list) = repo.refs().list_refs("refs/dimensions") {
        for r in list {
            if let Ok(oid) = repo.refs().resolve(&r.name) {
                let name = r
                    .name
                    .strip_prefix("refs/dimensions/")
                    .unwrap_or(&r.name)
                    .to_string();
                map.insert(name, oid);
            }
        }
    }
    let dims_dir = repo.dft_dir().join("dimensions");
    if dims_dir.is_dir() {
        if let Ok(entries) = fs::read_dir(dims_dir) {
            for e in entries.flatten() {
                if e.path().is_dir() {
                    let name = e.file_name().to_string_lossy().to_string();
                    if let Ok(s) = fs::read_to_string(e.path().join("meta.json")) {
                        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&s) {
                            if let Some(h) = v.get("head_commit").and_then(|x| x.as_str()) {
                                if let Ok(oid) = ObjectId::from_hex(h) {
                                    map.entry(name).or_insert(oid);
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    map.into_iter()
        .map(|(name, oid)| {
            serde_json::json!({
                "name": name,
                "commit": oid.to_hex(),
                "short_commit": &oid.to_hex()[..8],
                "latest_commit": read_commit_json(repo, &oid),
            })
        })
        .collect()
}

fn summarize_patches(patches: &[daft_core::diff::FilePatch]) -> (usize, usize, usize) {
    let mut files = 0usize;
    let mut add = 0usize;
    let mut del = 0usize;
    for p in patches {
        if p.is_empty() {
            continue;
        }
        files += 1;
        for h in &p.hunks {
            for l in &h.lines {
                match l.kind {
                    EditOpKind::Insert => add += 1,
                    EditOpKind::Delete => del += 1,
                    EditOpKind::Equal => {}
                }
            }
        }
    }
    (files, add, del)
}

fn compute_foresee_badge(repo: &Repository, base_ref: &str, head_ref: &str) -> serde_json::Value {
    let Some(base_oid) = resolve_any_ref(repo, base_ref) else {
        return serde_json::json!({ "valid": false });
    };
    let Some(head_oid) = resolve_any_ref(repo, head_ref) else {
        return serde_json::json!({ "valid": false });
    };
    let cas = repo.cas().as_ref();
    let graph = StoreCommitGraph::new(cas);
    let lcas = all_merge_bases(&graph, &base_oid, &head_oid).unwrap_or_default();
    let lca_tree = lcas
        .first()
        .and_then(|id| cas.read_raw(id).ok())
        .and_then(|r| Commit::deserialize(&r.data).ok())
        .map(|c| c.tree);

    let base_tree = cas
        .read_raw(&base_oid)
        .ok()
        .and_then(|r| Commit::deserialize(&r.data).ok())
        .map(|c| c.tree);
    let head_tree = cas
        .read_raw(&head_oid)
        .ok()
        .and_then(|r| Commit::deserialize(&r.data).ok())
        .map(|c| c.tree);

    let (Some(bt), Some(ht)) = (base_tree, head_tree) else {
        return serde_json::json!({ "valid": false });
    };

    let res = merge_trees_3way(cas, lca_tree.as_ref(), &bt, &ht, base_ref, head_ref);
    let (has_conflicts, conflicting_files) = match res {
        Ok(r) => (
            r.has_conflicts(),
            r.conflicted_files.keys().cloned().collect::<Vec<_>>(),
        ),
        Err(_) => (false, Vec::new()),
    };

    let entropy = estimate_divergence_entropy(cas, &bt, &ht, conflicting_files.len());
    let risk_tier = if entropy == 0.0 {
        "Identical"
    } else if entropy <= 0.15 {
        "Negligible"
    } else if entropy <= 0.35 {
        "Low"
    } else if entropy <= 0.65 {
        "Moderate"
    } else {
        "High"
    };
    serde_json::json!({
        "valid": true,
        "has_conflicts": has_conflicts,
        "conflicting_files": conflicting_files,
        "entropy": entropy,
        "risk_tier": risk_tier,
    })
}

fn write_dimension_or_head_ref(
    repo_dir: &Path,
    default_branch: &str,
    target_name: &str,
    commit_oid: &ObjectId,
) {
    let resolved_name = if target_name == "mainline"
        && !repo_dir.join("refs/dimensions/mainline").exists()
        && !repo_dir.join("refs/heads/mainline").exists()
    {
        default_branch
    } else {
        target_name
    };
    let dim_ref = repo_dir.join("refs/dimensions").join(resolved_name);
    let ref_file = if dim_ref.exists() {
        dim_ref
    } else {
        repo_dir.join("refs/heads").join(resolved_name)
    };
    if let Some(p) = ref_file.parent() {
        let _ = fs::create_dir_all(p);
    }
    let _ = fs::write(&ref_file, format!("{}\n", commit_oid.to_hex()));
}

fn check_fence_violation(repo_dir: &Path, file_path: &str, current_dim: &str) -> Option<String> {
    let default_branch = load_repo_meta(repo_dir, "", "").default_branch;
    let norm = |d: &str| -> String {
        if d == "mainline" {
            default_branch.clone()
        } else {
            d.to_string()
        }
    };
    let cur_norm = norm(current_dim);
    let fences: Vec<serde_json::Value> =
        read_json_file(&repo_dir.join("territory/fences.json")).unwrap_or_default();
    for f in fences {
        let fpath = f
            .get("path")
            .and_then(|v| v.as_str())
            .or_else(|| f.as_str())
            .unwrap_or("");
        let owner = f
            .get("dimension")
            .and_then(|v| v.as_str())
            .unwrap_or("mainline");
        if !fpath.is_empty() && norm(owner) != cur_norm {
            let clean_f = fpath.trim_end_matches('*').trim_end_matches('/');
            if file_path == fpath || (!clean_f.is_empty() && file_path.starts_with(clean_f)) {
                return Some(owner.to_string());
            }
        }
    }
    None
}

fn load_entanglements_json(repo_dir: &Path) -> Vec<serde_json::Value> {
    read_json_file(&repo_dir.join("entangle/rules.json")).unwrap_or_default()
}

fn load_cronos_log(repo_dir: &Path) -> Vec<serde_json::Value> {
    read_json_file(&repo_dir.join("cronos/wal.json")).unwrap_or_default()
}

fn append_cronos_wal(repo_dir: &Path, action: &str, detail: &str, commit: Option<&str>) {
    let mut wal = load_cronos_log(repo_dir);
    wal.insert(
        0,
        serde_json::json!({
            "id": generate_token_hex(6),
            "action": action,
            "detail": detail,
            "commit": commit,
            "timestamp": Utc::now().to_rfc3339(),
        }),
    );
    if wal.len() > 120 {
        wal.truncate(120);
    }
    let _ = write_json_atomic(&repo_dir.join("cronos/wal.json"), &wal);
}

fn load_messages_json(repo_dir: &Path) -> Vec<serde_json::Value> {
    read_json_file(&repo_dir.join("messages/swarm_inbox.json")).unwrap_or_default()
}

fn list_snapshots_json(repo: &Repository) -> Vec<serde_json::Value> {
    let mut out = Vec::new();
    if let Ok(list) = repo.refs().list_refs("refs/snapshots") {
        for r in list {
            if let Ok(oid) = repo.refs().resolve(&r.name) {
                let short = r
                    .name
                    .strip_prefix("refs/snapshots/")
                    .unwrap_or(&r.name)
                    .to_string();
                out.push(serde_json::json!({
                    "name": short,
                    "commit": oid.to_hex(),
                    "short_commit": &oid.to_hex()[..8],
                    "latest_commit": read_commit_json(repo, &oid),
                }));
            }
        }
    }
    out
}

fn path_matches_pattern(path: &str, pat: &str) -> bool {
    let p = pat.trim();
    if p.is_empty() || p == "*" || p == "**" || p == "**/*" {
        return true;
    }
    if let Some(prefix) = p.strip_suffix("/*").or_else(|| p.strip_suffix("/**")) {
        return path == prefix || path.starts_with(&format!("{}/", prefix));
    }
    if let Some(prefix) = p.strip_suffix('*') {
        return path.starts_with(prefix);
    }
    path == p || path.starts_with(&format!("{}/", p))
}

fn propagate_entanglements_from(repo: &Repository, repo_dir: &Path, source_dim: &str) -> usize {
    let rules = load_entanglements_json(repo_dir);
    if rules.is_empty() {
        return 0;
    }
    let Some(src_oid) = resolve_any_ref(repo, source_dim) else {
        return 0;
    };
    let Ok(src_raw) = repo.cas().read_raw(&src_oid) else {
        return 0;
    };
    let Ok(src_commit) = Commit::deserialize(&src_raw.data) else {
        return 0;
    };
    let mut src_files = BTreeMap::new();
    let _ = flatten_tree(repo.cas().as_ref(), &src_commit.tree, "", &mut src_files);

    let mut total_propagated = 0usize;
    for r in rules {
        let d1 = r.get("dim1").and_then(|v| v.as_str()).unwrap_or("");
        let d2 = r.get("dim2").and_then(|v| v.as_str()).unwrap_or("");
        let pat = r.get("paths").and_then(|v| v.as_str()).unwrap_or("*");
        let peer = if d1 == source_dim {
            d2
        } else if d2 == source_dim {
            d1
        } else {
            continue;
        };
        let Some(peer_oid) = resolve_any_ref(repo, peer) else {
            continue;
        };
        let Ok(peer_raw) = repo.cas().read_raw(&peer_oid) else {
            continue;
        };
        let Ok(peer_commit) = Commit::deserialize(&peer_raw.data) else {
            continue;
        };
        let mut peer_files = BTreeMap::new();
        let _ = flatten_tree(repo.cas().as_ref(), &peer_commit.tree, "", &mut peer_files);

        let mut changed = 0usize;
        for (fpath, (fmode, foid)) in &src_files {
            if path_matches_pattern(fpath, pat) && peer_files.get(fpath).map(|x| x.1) != Some(*foid)
            {
                peer_files.insert(fpath.clone(), (*fmode, *foid));
                changed += 1;
            }
        }
        if changed > 0 {
            if let Ok(new_tree) = build_hierarchical_tree(repo.cas().as_ref(), &peer_files) {
                let sig = Signature::now("Draft Entanglement Engine", "entangle@draft.local");
                let c = Commit::new(
                    new_tree,
                    vec![peer_oid],
                    sig.clone(),
                    sig,
                    format!(
                        "[entangle] Propagated {} file(s) from '{}'",
                        changed, source_dim
                    ),
                );
                let raw = RawObject::new(ObjectType::Commit, c.serialize());
                if let Ok(c_oid) = repo.cas().write_raw(&raw) {
                    let meta = load_repo_meta(repo_dir, "", "");
                    write_dimension_or_head_ref(repo_dir, &meta.default_branch, peer, &c_oid);
                    total_propagated += changed;
                }
            }
        }
    }
    total_propagated
}

fn compute_hot_zones(repo: &Repository, default_branch: &str) -> Vec<serde_json::Value> {
    let Some(base_oid) = resolve_any_ref(repo, default_branch) else {
        return Vec::new();
    };
    let cas = repo.cas().as_ref();
    let graph = StoreCommitGraph::new(cas);

    let mut dim_names = Vec::new();
    for b in list_ref_items(repo, "refs/heads", Some(default_branch)) {
        if let Some(n) = b["name"].as_str() {
            if n != default_branch {
                dim_names.push(n.to_string());
            }
        }
    }
    for d in list_dimension_items(repo) {
        if let Some(n) = d["name"].as_str() {
            if n != default_branch && !dim_names.contains(&n.to_string()) {
                dim_names.push(n.to_string());
            }
        }
    }

    let mut file_touches: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for dname in &dim_names {
        let Some(d_oid) = resolve_any_ref(repo, dname) else {
            continue;
        };
        let lca = merge_base(&graph, &base_oid, &d_oid)
            .ok()
            .flatten()
            .unwrap_or(base_oid);
        let lca_tree = cas
            .read_raw(&lca)
            .ok()
            .and_then(|r| Commit::deserialize(&r.data).ok())
            .map(|c| c.tree);
        let d_tree = cas
            .read_raw(&d_oid)
            .ok()
            .and_then(|r| Commit::deserialize(&r.data).ok())
            .map(|c| c.tree);
        let (Some(lt), Some(dt)) = (lca_tree, d_tree) else {
            continue;
        };
        let mut lf = BTreeMap::new();
        let mut df = BTreeMap::new();
        let _ = flatten_tree(cas, &lt, "", &mut lf);
        let _ = flatten_tree(cas, &dt, "", &mut df);

        let mut all_p: HashSet<&String> = HashSet::new();
        for k in lf.keys() {
            all_p.insert(k);
        }
        for k in df.keys() {
            all_p.insert(k);
        }
        for p in all_p {
            if lf.get(p).map(|x| x.1) != df.get(p).map(|x| x.1) {
                file_touches
                    .entry(p.clone())
                    .or_default()
                    .push(dname.clone());
            }
        }
    }

    let mut hot = Vec::new();
    for (path, dims) in file_touches {
        if dims.len() >= 2 {
            hot.push(serde_json::json!({
                "path": path,
                "touch_count": dims.len(),
                "dimensions": dims,
            }));
        }
    }
    hot
}

fn observe_file_across_dimensions(
    repo: &Repository,
    default_branch: &str,
    file_path: &str,
) -> Vec<serde_json::Value> {
    let mut refs = vec![(default_branch.to_string(), "mainline")];
    for d in list_dimension_items(repo) {
        if let Some(n) = d["name"].as_str() {
            if n != default_branch {
                refs.push((n.to_string(), "dimension"));
            }
        }
    }
    for b in list_ref_items(repo, "refs/heads", Some(default_branch)) {
        if let Some(n) = b["name"].as_str() {
            if !refs.iter().any(|(x, _)| x == n) {
                refs.push((n.to_string(), "branch"));
            }
        }
    }

    let cas = repo.cas().as_ref();
    let mut out = Vec::new();
    for (name, kind) in refs {
        let Some(c_oid) = resolve_any_ref(repo, &name) else {
            continue;
        };
        let Ok(rc) = cas.read_raw(&c_oid) else {
            continue;
        };
        let Ok(commit) = Commit::deserialize(&rc.data) else {
            continue;
        };
        match lookup_path_in_tree(cas, &commit.tree, file_path) {
            Some((mode, b_oid)) if !mode.is_tree() => {
                if let Ok(rb) = cas.read_raw(&b_oid) {
                    let txt = String::from_utf8_lossy(&rb.data).to_string();
                    out.push(serde_json::json!({
                        "dimension": name,
                        "kind": kind,
                        "exists": true,
                        "oid": b_oid.to_hex(),
                        "short_oid": &b_oid.to_hex()[..8],
                        "lines": txt.lines().count(),
                        "size": rb.data.len(),
                        "content": txt,
                    }));
                }
            }
            _ => {
                out.push(serde_json::json!({
                    "dimension": name,
                    "kind": kind,
                    "exists": false,
                }));
            }
        }
    }
    out
}

#[allow(clippy::too_many_arguments)]
fn execute_server_multi_converge(
    repo: &Repository,
    repo_dir: &Path,
    default_branch: &str,
    target: &str,
    dimensions: &[String],
    dry_run: bool,
    is_collapse: bool,
    custom_msg: Option<&str>,
    actor_name: &str,
    actor_email: &str,
) -> Result<serde_json::Value, String> {
    let cas = repo.cas().as_ref();
    let graph = StoreCommitGraph::new(cas);

    let target_oid = resolve_any_ref(repo, target)
        .ok_or_else(|| format!("Target dimension/branch '{}' not found", target))?;
    let target_raw = cas.read_raw(&target_oid).map_err(|e| e.to_string())?;
    let target_commit = Commit::deserialize(&target_raw.data).map_err(|e| e.to_string())?;

    let mut current_tree = target_commit.tree;
    let mut parents = vec![target_oid];
    let mut converged = Vec::new();
    let mut skipped = Vec::new();

    for dim in dimensions {
        if dim == target {
            continue;
        }
        let Some(d_oid) = resolve_any_ref(repo, dim) else {
            continue;
        };
        if d_oid == target_oid || is_ancestor(&graph, &d_oid, &target_oid).unwrap_or(false) {
            continue;
        }
        let Ok(d_raw) = cas.read_raw(&d_oid) else {
            continue;
        };
        let Ok(d_commit) = Commit::deserialize(&d_raw.data) else {
            continue;
        };
        let lca_tree = all_merge_bases(&graph, &target_oid, &d_oid)
            .ok()
            .and_then(|v| v.into_iter().next())
            .and_then(|id| cas.read_raw(&id).ok())
            .and_then(|r| Commit::deserialize(&r.data).ok())
            .map(|c| c.tree);

        let mres = merge_trees_3way(
            cas,
            lca_tree.as_ref(),
            &current_tree,
            &d_commit.tree,
            target,
            dim,
        )
        .map_err(|e| e.to_string())?;

        if mres.has_conflicts() {
            let cfiles: Vec<String> = mres.conflicted_files.keys().cloned().collect();
            skipped.push(serde_json::json!({
                "dimension": dim,
                "conflicting_files": cfiles,
            }));
        } else {
            current_tree = mres.write_clean_tree(cas).map_err(|e| e.to_string())?;
            if !parents.contains(&d_oid) {
                parents.push(d_oid);
            }
            converged.push(dim.clone());
        }
    }

    if dry_run {
        return Ok(serde_json::json!({
            "ok": true,
            "dry_run": true,
            "target": target,
            "converged_dimensions": converged,
            "skipped_conflicting": skipped,
            "clean_ready": skipped.is_empty(),
        }));
    }

    if converged.is_empty() {
        return Ok(serde_json::json!({
            "ok": true,
            "noop": true,
            "target": target,
            "converged_dimensions": converged,
            "skipped_conflicting": skipped,
            "message": if skipped.is_empty() {
                "All parallel dimensions are already converged into target"
            } else {
                "Remaining dimensions have conflicts — see skipped_conflicting"
            },
        }));
    }

    let sig = Signature::now(actor_name, actor_email);
    let prefix_tag = if is_collapse {
        "[collapse]"
    } else {
        "[converge]"
    };
    let msg = custom_msg
        .filter(|s| !s.trim().is_empty())
        .map(String::from)
        .unwrap_or_else(|| {
            format!(
                "{} Reconcile [{}] into {}",
                prefix_tag,
                converged.join(", "),
                target
            )
        });

    let commit = Commit::new(current_tree, parents, sig.clone(), sig, msg);
    let raw = RawObject::new(ObjectType::Commit, commit.serialize());
    let commit_oid = cas.write_raw(&raw).map_err(|e| e.to_string())?;
    write_dimension_or_head_ref(repo_dir, default_branch, target, &commit_oid);

    Ok(serde_json::json!({
        "ok": true,
        "dry_run": false,
        "commit_id": commit_oid.to_hex(),
        "target": target,
        "converged_dimensions": converged,
        "skipped_conflicting": skipped,
    }))
}

fn execute_server_cascade(
    repo: &Repository,
    repo_dir: &Path,
    default_branch: &str,
    source: &str,
    actor_name: &str,
    actor_email: &str,
) -> Result<serde_json::Value, String> {
    let cas = repo.cas().as_ref();
    let graph = StoreCommitGraph::new(cas);

    let src_oid =
        resolve_any_ref(repo, source).ok_or_else(|| format!("Source '{}' not found", source))?;
    let src_raw = cas.read_raw(&src_oid).map_err(|e| e.to_string())?;
    let src_commit = Commit::deserialize(&src_raw.data).map_err(|e| e.to_string())?;

    let mut propagated = Vec::new();
    let mut blocked = Vec::new();

    for d in list_dimension_items(repo) {
        let Some(dname) = d["name"].as_str() else {
            continue;
        };
        if dname == source {
            continue;
        }
        let Some(d_oid) = resolve_any_ref(repo, dname) else {
            continue;
        };
        if d_oid == src_oid || is_ancestor(&graph, &src_oid, &d_oid).unwrap_or(false) {
            continue;
        }
        let Ok(d_raw) = cas.read_raw(&d_oid) else {
            continue;
        };
        let Ok(d_commit) = Commit::deserialize(&d_raw.data) else {
            continue;
        };
        let lca_tree = all_merge_bases(&graph, &d_oid, &src_oid)
            .ok()
            .and_then(|v| v.into_iter().next())
            .and_then(|id| cas.read_raw(&id).ok())
            .and_then(|r| Commit::deserialize(&r.data).ok())
            .map(|c| c.tree);

        let mres = merge_trees_3way(
            cas,
            lca_tree.as_ref(),
            &d_commit.tree,
            &src_commit.tree,
            dname,
            source,
        )
        .map_err(|e| e.to_string())?;

        if mres.has_conflicts() {
            blocked.push(serde_json::json!({
                "dimension": dname,
                "conflicting_files": mres.conflicted_files.keys().cloned().collect::<Vec<_>>(),
            }));
        } else {
            let new_tree = mres.write_clean_tree(cas).map_err(|e| e.to_string())?;
            let sig = Signature::now(actor_name, actor_email);
            let c = Commit::new(
                new_tree,
                vec![d_oid, src_oid],
                sig.clone(),
                sig,
                format!("[cascade] Propagate '{}' into '{}'", source, dname),
            );
            let raw = RawObject::new(ObjectType::Commit, c.serialize());
            let c_oid = cas.write_raw(&raw).map_err(|e| e.to_string())?;
            write_dimension_or_head_ref(repo_dir, default_branch, dname, &c_oid);
            propagated.push(dname.to_string());
        }
    }

    Ok(serde_json::json!({
        "ok": true,
        "source": source,
        "propagated_dimensions": propagated,
        "blocked": blocked,
    }))
}

fn execute_server_splice(
    repo: &Repository,
    repo_dir: &Path,
    default_branch: &str,
    commit_rev: &str,
    into_dim: &str,
    actor_name: &str,
    actor_email: &str,
) -> Result<serde_json::Value, String> {
    let cas = repo.cas().as_ref();
    let donor_oid = resolve_any_ref(repo, commit_rev)
        .ok_or_else(|| format!("Donor commit/ref '{}' not found", commit_rev))?;
    let into_oid = resolve_any_ref(repo, into_dim)
        .ok_or_else(|| format!("Target dimension '{}' not found", into_dim))?;

    let donor_raw = cas.read_raw(&donor_oid).map_err(|e| e.to_string())?;
    let donor_commit = Commit::deserialize(&donor_raw.data).map_err(|e| e.to_string())?;
    let parent_tree = donor_commit
        .parents
        .first()
        .and_then(|p| cas.read_raw(p).ok())
        .and_then(|r| Commit::deserialize(&r.data).ok())
        .map(|c| c.tree);

    let into_raw = cas.read_raw(&into_oid).map_err(|e| e.to_string())?;
    let into_commit = Commit::deserialize(&into_raw.data).map_err(|e| e.to_string())?;

    let mres = merge_trees_3way(
        cas,
        parent_tree.as_ref(),
        &into_commit.tree,
        &donor_commit.tree,
        into_dim,
        commit_rev,
    )
    .map_err(|e| e.to_string())?;

    if mres.has_conflicts() {
        return Err(format!(
            "Splice conflicts in: {}",
            mres.conflicted_files
                .keys()
                .cloned()
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }

    let new_tree = mres.write_clean_tree(cas).map_err(|e| e.to_string())?;
    let sig = Signature::now(actor_name, actor_email);
    let summary = donor_commit.message.lines().next().unwrap_or("commit");
    let c = Commit::new(
        new_tree,
        vec![into_oid],
        sig.clone(),
        sig,
        format!(
            "[splice] {} (transplanted {} into {})",
            summary,
            &donor_oid.to_hex()[..8],
            into_dim
        ),
    );
    let raw = RawObject::new(ObjectType::Commit, c.serialize());
    let c_oid = cas.write_raw(&raw).map_err(|e| e.to_string())?;
    write_dimension_or_head_ref(repo_dir, default_branch, into_dim, &c_oid);

    Ok(serde_json::json!({
        "ok": true,
        "commit_id": c_oid.to_hex(),
        "into": into_dim,
    }))
}

fn compute_comparison(repo: &Repository, base_ref: &str, head_ref: &str) -> serde_json::Value {
    let Some(base_oid) = resolve_any_ref(repo, base_ref) else {
        return serde_json::json!({ "error": "Base reference not found" });
    };
    let Some(head_oid) = resolve_any_ref(repo, head_ref) else {
        return serde_json::json!({ "error": "Head reference not found" });
    };

    let cas = repo.cas().as_ref();
    let graph = StoreCommitGraph::new(cas);
    let can_ff = is_ancestor(&graph, &base_oid, &head_oid).unwrap_or(false);
    let foresee = compute_foresee_badge(repo, base_ref, head_ref);

    let base_tree = cas
        .read_raw(&base_oid)
        .ok()
        .and_then(|r| Commit::deserialize(&r.data).ok())
        .map(|c| c.tree);
    let head_tree = cas
        .read_raw(&head_oid)
        .ok()
        .and_then(|r| Commit::deserialize(&r.data).ok())
        .map(|c| c.tree);

    let patches = diff_trees(cas, base_tree.as_ref(), head_tree.as_ref()).unwrap_or_default();
    let diff_text = format_unified_diff(&patches);
    let (files_changed, additions, deletions) = summarize_patches(&patches);

    // Commits in head not in base
    let base_ancestors: HashSet<ObjectId> = walk_commits(&graph, &[base_oid])
        .unwrap_or_default()
        .into_iter()
        .collect();
    let ahead_commits: Vec<serde_json::Value> = walk_commits(&graph, &[head_oid])
        .unwrap_or_default()
        .into_iter()
        .filter(|oid| !base_ancestors.contains(oid))
        .take(50)
        .filter_map(|oid| read_commit_json(repo, &oid))
        .collect();

    serde_json::json!({
        "base_ref": base_ref,
        "head_ref": head_ref,
        "base_commit": base_oid.to_hex(),
        "head_commit": head_oid.to_hex(),
        "identical": base_oid == head_oid,
        "can_fast_forward": can_ff,
        "foresee": foresee,
        "commits": ahead_commits,
        "stats": {
            "files_changed": files_changed,
            "additions": additions,
            "deletions": deletions,
        },
        "diff": diff_text,
    })
}

fn estimate_divergence_entropy(
    cas: &daft_core::cas::ObjectStore,
    tree_a: &ObjectId,
    tree_b: &ObjectId,
    conflict_count: usize,
) -> f64 {
    if tree_a == tree_b {
        return 0.0;
    }
    let mut fa = BTreeMap::new();
    let mut fb = BTreeMap::new();
    let _ = flatten_tree(cas, tree_a, "", &mut fa);
    let _ = flatten_tree(cas, tree_b, "", &mut fb);
    let mut union_paths: HashSet<&String> = HashSet::new();
    for k in fa.keys() {
        union_paths.insert(k);
    }
    for k in fb.keys() {
        union_paths.insert(k);
    }
    if union_paths.is_empty() {
        return 0.0;
    }
    let mut diff_files = 0usize;
    for p in &union_paths {
        if fa.get(*p).map(|x| x.1) != fb.get(*p).map(|x| x.1) {
            diff_files += 1;
        }
    }
    let ratio = diff_files as f64 / union_paths.len() as f64;
    let conf_boost = (conflict_count as f64 * 0.2).min(0.5);
    ((ratio * 0.65) + conf_boost).min(1.0)
}

fn compute_radar_matrix(repo: &Repository, default_branch: &str) -> Vec<serde_json::Value> {
    let mut refs: Vec<(String, String)> = Vec::new();
    for b in list_ref_items(repo, "refs/heads", Some(default_branch)) {
        if let Some(n) = b["name"].as_str() {
            refs.push((n.to_string(), "branch".to_string()));
        }
    }
    for d in list_dimension_items(repo) {
        if let Some(n) = d["name"].as_str() {
            if !refs.iter().any(|(x, _)| x == n) {
                refs.push((n.to_string(), "dimension".to_string()));
            }
        }
    }

    let mut out = Vec::new();
    for (name, kind) in &refs {
        if name == default_branch {
            continue;
        }
        let foresee = compute_foresee_badge(repo, default_branch, name);
        out.push(serde_json::json!({
            "source": name,
            "kind": kind,
            "target": default_branch,
            "foresee": foresee,
        }));
    }
    out
}

fn build_server_timeline_graph(
    repo: &Arc<Repository>,
    repo_label: &str,
    default_branch: &str,
    filter_dim: Option<&str>,
) -> MultiverseTimelineGraph {
    let mut lanes: Vec<(String, ObjectId)> = Vec::new();
    if let Some(def_oid) = resolve_any_ref(repo, default_branch) {
        lanes.push((default_branch.to_string(), def_oid));
    }
    for (ref_name, hex_id) in collect_all_repo_refs(repo) {
        let short = ref_name
            .strip_prefix("refs/heads/")
            .or_else(|| ref_name.strip_prefix("refs/dimensions/"))
            .or_else(|| ref_name.strip_prefix("refs/tags/"))
            .unwrap_or(&ref_name)
            .to_string();
        if ref_name.starts_with("refs/tags/") {
            continue;
        }
        if let Some(f) = filter_dim {
            if short != f && short != default_branch {
                continue;
            }
        }
        if !lanes.iter().any(|(n, _)| n == &short) {
            if let Ok(oid) = ObjectId::from_hex(&hex_id) {
                lanes.push((short, oid));
            }
        }
    }

    let store_graph = StoreCommitGraph::new(repo.cas().as_ref());
    let root_head = lanes.first().map(|(_, o)| *o);

    let mut dimensions = Vec::new();
    let mut head_oids = Vec::new();
    let mut head_map: HashMap<ObjectId, Vec<String>> = HashMap::new();

    for (idx, (name, oid)) in lanes.iter().enumerate() {
        let fork_base = if idx > 0 {
            root_head.and_then(|rh| merge_base(&store_graph, &rh, oid).ok().flatten())
        } else {
            None
        };
        dimensions.push(TimelineDimension {
            name: name.clone(),
            parent: if idx > 0 {
                Some(default_branch.to_string())
            } else {
                None
            },
            creator: "drafthub".to_string(),
            created_at: String::new(),
            fork_base_commit: fork_base.map(|o| o.to_hex()),
            head_commit: Some(oid.to_hex()),
            status: "clean".to_string(),
            is_active: idx == 0,
            lane: idx,
            dirty_files_count: 0,
        });
        if !head_oids.contains(oid) {
            head_oids.push(*oid);
        }
        head_map.entry(*oid).or_default().push(name.clone());
    }

    let mut nodes = Vec::new();
    let mut edges = Vec::new();

    if !head_oids.is_empty() {
        if let Ok(ordered) = walk_commits(&store_graph, &head_oids) {
            let mut child_map: HashMap<ObjectId, Vec<ObjectId>> = HashMap::new();
            let mut commits_cache: HashMap<ObjectId, Commit> = HashMap::new();
            for oid in &ordered {
                if let Ok(raw) = repo.cas().read_raw(oid) {
                    if let Ok(c) = Commit::deserialize(&raw.data) {
                        for p in &c.parents {
                            child_map.entry(*p).or_default().push(*oid);
                        }
                        commits_cache.insert(*oid, c);
                    }
                }
            }

            for oid in &ordered {
                if let Some(c) = commits_cache.get(oid) {
                    let id_hex = oid.to_hex();
                    let short_id = id_hex[..id_hex.len().min(7)].to_string();
                    let heads_here = head_map.get(oid).cloned().unwrap_or_default();
                    let forked_here: Vec<String> = dimensions
                        .iter()
                        .filter(|d| d.fork_base_commit.as_deref() == Some(&id_hex))
                        .map(|d| d.name.clone())
                        .collect();

                    let node_type = if c.parents.is_empty() {
                        TimelineNodeType::Root
                    } else if !forked_here.is_empty() {
                        TimelineNodeType::ForkPoint
                    } else if c.parents.len() > 1 {
                        TimelineNodeType::Merge
                    } else if c.message.contains("[converge]") || c.message.contains("[collapse]") {
                        TimelineNodeType::Convergence
                    } else {
                        TimelineNodeType::Commit
                    };

                    let lane = heads_here
                        .first()
                        .and_then(|dn| dimensions.iter().find(|d| &d.name == dn).map(|d| d.lane))
                        .unwrap_or(0);

                    let parent_hexes: Vec<String> = c.parents.iter().map(|p| p.to_hex()).collect();
                    let child_hexes: Vec<String> = child_map
                        .get(oid)
                        .map(|ch| ch.iter().map(|x| x.to_hex()).collect())
                        .unwrap_or_default();

                    for p in &parent_hexes {
                        edges.push(TimelineEdge {
                            source: id_hex.clone(),
                            target: p.clone(),
                            edge_type: TimelineEdgeType::CommitParent,
                            label: None,
                        });
                    }

                    let dt = chrono::DateTime::<Utc>::from_timestamp(c.author.time, 0)
                        .unwrap_or_else(Utc::now)
                        .to_rfc3339();
                    let summary = c.message.lines().next().unwrap_or("").trim().to_string();

                    nodes.push(TimelineNode {
                        id: id_hex,
                        short_id,
                        node_type,
                        author: c.author.name.clone(),
                        timestamp: c.author.time,
                        datetime: dt,
                        summary,
                        parent_ids: parent_hexes,
                        child_ids: child_hexes,
                        reachable_dimensions: heads_here.clone(),
                        head_of_dimensions: heads_here,
                        forked_dimensions: forked_here,
                        lane,
                    });
                }
            }
        }
    }

    MultiverseTimelineGraph {
        version: "1.0".to_string(),
        repository: repo_label.to_string(),
        current_dimension: default_branch.to_string(),
        dimensions,
        nodes,
        edges,
    }
}

// ============================================================================
// Persistence & Utility Helpers
// ============================================================================

fn load_repo_meta(repo_dir: &Path, owner: &str, name: &str) -> RepoMeta {
    if let Some(m) = read_json_file::<RepoMeta>(&repo_dir.join("repo_meta.json")) {
        return m;
    }
    let head_branch = fs::read_to_string(repo_dir.join("HEAD"))
        .ok()
        .and_then(|s| s.trim().strip_prefix("ref: refs/heads/").map(String::from))
        .unwrap_or_else(|| "main".to_string());
    let now = Utc::now().to_rfc3339();
    RepoMeta {
        owner: owner.to_string(),
        name: name.to_string(),
        description: String::new(),
        private: false,
        default_branch: head_branch,
        topics: Vec::new(),
        stars: Vec::new(),
        forks_count: 0,
        forked_from: None,
        migrated_from: None,
        created_at: now.clone(),
        updated_at: now,
    }
}

fn save_repo_meta(repo_dir: &Path, meta: &RepoMeta) -> Result<(), CliError> {
    write_json_atomic(&repo_dir.join("repo_meta.json"), meta)
}

fn list_all_repo_metas(state: &ServerState, user: Option<&UserRecord>) -> Vec<RepoMeta> {
    let mut out = Vec::new();
    let root = state.repos_root();
    let Ok(owners) = fs::read_dir(root) else {
        return out;
    };
    for o_entry in owners.flatten() {
        if !o_entry.path().is_dir() {
            continue;
        }
        let owner_name = o_entry.file_name().to_string_lossy().to_string();
        let Ok(repos) = fs::read_dir(o_entry.path()) else {
            continue;
        };
        for r_entry in repos.flatten() {
            let p = r_entry.path();
            if p.is_dir() && p.join("HEAD").exists() {
                let raw_name = r_entry.file_name().to_string_lossy().to_string();
                let clean_name = raw_name.trim_end_matches(".dft").to_string();
                let meta = load_repo_meta(&p, &owner_name, &clean_name);
                if !meta.private || can_access_private(&meta, user, state) {
                    out.push(meta);
                }
            }
        }
    }
    out
}

fn summarize_repo_card(state: &ServerState, m: &RepoMeta, base_url: &str) -> serde_json::Value {
    let rdir = state.repo_dir(&m.owner, &m.name);
    let mut branches_cnt = 0usize;
    let mut dims_cnt = 0usize;
    let mut commits_cnt = 0usize;

    if let Ok(repo) = Repository::open(&rdir) {
        branches_cnt = repo
            .refs()
            .list_refs("refs/heads")
            .map(|v| v.len())
            .unwrap_or(0);
        dims_cnt = list_dimension_items(&repo).len();
        if let Some(oid) = resolve_any_ref(&repo, &m.default_branch) {
            let g = StoreCommitGraph::new(repo.cas().as_ref());
            commits_cnt = walk_commits(&g, &[oid]).map(|v| v.len()).unwrap_or(0);
        }
    }
    let issues: Vec<IssueRecord> = read_json_file(&rdir.join("issues.json")).unwrap_or_default();
    let pulls: Vec<PullRecord> = read_json_file(&rdir.join("pulls.json")).unwrap_or_default();

    serde_json::json!({
        "owner": m.owner,
        "name": m.name,
        "full_name": format!("{}/{}", m.owner, m.name),
        "description": m.description,
        "private": m.private,
        "default_branch": m.default_branch,
        "topics": m.topics,
        "stars_count": m.stars.len(),
        "forks_count": m.forks_count,
        "forked_from": m.forked_from,
        "migrated_from": m.migrated_from,
        "branches_count": branches_cnt,
        "dimensions_count": dims_cnt,
        "commits_count": commits_cnt,
        "open_issues_count": issues.iter().filter(|i| i.state == "open").count(),
        "open_pulls_count": pulls.iter().filter(|p| p.state == "open").count(),
        "clone_url": format!("{}/{}/{}.dft", base_url, m.owner, m.name),
        "created_at": m.created_at,
        "updated_at": m.updated_at,
    })
}

fn can_access_private(meta: &RepoMeta, user: Option<&UserRecord>, state: &ServerState) -> bool {
    let Some(u) = user else {
        return false;
    };
    if u.is_admin || u.username.eq_ignore_ascii_case(&meta.owner) {
        return true;
    }
    if let Ok(orgs) = state.orgs.read() {
        if let Some(org) = orgs
            .iter()
            .find(|o| o.name.eq_ignore_ascii_case(&meta.owner))
        {
            if org
                .members
                .iter()
                .any(|m| m.eq_ignore_ascii_case(&u.username))
            {
                return true;
            }
        }
    }
    false
}

fn load_territory_json(repo_dir: &Path) -> serde_json::Value {
    let claims: serde_json::Value = read_json_file(&repo_dir.join("territory/claims.json"))
        .unwrap_or_else(|| serde_json::json!({}));
    let fences: serde_json::Value = read_json_file(&repo_dir.join("territory/fences.json"))
        .unwrap_or_else(|| serde_json::json!([]));
    serde_json::json!({
        "claims": claims,
        "fences": fences,
    })
}

fn load_agents_json(repo_dir: &Path) -> Vec<serde_json::Value> {
    let mut out = Vec::new();
    let dir = repo_dir.join("agents");
    if dir.is_dir() {
        if let Ok(entries) = fs::read_dir(dir) {
            for e in entries.flatten() {
                if e.path().extension().is_some_and(|x| x == "json") {
                    if let Some(v) = read_json_file::<serde_json::Value>(&e.path()) {
                        out.push(v);
                    }
                }
            }
        }
    }
    out
}

fn create_session(state: &ServerState, username: &str) -> String {
    let tok = generate_token_hex(24);
    let now = Utc::now().timestamp();
    let sess = SessionRecord {
        token: tok.clone(),
        username: username.to_string(),
        created_at: now,
        expires_at: now + 30 * 86_400,
    };
    if let Ok(mut sessions) = state.sessions.write() {
        sessions.retain(|s| s.expires_at > now);
        sessions.push(sess);
        let _ = write_json_atomic(&state.data_dir.join("sessions.json"), &*sessions);
    }
    tok
}

fn sanitize_slug(raw: &str) -> String {
    let s: String = raw
        .trim()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.' {
                c
            } else {
                '-'
            }
        })
        .collect();
    s.trim_matches('.').trim_matches('-').to_string()
}

fn sha256_hex(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hex::encode(hasher.finalize())
}

fn hash_password(password: &str, salt: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(salt.as_bytes());
    hasher.update(b":draft-server-v1:");
    hasher.update(password.as_bytes());
    hex::encode(hasher.finalize())
}

fn generate_token_hex(byte_len: usize) -> String {
    let mut seed = Vec::new();
    seed.extend_from_slice(&Utc::now().timestamp_nanos_opt().unwrap_or(0).to_le_bytes());
    seed.extend_from_slice(&std::process::id().to_le_bytes());
    if let Ok(mut f) = fs::File::open("/dev/urandom") {
        use std::io::Read;
        let mut rnd = vec![0u8; byte_len];
        if f.read_exact(&mut rnd).is_ok() {
            return hex::encode(rnd);
        }
    }
    let digest = sha256_hex(&seed);
    digest[..digest.len().min(byte_len * 2)].to_string()
}

fn read_json_file<T: DeserializeOwned>(path: &Path) -> Option<T> {
    let s = fs::read_to_string(path).ok()?;
    serde_json::from_str(&s).ok()
}

fn write_json_atomic<T: Serialize + ?Sized>(path: &Path, val: &T) -> Result<(), CliError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension(format!("tmp.{}", std::process::id()));
    let bytes = serde_json::to_vec_pretty(val)
        .map_err(|e| CliError::General(format!("JSON serialize error: {}", e)))?;
    fs::write(&tmp, bytes)?;
    fs::rename(&tmp, path)?;
    Ok(())
}

fn compute_dir_size(path: &Path) -> u64 {
    let mut total = 0u64;
    for entry in walkdir::WalkDir::new(path).into_iter().flatten() {
        if entry.file_type().is_file() {
            total += entry.metadata().map(|m| m.len()).unwrap_or(0);
        }
    }
    total
}

fn guess_mime(path: &str) -> &'static str {
    let lower = path.to_ascii_lowercase();
    if lower.ends_with(".svg") {
        "image/svg+xml"
    } else if lower.ends_with(".png") {
        "image/png"
    } else if lower.ends_with(".jpg") || lower.ends_with(".jpeg") {
        "image/jpeg"
    } else if lower.ends_with(".webp") {
        "image/webp"
    } else if lower.ends_with(".gif") {
        "image/gif"
    } else if lower.ends_with(".json") {
        "application/json"
    } else if lower.ends_with(".html") {
        "text/html; charset=utf-8"
    } else {
        "text/plain; charset=utf-8"
    }
}

fn render_dynamic_install_script(base_url: &str) -> String {
    format!(
        r#"#!/bin/sh
set -e
SERVER_URL="{base}"
OS="$(uname -s)"
ARCH="$(uname -m)"
INSTALL_DIR="${{DFT_INSTALL_DIR:-$HOME/.local/bin}}"
mkdir -p "$INSTALL_DIR"

echo "Installing Draft (dft) CLI from $SERVER_URL ..."
if [ "$OS" = "Linux" ] && [ "$ARCH" = "x86_64" ]; then
  curl -fsSL "$SERVER_URL/download/dft" -o "$INSTALL_DIR/dft"
  chmod +x "$INSTALL_DIR/dft"
  ln -sf "$INSTALL_DIR/dft" "$INSTALL_DIR/draft"
  echo "Installed dft and draft into $INSTALL_DIR"
else
  echo "Platform $OS/$ARCH differs from server binary; installing via cargo..."
  cargo install --git https://github.com/Pathomphong-i/draft.git daft-cli
fi
"#,
        base = base_url
    )
}

fn render_server_ui_html(state: &ServerState, req: &HttpRequest) -> String {
    let base_url = state.base_url_for(req);
    let template = include_str!("server_ui.html");
    template
        .replace("__VERSION__", env!("CARGO_PKG_VERSION"))
        .replace("__SERVER_URL__", &base_url)
}

fn send_json<T: Serialize>(
    stream: &mut TcpStream,
    status: u16,
    value: &T,
    set_cookie: Option<&str>,
) {
    let body = serde_json::to_vec(value).unwrap_or_else(|_| b"{}".to_vec());
    send_response(stream, status, "application/json", &body, set_cookie);
}

fn send_json_error(stream: &mut TcpStream, status: u16, msg: &str) {
    let body = serde_json::json!({ "error": msg });
    send_json(stream, status, &body, None);
}

fn send_response(
    stream: &mut TcpStream,
    status: u16,
    content_type: &str,
    body: &[u8],
    set_cookie: Option<&str>,
) {
    let reason = match status {
        200 => "OK",
        201 => "Created",
        204 => "No Content",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        409 => "Conflict",
        _ => "Internal Server Error",
    };
    let cookie_hdr = set_cookie
        .map(|c| format!("Set-Cookie: {}\r\n", c))
        .unwrap_or_default();
    let header = format!(
        "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nAccess-Control-Allow-Origin: *\r\nAccess-Control-Allow-Headers: Content-Type, Authorization\r\nAccess-Control-Allow-Methods: GET, POST, PATCH, DELETE, OPTIONS\r\n{}Connection: close\r\n\r\n",
        status,
        reason,
        content_type,
        body.len(),
        cookie_hdr
    );
    let _ = stream.write_all(header.as_bytes());
    if !body.is_empty() {
        let _ = stream.write_all(body);
    }
    let _ = stream.flush();
}
