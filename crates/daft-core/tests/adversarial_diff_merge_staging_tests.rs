//! Empirical Adversarial Test Suite for Daft VCS Milestone 2:
//! Diff Engine, 3-Way Recursive Merge Engine, and Staging / Worktree Operations.

use daft_core::cas::{ObjectStore, ObjectType, RawObject};
use daft_core::diff::binary::is_binary;
use daft_core::diff::myers::{myers_diff, EditOpKind};
use daft_core::diff::unified::{create_hunks, format_unified_diff, generate_file_patch};
use daft_core::graph::{all_merge_bases, merge_base, StoreCommitGraph};
use daft_core::index::Stage;
use daft_core::init::{init, InitOptions};
use daft_core::merge::three_way::merge_text_3way;
use daft_core::merge::tree_merge::{build_hierarchical_tree, merge_trees_3way};
use daft_core::merge::{merge_commits, MergeOutcome};
use daft_core::object::{Commit, FileMode, Signature, Tree, TreeEntry};
use daft_core::refs::ReferenceTarget;
use daft_core::worktree::add::add_paths;
use daft_core::worktree::status::get_status;
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::thread;
use tempfile::tempdir;

// =========================================================================
// SECTION 1: DIFF ENGINE ADVERSARIAL CHALLENGES
// =========================================================================

#[test]
fn test_diff_empty_files_matrix() {
    // 1. Both files empty
    let patch_empty =
        generate_file_patch(Some("file.txt"), Some("file.txt"), Some(b""), Some(b""), 3);
    assert!(
        patch_empty.is_empty(),
        "Empty vs empty must produce empty patch"
    );
    let formatted_empty = format_unified_diff(&[patch_empty]);
    assert!(
        formatted_empty.is_empty(),
        "Formatted diff must be empty for identical empty files"
    );

    // 2. Old empty, new non-empty (pure addition)
    let patch_add = generate_file_patch(
        None,
        Some("new.txt"),
        None,
        Some(b"first line\nsecond line\n"),
        3,
    );
    assert!(!patch_add.is_empty());
    assert_eq!(patch_add.hunks.len(), 1);
    let hunk = &patch_add.hunks[0];
    assert_eq!(hunk.old_start, 1);
    assert_eq!(hunk.new_start, 1);
    assert_eq!(hunk.new_count, 2);
    let formatted_add = format_unified_diff(&[patch_add]);
    assert!(formatted_add.contains("--- /dev/null"));
    assert!(formatted_add.contains("+++ b/new.txt"));
    assert!(formatted_add.contains("+first line"));
    assert!(formatted_add.contains("+second line"));

    // 3. Old non-empty, new empty (pure deletion)
    let patch_del = generate_file_patch(
        Some("old.txt"),
        None,
        Some(b"deleted line 1\ndeleted line 2\n"),
        None,
        3,
    );
    assert!(!patch_del.is_empty());
    assert_eq!(patch_del.hunks.len(), 1);
    let hunk_del = &patch_del.hunks[0];
    assert_eq!(hunk_del.old_count, 2);
    assert_eq!(hunk_del.new_count, 0);
    let formatted_del = format_unified_diff(&[patch_del]);
    assert!(formatted_del.contains("--- a/old.txt"));
    assert!(formatted_del.contains("+++ /dev/null"));
    assert!(formatted_del.contains("-deleted line 1"));
    assert!(formatted_del.contains("-deleted line 2"));
}

#[test]
fn test_diff_massive_single_line() {
    // 250,000 character single line with difference in middle
    let mut s1 = "A".repeat(120_000);
    s1.push_str("MIDDLE_OLD");
    s1.push_str(&"B".repeat(120_000));
    s1.push('\n');

    let mut s2 = "A".repeat(120_000);
    s2.push_str("MIDDLE_NEW");
    s2.push_str(&"B".repeat(120_000));
    s2.push('\n');

    let ops = myers_diff(&s1, &s2);
    assert_eq!(ops.len(), 2);
    assert_eq!(ops[0].kind, EditOpKind::Delete);
    assert_eq!(ops[1].kind, EditOpKind::Insert);

    let hunks = create_hunks(&ops, 3);
    assert_eq!(hunks.len(), 1);
    assert_eq!(hunks[0].lines.len(), 2);
}

#[test]
fn test_diff_binary_detection_heuristics_and_formatting() {
    // Empty data is not binary
    assert!(!is_binary(b""));

    // Exactly 8000 ASCII bytes without null is not binary
    let text_8000 = vec![b'a'; 8000];
    assert!(!is_binary(&text_8000));

    // NUL byte at position 0 is binary
    let mut null_at_0 = vec![b'x'; 8000];
    null_at_0[0] = 0x00;
    assert!(is_binary(&null_at_0));

    // NUL byte at position 7999 is binary
    let mut null_at_7999 = vec![b'x'; 8000];
    null_at_7999[7999] = 0x00;
    assert!(is_binary(&null_at_7999));

    // NUL byte at position 8000 is outside 8000-byte window
    let mut null_at_8000 = vec![b'x'; 10000];
    null_at_8000[8000] = 0x00;
    assert!(!is_binary(&null_at_8000));

    // 100KB binary data with embedded nulls
    let mut large_bin = vec![0xAB; 100_000];
    large_bin[42] = 0x00;
    assert!(is_binary(&large_bin));

    // Binary diff formatting
    let patch_bin = generate_file_patch(
        Some("data.bin"),
        Some("data.bin"),
        Some(&null_at_0),
        Some(&null_at_7999),
        3,
    );
    assert!(patch_bin.is_binary);
    let diff_text = format_unified_diff(&[patch_bin]);
    assert_eq!(diff_text, "Binary files a/data.bin and b/data.bin differ\n");
    assert!(
        !diff_text.contains("@@"),
        "Binary diff must not have hunk headers"
    );
}

#[test]
fn test_diff_unicode_graphemes_and_newlines() {
    let old_text = "🦀 Rust VCS\nLine 2: 🚀 Rocket\nLine 3: 漢字\n";
    let new_text = "🦀 Rust VCS\nLine 2: 🛸 UFO\nLine 3: 漢字\nLine 4: 🌍 Earth\n";

    let ops = myers_diff(old_text, new_text);
    eprintln!("UNICODE OPS: {:?}", ops);
    assert_eq!(ops[0].kind, EditOpKind::Equal);
    assert_eq!(ops[0].text, "🦀 Rust VCS\n");
    assert_eq!(ops[1].kind, EditOpKind::Delete);
    assert_eq!(ops[1].text, "Line 2: 🚀 Rocket\n");
    assert_eq!(ops[2].kind, EditOpKind::Insert);
    assert_eq!(ops[2].text, "Line 2: 🛸 UFO\n");
    assert_eq!(ops[3].kind, EditOpKind::Equal);
    assert_eq!(ops[3].text, "Line 3: 漢字\n");
    assert_eq!(ops[4].kind, EditOpKind::Insert);
    assert_eq!(ops[4].text, "Line 4: 🌍 Earth\n");
}

#[test]
fn test_diff_inner_snake_dropped_lines() {
    let old = "header\nA\ncommon\nB\nfooter\n";
    let new = "header\nA_mod\ncommon\nB_mod\nfooter\n";
    let ops = myers_diff(old, new);
    eprintln!("INNER SNAKE OPS: {:?}", ops);
    let common_op = ops.iter().find(|op| op.text == "common\n");
    assert!(
        common_op.is_some(),
        "Inner common line 'common\\n' was dropped by Myers diff!"
    );
    assert_eq!(common_op.unwrap().kind, EditOpKind::Equal);
}

#[test]
fn test_diff_identical_large_file_performance() {
    let mut content = String::with_capacity(100_000);
    for i in 0..5_000 {
        content.push_str(&format!(
            "This is line number {} in a large source file\n",
            i
        ));
    }

    let start = std::time::Instant::now();
    let ops = myers_diff(&content, &content);
    let duration = start.elapsed();

    assert_eq!(ops.len(), 5000);
    assert!(ops.iter().all(|op| op.kind == EditOpKind::Equal));
    assert!(
        duration.as_millis() < 500,
        "Identical 5000 lines diff must complete under 500ms"
    );
}

// =========================================================================
// SECTION 2: 3-WAY MERGE ADVERSARIAL CHALLENGES
// =========================================================================

#[test]
fn test_merge_empty_base_scenarios() {
    // 1. Base empty, both add identical content -> Clean
    let res_ident = merge_text_3way(
        "",
        "shared added line\n",
        "shared added line\n",
        "HEAD",
        "feature",
    );
    assert!(!res_ident.has_conflicts);
    assert_eq!(res_ident.content, "shared added line\n");

    // 2. Base empty, ours adds, theirs stays empty -> Clean
    let res_ours = merge_text_3way("", "added by ours\n", "", "HEAD", "feature");
    assert!(!res_ours.has_conflicts);
    assert_eq!(res_ours.content, "added by ours\n");

    // 3. Base empty, ours stays empty, theirs adds -> Clean
    let res_theirs = merge_text_3way("", "", "added by theirs\n", "HEAD", "feature");
    assert!(!res_theirs.has_conflicts);
    assert_eq!(res_theirs.content, "added by theirs\n");

    // 4. Base empty, both add different content -> Conflict!
    let res_conf = merge_text_3way("", "left\n", "right\n", "HEAD", "feature");
    assert!(res_conf.has_conflicts);
    assert!(res_conf.content.contains("<<<<<<< HEAD\n"));
    assert!(res_conf.content.contains("left\n"));
    assert!(res_conf.content.contains("=======\n"));
    assert!(res_conf.content.contains("right\n"));
    assert!(res_conf.content.contains(">>>>>>> feature\n"));
}

#[test]
fn test_merge_conflict_marker_formatting_exact() {
    let base = "alpha\nbeta\ngamma\n";
    let ours = "alpha\nbeta_ours\ngamma\n";
    let theirs = "alpha\nbeta_theirs\ngamma\n";

    let res = merge_text_3way(base, ours, theirs, "HEAD", "theirs-branch");
    assert!(res.has_conflicts);

    // Verify EXACT 7-character conflict markers
    let expected =
        "alpha\n<<<<<<< HEAD\nbeta_ours\n=======\nbeta_theirs\n>>>>>>> theirs-branch\ngamma\n";
    assert_eq!(
        res.content, expected,
        "Conflict marker formatting must match standard Git format"
    );
}

#[test]
fn test_merge_adjacent_edits_clean() {
    // Non-overlapping edits on consecutive lines
    let base = "line 1\nline 2\nline 3\nline 4\nline 5\n";
    let ours = "line 1\nline 2 MODIFIED\nline 3\nline 4\nline 5\n";
    let theirs = "line 1\nline 2\nline 3\nline 4 MODIFIED\nline 5\n";

    let res = merge_text_3way(base, ours, theirs, "HEAD", "feature");
    assert!(
        !res.has_conflicts,
        "Adjacent edits on disjoint lines must cleanly merge"
    );
    let expected = "line 1\nline 2 MODIFIED\nline 3\nline 4 MODIFIED\nline 5\n";
    assert_eq!(res.content, expected);
}

#[test]
fn test_merge_multiple_conflicts_with_clean_islands() {
    let base = "island 1\nconf 1 base\nisland 2\nconf 2 base\nisland 3\n";
    let ours = "island 1\nconf 1 ours\nisland 2\nconf 2 ours\nisland 3\n";
    let theirs = "island 1\nconf 1 theirs\nisland 2\nconf 2 theirs\nisland 3\n";

    let ops_a = myers_diff(base, ours);
    eprintln!("OPS A: {:?}", ops_a);
    let res = merge_text_3way(base, ours, theirs, "HEAD", "branch");
    eprintln!("ACTUAL MERGE RESULT:\n{}", res.content);
    assert!(res.has_conflicts);

    // Both conflicts must be captured, with island 2 cleanly preserved between them
    assert!(res.content.contains(
        "island 1\n<<<<<<< HEAD\nconf 1 ours\n=======\nconf 1 theirs\n>>>>>>> branch\nisland 2\n"
    ));
    assert!(res.content.contains(
        "island 2\n<<<<<<< HEAD\nconf 2 ours\n=======\nconf 2 theirs\n>>>>>>> branch\nisland 3\n"
    ));
}

#[test]
fn test_merge_files_without_trailing_newlines() {
    let base = "line1\nline2";
    let ours = "line1\nline2 modified";
    let theirs = "line1\nline2";

    let res = merge_text_3way(base, ours, theirs, "HEAD", "theirs");
    assert!(!res.has_conflicts);
    assert!(res.content.contains("line2 modified"));
}

#[test]
fn test_merge_tree_binary_and_delete_conflicts() {
    let dir = tempdir().expect("tempdir");
    let cas = ObjectStore::init(dir.path().join("objects")).expect("cas init");

    // Base tree with a text file and a binary file
    let base_text_blob = cas
        .write_raw(&RawObject::new(ObjectType::Blob, b"base text\n".to_vec()))
        .expect("blob");
    let base_bin_blob = cas
        .write_raw(&RawObject::new(ObjectType::Blob, vec![0x00, 0x01, 0x02]))
        .expect("blob");
    let to_delete_blob = cas
        .write_raw(&RawObject::new(
            ObjectType::Blob,
            b"will be deleted\n".to_vec(),
        ))
        .expect("blob");

    let base_tree = Tree::from_entries(vec![
        TreeEntry::new(FileMode::REGULAR, "doc.txt", base_text_blob).unwrap(),
        TreeEntry::new(FileMode::REGULAR, "image.png", base_bin_blob).unwrap(),
        TreeEntry::new(FileMode::REGULAR, "deleted.txt", to_delete_blob).unwrap(),
    ])
    .unwrap();
    let base_tree_oid = cas
        .write_raw(&RawObject::new(ObjectType::Tree, base_tree.serialize()))
        .expect("tree");

    // Ours modifies doc.txt, modifies image.png, modifies deleted.txt
    let ours_text_blob = cas
        .write_raw(&RawObject::new(ObjectType::Blob, b"ours text\n".to_vec()))
        .expect("blob");
    let ours_bin_blob = cas
        .write_raw(&RawObject::new(ObjectType::Blob, vec![0x00, 0xAA, 0xBB]))
        .expect("blob");
    let ours_del_mod_blob = cas
        .write_raw(&RawObject::new(
            ObjectType::Blob,
            b"modified by ours\n".to_vec(),
        ))
        .expect("blob");

    let ours_tree = Tree::from_entries(vec![
        TreeEntry::new(FileMode::REGULAR, "doc.txt", ours_text_blob).unwrap(),
        TreeEntry::new(FileMode::REGULAR, "image.png", ours_bin_blob).unwrap(),
        TreeEntry::new(FileMode::REGULAR, "deleted.txt", ours_del_mod_blob).unwrap(),
    ])
    .unwrap();
    let ours_tree_oid = cas
        .write_raw(&RawObject::new(ObjectType::Tree, ours_tree.serialize()))
        .expect("tree");

    // Theirs modifies doc.txt differently, modifies image.png differently, deletes deleted.txt
    let theirs_text_blob = cas
        .write_raw(&RawObject::new(ObjectType::Blob, b"theirs text\n".to_vec()))
        .expect("blob");
    let theirs_bin_blob = cas
        .write_raw(&RawObject::new(ObjectType::Blob, vec![0x00, 0xCC, 0xDD]))
        .expect("blob");

    let theirs_tree = Tree::from_entries(vec![
        TreeEntry::new(FileMode::REGULAR, "doc.txt", theirs_text_blob).unwrap(),
        TreeEntry::new(FileMode::REGULAR, "image.png", theirs_bin_blob).unwrap(),
        // deleted.txt omitted
    ])
    .unwrap();
    let theirs_tree_oid = cas
        .write_raw(&RawObject::new(ObjectType::Tree, theirs_tree.serialize()))
        .expect("tree");

    let merge_res = merge_trees_3way(
        &cas,
        Some(&base_tree_oid),
        &ours_tree_oid,
        &theirs_tree_oid,
        "HEAD",
        "feature",
    )
    .unwrap();

    assert!(merge_res.has_conflicts());
    assert_eq!(merge_res.conflicted_files.len(), 3);

    // 1. Text conflict
    let doc_conf = &merge_res.conflicted_files["doc.txt"];
    assert!(doc_conf
        .marker_content
        .contains("<<<<<<< HEAD\nours text\n=======\ntheirs text\n>>>>>>> feature\n"));

    // 2. Binary conflict
    let bin_conf = &merge_res.conflicted_files["image.png"];
    assert!(bin_conf
        .marker_content
        .contains("<<<<<<< HEAD\n[Binary file]\n=======\n[Binary file]\n>>>>>>> feature\n"));

    // 3. Modify/Delete conflict
    let del_conf = &merge_res.conflicted_files["deleted.txt"];
    assert!(del_conf
        .marker_content
        .contains("<<<<<<< HEAD\nmodified by ours\n=======\n>>>>>>> feature\n"));
}

#[test]
fn test_merge_criss_cross_bases_and_commit_merge() {
    // Construct real criss-cross commit DAG:
    //       A
    //      / \
    //     B   C
    //     |\ /|
    //     | X |
    //     |/ \|
    //     D   E
    let dir = tempdir().expect("tempdir");
    let repo = init(dir.path(), &InitOptions::default()).expect("repo init");
    let cas = repo.cas();

    let sig = Signature::now("Daft Tester", "test@daft-vcs.org");

    // Empty initial tree
    let empty_tree = Tree::from_entries(vec![]).unwrap();
    let empty_tree_oid = cas
        .write_raw(&RawObject::new(ObjectType::Tree, empty_tree.serialize()))
        .unwrap();

    // Commit A
    let commit_a = Commit::new(empty_tree_oid, vec![], sig.clone(), sig.clone(), "A");
    let oid_a = cas
        .write_raw(&RawObject::new(ObjectType::Commit, commit_a.serialize()))
        .unwrap();

    // Tree for B with file_b.txt
    let blob_b = cas
        .write_raw(&RawObject::new(ObjectType::Blob, b"file B\n".to_vec()))
        .unwrap();
    let tree_b = Tree::from_entries(vec![TreeEntry::new(
        FileMode::REGULAR,
        "file_b.txt",
        blob_b,
    )
    .unwrap()])
    .unwrap();
    let tree_b_oid = cas
        .write_raw(&RawObject::new(ObjectType::Tree, tree_b.serialize()))
        .unwrap();

    // Tree for C with file_c.txt
    let blob_c = cas
        .write_raw(&RawObject::new(ObjectType::Blob, b"file C\n".to_vec()))
        .unwrap();
    let tree_c = Tree::from_entries(vec![TreeEntry::new(
        FileMode::REGULAR,
        "file_c.txt",
        blob_c,
    )
    .unwrap()])
    .unwrap();
    let tree_c_oid = cas
        .write_raw(&RawObject::new(ObjectType::Tree, tree_c.serialize()))
        .unwrap();

    // Tree for D & E with both file_b.txt and file_c.txt
    let tree_bc = Tree::from_entries(vec![
        TreeEntry::new(FileMode::REGULAR, "file_b.txt", blob_b).unwrap(),
        TreeEntry::new(FileMode::REGULAR, "file_c.txt", blob_c).unwrap(),
    ])
    .unwrap();
    let tree_bc_oid = cas
        .write_raw(&RawObject::new(ObjectType::Tree, tree_bc.serialize()))
        .unwrap();

    // Commit B (parent A)
    let commit_b = Commit::new(tree_b_oid, vec![oid_a], sig.clone(), sig.clone(), "B");
    let oid_b = cas
        .write_raw(&RawObject::new(ObjectType::Commit, commit_b.serialize()))
        .unwrap();

    // Commit C (parent A)
    let commit_c = Commit::new(tree_c_oid, vec![oid_a], sig.clone(), sig.clone(), "C");
    let oid_c = cas
        .write_raw(&RawObject::new(ObjectType::Commit, commit_c.serialize()))
        .unwrap();

    // Commit D (parents B, C)
    let commit_d = Commit::new(
        tree_bc_oid,
        vec![oid_b, oid_c],
        sig.clone(),
        sig.clone(),
        "D",
    );
    let oid_d = cas
        .write_raw(&RawObject::new(ObjectType::Commit, commit_d.serialize()))
        .unwrap();

    // Commit E (parents C, B)
    let commit_e = Commit::new(
        tree_bc_oid,
        vec![oid_c, oid_b],
        sig.clone(),
        sig.clone(),
        "E",
    );
    let oid_e = cas
        .write_raw(&RawObject::new(ObjectType::Commit, commit_e.serialize()))
        .unwrap();

    let graph = StoreCommitGraph::new(cas.as_ref());
    let bases = all_merge_bases(&graph, &oid_d, &oid_e).unwrap();

    // Criss-cross has 2 common ancestors (B and C)
    assert_eq!(
        bases.len(),
        2,
        "Criss-cross must identify exactly 2 lowest common ancestors"
    );
    assert!(bases.contains(&oid_b));
    assert!(bases.contains(&oid_c));

    // Best merge base picks deterministically
    let best_base = merge_base(&graph, &oid_d, &oid_e).unwrap();
    assert!(best_base.is_some());
    assert!(bases.contains(&best_base.unwrap()));

    // Now test merge_commits on this criss-cross repository
    // Set HEAD to D
    repo.refs()
        .write_ref(
            "refs/heads/main",
            &ReferenceTarget::Direct(oid_d),
            None,
            None,
        )
        .unwrap();
    repo.set_head(&ReferenceTarget::Symbolic("refs/heads/main".into()))
        .unwrap();

    let outcome = merge_commits(&repo, "branch-e", &oid_e, Some("Merge D and E")).unwrap();
    match outcome {
        MergeOutcome::Clean { commit_oid } => {
            let raw_merged = cas.read_raw(&commit_oid).unwrap();
            let merged_commit = Commit::deserialize(&raw_merged.data).unwrap();
            assert_eq!(merged_commit.parents.len(), 2);
            assert_eq!(merged_commit.parents[0], oid_d);
            assert_eq!(merged_commit.parents[1], oid_e);
        }
        _ => panic!(
            "Expected clean merge between identical trees D and E, got {:?}",
            outcome
        ),
    }
}

// =========================================================================
// SECTION 3: STAGING & WORKTREE ADVERSARIAL CHALLENGES
// =========================================================================

#[test]
fn test_stage_empty_file() {
    let dir = tempdir().expect("tempdir");
    let repo = init(dir.path(), &InitOptions::default()).expect("repo init");
    let workdir = repo.workdir().unwrap();

    let empty_file = workdir.join("empty.txt");
    fs::write(&empty_file, b"").unwrap();

    add_paths(&repo, &[PathBuf::from("empty.txt")], false, false).unwrap();

    let index = repo.index().unwrap();
    let entry = index
        .find_entry("empty.txt", Stage::Normal)
        .expect("entry must exist in index");
    assert_eq!(entry.file_size, 0);

    // In Daft CAS, object framing is "blob 0\0", SHA-256 is 473a0f4c3be8a93681a267e3b1e9a7dcda1185436fe141f7749120a303721813
    assert_eq!(
        entry.oid.to_hex(),
        "473a0f4c3be8a93681a267e3b1e9a7dcda1185436fe141f7749120a303721813"
    );

    let status = get_status(&repo).unwrap();
    assert_eq!(status.staged_added, vec!["empty.txt"]);
}

#[test]
fn test_stage_binary_file_with_null_bytes() {
    let dir = tempdir().expect("tempdir");
    let repo = init(dir.path(), &InitOptions::default()).expect("repo init");
    let workdir = repo.workdir().unwrap();

    let binary_data = vec![0x00, 0xFF, 0xFE, 0x00, 0xAA, 0x55, 0x00];
    let bin_path = workdir.join("binary.dat");
    fs::write(&bin_path, &binary_data).unwrap();

    add_paths(&repo, &[PathBuf::from("binary.dat")], false, false).unwrap();

    let index = repo.index().unwrap();
    let entry = index
        .find_entry("binary.dat", Stage::Normal)
        .expect("binary entry");
    assert_eq!(entry.file_size as usize, binary_data.len());

    let raw = repo.cas().read_raw(&entry.oid).unwrap();
    assert_eq!(
        raw.data, binary_data,
        "CAS must store binary content exactly"
    );
}

#[test]
fn test_stage_deeply_nested_paths_and_tree_building() {
    let dir = tempdir().expect("tempdir");
    let repo = init(dir.path(), &InitOptions::default()).expect("repo init");
    let workdir = repo.workdir().unwrap();

    // 25 levels of directory nesting
    let mut nested_rel = PathBuf::new();
    for i in 0..25 {
        nested_rel = nested_rel.join(format!("level_{:02}", i));
    }
    let leaf_rel = nested_rel.join("leaf.txt");
    let full_leaf = workdir.join(&leaf_rel);
    fs::create_dir_all(full_leaf.parent().unwrap()).unwrap();
    fs::write(&full_leaf, b"deep content\n").unwrap();

    add_paths(&repo, &[PathBuf::from(".")], false, false).unwrap();

    let index = repo.index().unwrap();
    let normalized = leaf_rel.to_string_lossy().replace('\\', "/");
    assert!(index.find_entry(&normalized, Stage::Normal).is_some());

    // Build hierarchical tree
    let mut files = BTreeMap::new();
    for e in index.entries() {
        files.insert(e.path.clone(), (FileMode(e.mode), e.oid));
    }
    let root_tree_oid = build_hierarchical_tree(repo.cas().as_ref(), &files).unwrap();
    assert_eq!(root_tree_oid.to_hex().len(), 64);
}

#[test]
fn test_stage_paths_with_whitespace_and_unicode() {
    let dir = tempdir().expect("tempdir");
    let repo = init(dir.path(), &InitOptions::default()).expect("repo init");
    let workdir = repo.workdir().unwrap();

    let space_dir = workdir.join("path with multiple spaces");
    fs::create_dir_all(&space_dir).unwrap();
    let space_file = space_dir.join("my test file.txt");
    fs::write(&space_file, b"content with space\n").unwrap();

    let unicode_dir = workdir.join("日本語ディレクトリ");
    fs::create_dir_all(&unicode_dir).unwrap();
    let unicode_file = unicode_dir.join("документ.txt");
    fs::write(&unicode_file, b"unicode content\n").unwrap();

    add_paths(&repo, &[PathBuf::from(".")], false, false).unwrap();

    let index = repo.index().unwrap();
    assert!(index
        .find_entry("path with multiple spaces/my test file.txt", Stage::Normal)
        .is_some());
    assert!(index
        .find_entry("日本語ディレクトリ/документ.txt", Stage::Normal)
        .is_some());

    let status = get_status(&repo).unwrap();
    assert_eq!(status.staged_added.len(), 2);
}

#[test]
fn test_stage_deleted_files_and_update_flag() {
    let dir = tempdir().expect("tempdir");
    let repo = init(dir.path(), &InitOptions::default()).expect("repo init");
    let workdir = repo.workdir().unwrap();

    let file_a = workdir.join("file_a.txt");
    let file_b = workdir.join("file_b.txt");
    fs::write(&file_a, b"version 1\n").unwrap();
    fs::write(&file_b, b"version 1\n").unwrap();

    add_paths(&repo, &[PathBuf::from(".")], false, false).unwrap();
    let mut index = repo.index().unwrap();
    assert_eq!(index.entries().len(), 2);

    // Delete file_a from disk, create file_c (untracked), and modify file_b
    fs::remove_file(&file_a).unwrap();
    fs::write(&file_b, b"version 2\n").unwrap();
    let file_c = workdir.join("file_c.txt");
    fs::write(&file_c, b"untracked\n").unwrap();

    // Run add with update=true: only file_b should update; file_c should NOT be staged
    add_paths(&repo, &[PathBuf::from(".")], true, true).unwrap();
    index = repo.index().unwrap();
    assert!(index.find_entry("file_c.txt", Stage::Normal).is_none());

    // Run add with all=true: file_c should be staged, deleted file_a should be removed
    add_paths(&repo, &[PathBuf::from(".")], true, false).unwrap();
    index = repo.index().unwrap();
    assert!(
        index.find_entry("file_a.txt", Stage::Normal).is_none(),
        "Deleted file must be removed from index"
    );
    assert!(
        index.find_entry("file_c.txt", Stage::Normal).is_some(),
        "New file must be staged"
    );
}

#[test]
fn test_adversarial_concurrent_staging_lock_safety() {
    let dir = tempdir().expect("tempdir");
    let repo = Arc::new(init(dir.path(), &InitOptions::default()).expect("repo init"));
    let workdir = repo.workdir().unwrap().to_path_buf();

    // Create 10 distinct files
    for i in 0..10 {
        let path = workdir.join(format!("file_{:02}.txt", i));
        fs::write(&path, format!("thread content {}\n", i)).unwrap();
    }

    let mut handles = Vec::new();
    for i in 0..10 {
        let repo_clone = Arc::clone(&repo);
        let path_name = format!("file_{:02}.txt", i);
        let handle = thread::spawn(move || {
            // Stage single file
            let _ = add_paths(&repo_clone, &[PathBuf::from(&path_name)], false, false);
        });
        handles.push(handle);
    }

    for h in handles {
        let _ = h.join();
    }

    // Index file must remain completely valid and loadable without error
    let index = repo
        .index()
        .expect("Index must not be corrupted by concurrent writes");
    assert!(!index.entries().is_empty());
}
