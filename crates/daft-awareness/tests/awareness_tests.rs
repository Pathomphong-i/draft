use daft_awareness::*;
use daft_core::init::{init, InitOptions};
use daft_dimension::DimensionManager;
use std::fs;
use std::path::Path;
use std::sync::Arc;
use tempfile::tempdir;

#[test]
fn test_observe_subsystem_fallback() {
    let dir = tempdir().unwrap();
    let repo = init(dir.path(), &InitOptions::default()).unwrap();
    let repo = Arc::new(repo);

    let observe = ObserveSubsystem::new(Arc::clone(&repo));

    // Initially file does not exist
    assert!(observe
        .cat_file("mainline", Path::new("hello.txt"))
        .is_err());

    // Write file to workspace
    fs::write(dir.path().join("hello.txt"), "workspace version").unwrap();
    let content = observe
        .cat_file("mainline", Path::new("hello.txt"))
        .unwrap();
    assert_eq!(String::from_utf8_lossy(&content), "workspace version");
}

#[test]
fn test_radar_subsystem_activity() {
    let dir = tempdir().unwrap();
    let repo = init(dir.path(), &InitOptions::default()).unwrap();
    let repo = Arc::new(repo);

    let dim_mgr = DimensionManager::new(Arc::clone(&repo));
    dim_mgr.init().unwrap();
    dim_mgr.create_dimension("dim-a", None, None).unwrap();

    let radar = RadarSubsystem::new(Arc::clone(&repo));
    let report = radar.scan(3).unwrap();
    assert!(report.scanned_dimensions.contains(&"dim-a".to_string()));
}

#[test]
fn test_entropy_metric_bounds() {
    let dir = tempdir().unwrap();
    let repo = init(dir.path(), &InitOptions::default()).unwrap();
    let repo = Arc::new(repo);

    let dim_mgr = DimensionManager::new(Arc::clone(&repo));
    dim_mgr.init().unwrap();
    dim_mgr.create_dimension("dim-1", None, None).unwrap();
    dim_mgr.create_dimension("dim-2", None, None).unwrap();

    let entropy_sys = EntropySubsystem::new(Arc::clone(&repo));
    let metrics = entropy_sys.calculate("dim-1", "dim-2").unwrap();

    assert!(metrics.total_entropy >= 0.0 && metrics.total_entropy <= 1.0);
    assert!(metrics.hunk_collision_score >= 0.0 && metrics.hunk_collision_score <= 1.0);
    assert!(metrics.file_overlap_score >= 0.0 && metrics.file_overlap_score <= 1.0);
}

#[test]
fn test_foresee_clean_simulation() {
    let dir = tempdir().unwrap();
    let repo = init(dir.path(), &InitOptions::default()).unwrap();
    let repo = Arc::new(repo);

    let dim_mgr = DimensionManager::new(Arc::clone(&repo));
    dim_mgr.init().unwrap();
    dim_mgr.create_dimension("feat-1", None, None).unwrap();
    dim_mgr.create_dimension("feat-2", None, None).unwrap();

    let foresee = ForeseeSubsystem::new(Arc::clone(&repo));
    let report = foresee.predict("feat-1", "feat-2").unwrap();
    assert!(!report.has_conflicts);
    assert!(report.conflict_hunks.is_empty());
}

#[test]
fn test_territory_manager_claims_and_fences() {
    let dir = tempdir().unwrap();
    let dft_dir = dir.path().join(".dft");
    fs::create_dir_all(&dft_dir).unwrap();

    let tm = TerritoryManager::new(&dft_dir);

    // Test claims
    let claim = tm
        .claim("src/**/*.rs", "dim-alpha", Some("agent-1"), Some(60), true)
        .unwrap();
    assert_eq!(claim.path_glob, "src/**/*.rs");
    assert_eq!(claim.dimension, "dim-alpha");

    // Conflicting claim fails
    let conflict = tm.claim("src/main.rs", "dim-beta", Some("agent-2"), Some(60), true);
    assert!(conflict.is_err());

    // Yield claim
    let released = tm
        .yield_path("src/**/*.rs", Some("agent-1"), Some("dim-alpha"))
        .unwrap();
    assert_eq!(released.len(), 1);

    // Create fence
    let fence = tm
        .fence(
            "crates/core/**",
            Some("security-team"),
            Some("agent-sec"),
            true,
            Some(3600),
            Some("L0 write lock"),
        )
        .unwrap();
    assert_eq!(fence.path_glob, "crates/core/**");
    assert!(fence.hard);

    // List fences
    let active_fences = tm.list_fences().unwrap();
    assert_eq!(active_fences.len(), 1);

    // Unfence
    let removed = tm.unfence("crates/core/**").unwrap();
    assert!(removed.is_some());
    assert_eq!(tm.list_fences().unwrap().len(), 0);
}

#[test]
fn test_territory_ttl_parsing() {
    assert_eq!(parse_ttl_string("30s"), Ok(30));
    assert_eq!(parse_ttl_string("5m"), Ok(300));
    assert_eq!(parse_ttl_string("2h"), Ok(7200));
    assert_eq!(parse_ttl_string("1d"), Ok(86400));
    assert!(parse_ttl_string("invalid").is_err());
}
