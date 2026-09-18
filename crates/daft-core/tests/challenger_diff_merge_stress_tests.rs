//! Empirical Adversarial Challenger Test Suite for Myers Diff & 3-Way Merge
//! Written by teamwork_preview_challenger_m2_it2_1.

use daft_core::cas::{ObjectType, RawObject};
use daft_core::diff::myers::{myers_diff, EditOpKind};
use daft_core::graph::{all_merge_bases, StoreCommitGraph};
use daft_core::init::{init, InitOptions};
use daft_core::merge::three_way::merge_text_3way;
use daft_core::merge::{merge_commits, MergeOutcome};
use daft_core::object::{Commit, FileMode, Signature, Tree, TreeEntry};
use daft_core::refs::ReferenceTarget;
use tempfile::tempdir;

/// Verification Oracle: Reconstructs text from Myers diff EditOp sequence
fn apply_diff_oracle(old_text: &str, ops: &[daft_core::diff::myers::EditOp]) -> String {
    let mut result = String::new();
    let mut old_lines: Vec<&str> = Vec::new();
    let mut start = 0;
    for (i, b) in old_text.bytes().enumerate() {
        if b == b'\n' {
            old_lines.push(&old_text[start..=i]);
            start = i + 1;
        }
    }
    if start < old_text.len() {
        old_lines.push(&old_text[start..]);
    }

    let mut old_idx = 0;
    for op in ops {
        match op.kind {
            EditOpKind::Equal => {
                assert!(
                    old_idx < old_lines.len(),
                    "Oracle: Equal op out of bounds on old lines"
                );
                assert_eq!(
                    old_lines[old_idx], op.text,
                    "Oracle: Equal op text mismatch with old line"
                );
                result.push_str(op.text);
                old_idx += 1;
            }
            EditOpKind::Delete => {
                assert!(
                    old_idx < old_lines.len(),
                    "Oracle: Delete op out of bounds on old lines"
                );
                assert_eq!(
                    old_lines[old_idx], op.text,
                    "Oracle: Delete op text mismatch with old line"
                );
                old_idx += 1;
            }
            EditOpKind::Insert => {
                result.push_str(op.text);
            }
        }
    }
    assert_eq!(
        old_idx,
        old_lines.len(),
        "Oracle: not all old lines consumed by diff ops"
    );
    result
}

// =========================================================================
// SECTION 1: MYERS DIFF ADVERSARIAL STRESS TESTS
// =========================================================================

#[test]
fn test_challenger_diff_reconstruction_oracle_alternating_edits() {
    let n = 100;
    let mut old_text = String::new();
    let mut new_text = String::new();

    for i in 0..n {
        if i % 2 == 0 {
            // Clean island
            let island = format!("clean_island_line_{:03}\n", i);
            old_text.push_str(&island);
            new_text.push_str(&island);
        } else {
            // Alternating edit
            old_text.push_str(&format!("old_divergent_line_{:03}\n", i));
            new_text.push_str(&format!("new_divergent_line_{:03}\n", i));
        }
    }

    let ops = myers_diff(&old_text, &new_text);

    // Verify reconstruction oracle
    let reconstructed = apply_diff_oracle(&old_text, &ops);
    assert_eq!(
        reconstructed, new_text,
        "Reconstructed text must match new_text bit-for-bit"
    );

    // Verify all even lines are identified as Equal clean islands
    let equal_ops: Vec<&str> = ops
        .iter()
        .filter(|op| op.kind == EditOpKind::Equal)
        .map(|op| op.text)
        .collect();

    assert_eq!(
        equal_ops.len(),
        n / 2,
        "All clean islands must be preserved as Equal ops"
    );
    for (idx, &eq_text) in equal_ops.iter().enumerate() {
        let expected = format!("clean_island_line_{:03}\n", idx * 2);
        assert_eq!(eq_text, expected);
    }
}

#[test]
fn test_challenger_diff_unicode_grapheme_clusters_and_bidi() {
    let old_text = "\
🇺🇸 United States
👨‍👩‍👧‍👦 Family ZWJ Emoji
🤹🏽‍♂️ Multibyte Skin Tone Gender
n\u{0303}o\u{0308}e\u{0301}l Combining Diacritics
مرحبا بالعالم Arabic RTL
שלום עולם Hebrew RTL
Line with U+2028 \u{2028} Line Separator embedded
Final clean line
";

    let new_text = "\
🇺🇸 United States
👨‍👩‍👧‍👦 Family ZWJ Emoji
🤹🏿‍♀️ Modified Skin Tone Gender
ñöél Precomposed Diacritics
مرحبا بكم Arabic RTL Modified
שלום עולם Hebrew RTL
Line with U+2028 \u{2028} Line Separator embedded
Final clean line
";

    let ops = myers_diff(old_text, new_text);

    // Verify reconstruction oracle
    let reconstructed = apply_diff_oracle(old_text, &ops);
    assert_eq!(
        reconstructed, new_text,
        "Unicode diff reconstruction must match exactly"
    );

    // Verify specific clean lines
    let equals: Vec<&str> = ops
        .iter()
        .filter(|op| op.kind == EditOpKind::Equal)
        .map(|op| op.text)
        .collect();

    assert!(equals.contains(&"🇺🇸 United States\n"));
    assert!(equals.contains(&"👨‍👩‍👧‍👦 Family ZWJ Emoji\n"));
    assert!(equals.contains(&"שלום עולם Hebrew RTL\n"));
    assert!(equals.contains(&"Line with U+2028 \u{2028} Line Separator embedded\n"));
    assert!(equals.contains(&"Final clean line\n"));
}

#[test]
fn test_challenger_diff_clean_islands_boundary_matrix() {
    // 1. Single line clean island between large deletions/insertions
    let old = "DEL_1\nDEL_2\nDEL_3\nISLAND\nDEL_4\nDEL_5\n";
    let new = "INS_1\nINS_2\nISLAND\nINS_3\nINS_4\n";
    let ops = myers_diff(old, new);
    let reconstructed = apply_diff_oracle(old, &ops);
    assert_eq!(reconstructed, new);
    let island_op = ops.iter().find(|op| op.text == "ISLAND\n");
    assert!(island_op.is_some());
    assert_eq!(island_op.unwrap().kind, EditOpKind::Equal);
    assert_eq!(island_op.unwrap().old_line, Some(4));
    assert_eq!(island_op.unwrap().new_line, Some(3));

    // 2. Huge clean island (200 lines) between single line edits
    let mut old_huge = "header_old\n".to_string();
    let mut new_huge = "header_new\n".to_string();
    for i in 0..200 {
        let island_line = format!("island_bulk_line_{}\n", i);
        old_huge.push_str(&island_line);
        new_huge.push_str(&island_line);
    }
    old_huge.push_str("footer_old\n");
    new_huge.push_str("footer_new\n");

    let ops_huge = myers_diff(&old_huge, &new_huge);
    let reconstructed_huge = apply_diff_oracle(&old_huge, &ops_huge);
    assert_eq!(reconstructed_huge, new_huge);
    let equals_huge = ops_huge
        .iter()
        .filter(|op| op.kind == EditOpKind::Equal)
        .count();
    assert_eq!(equals_huge, 200);
}

#[test]
fn test_challenger_diff_crlf_and_mixed_line_endings() {
    // CRLF on both sides
    let old_crlf = "line 1\r\nline 2\r\nline 3\r\n";
    let new_crlf = "line 1\r\nline 2 mod\r\nline 3\r\n";
    let ops_crlf = myers_diff(old_crlf, new_crlf);
    assert_eq!(apply_diff_oracle(old_crlf, &ops_crlf), new_crlf);
    let eq_crlf: Vec<&str> = ops_crlf
        .iter()
        .filter(|op| op.kind == EditOpKind::Equal)
        .map(|op| op.text)
        .collect();
    assert_eq!(eq_crlf, vec!["line 1\r\n", "line 3\r\n"]);

    // No trailing newline on both sides
    let old_notrail = "alpha\nbeta";
    let new_notrail = "alpha\nbeta modified";
    let ops_notrail = myers_diff(old_notrail, new_notrail);
    assert_eq!(apply_diff_oracle(old_notrail, &ops_notrail), new_notrail);
}

// =========================================================================
// SECTION 2: 3-WAY MERGE ADVERSARIAL STRESS TESTS
// =========================================================================

#[test]
fn test_challenger_merge_multi_conflict_clean_islands_20_blocks() {
    let count = 20;
    let mut base = String::new();
    let mut ours = String::new();
    let mut theirs = String::new();

    for i in 0..count {
        let island = format!("clean_island_separator_{:02}\n", i);
        base.push_str(&island);
        ours.push_str(&island);
        theirs.push_str(&island);

        let base_conflict = format!("conflict_block_{:02}_base\n", i);
        let ours_conflict = format!("conflict_block_{:02}_ours\n", i);
        let theirs_conflict = format!("conflict_block_{:02}_theirs\n", i);

        base.push_str(&base_conflict);
        ours.push_str(&ours_conflict);
        theirs.push_str(&theirs_conflict);
    }
    let final_island = format!("clean_island_separator_{:02}\n", count);
    base.push_str(&final_island);
    ours.push_str(&final_island);
    theirs.push_str(&final_island);

    let res = merge_text_3way(&base, &ours, &theirs, "HEAD", "feature");
    assert!(res.has_conflicts);

    // Check that every single clean island from 0 to 20 is intact and present
    for i in 0..=count {
        let island = format!("clean_island_separator_{:02}\n", i);
        assert!(
            res.content.contains(&island),
            "Missing clean island {:02} in merged content!",
            i
        );
    }

    // Check conflict count by counting "<<<<<<< HEAD\n"
    let conflict_count = res.content.matches("<<<<<<< HEAD\n").count();
    assert_eq!(
        conflict_count, count,
        "Expected exactly {} conflict blocks, got {}",
        count, conflict_count
    );

    // Verify ordering: clean island i, then conflict i, then clean island i+1
    for i in 0..count {
        let island_before = format!("clean_island_separator_{:02}\n", i);
        let marker = format!(
            "<<<<<<< HEAD\nconflict_block_{:02}_ours\n=======\nconflict_block_{:02}_theirs\n>>>>>>> feature\n",
            i, i
        );
        let pos_island = res.content.find(&island_before).unwrap();
        let pos_marker = res.content.find(&marker).unwrap();
        assert!(
            pos_island < pos_marker,
            "Clean island {} must precede conflict {}",
            i,
            i
        );
    }
}

#[test]
fn test_challenger_merge_cascading_overlapping_conflict_window() {
    // Overlapping edits that require iterative window expansion
    // Base lines: 0, 1, 2, 3, 4, 5, 6, 7, 8
    let mut base = String::new();
    for i in 0..9 {
        base.push_str(&format!("line {}\n", i));
    }

    // Ours modifies lines 1..3 and lines 5..7
    let mut ours = String::new();
    ours.push_str("line 0\n");
    ours.push_str("ours mod 1-3\n");
    ours.push_str("line 4\n");
    ours.push_str("ours mod 5-7\n");
    ours.push_str("line 8\n");

    // Theirs modifies lines 2..6 (overlaps with BOTH ours 1..3 and ours 5..7)
    let mut theirs = String::new();
    theirs.push_str("line 0\n");
    theirs.push_str("line 1\n");
    theirs.push_str("theirs mod 2-6\n");
    theirs.push_str("line 7\n");
    theirs.push_str("line 8\n");

    let res = merge_text_3way(&base, &ours, &theirs, "HEAD", "feature");
    assert!(res.has_conflicts);

    // The entire window lines 1..7 must expand into ONE single conflict block
    let conflict_count = res.content.matches("<<<<<<< HEAD\n").count();
    assert_eq!(
        conflict_count, 1,
        "Iterative window expansion must fuse chained overlapping edits into 1 conflict window"
    );

    // Line 0 and Line 8 must remain clean islands outside
    assert!(res.content.starts_with("line 0\n<<<<<<< HEAD\n"));
    assert!(res.content.ends_with(">>>>>>> feature\nline 8\n"));
}

#[test]
fn test_challenger_merge_identical_modifications_clean() {
    let base = "line 0\nline 1\nline 2\nline 3\n";
    let ours = "line 0\nline 1 MOD\nline 2\nline 3 MOD\n";
    let theirs = "line 0\nline 1 MOD\nline 2\nline 3 MOD\n";

    let res = merge_text_3way(base, ours, theirs, "HEAD", "theirs");
    assert!(
        !res.has_conflicts,
        "Identical changes on both sides must merge cleanly"
    );
    assert_eq!(res.content, ours);
}

#[test]
fn test_challenger_merge_adjacent_lines_no_conflict() {
    let base = "line 0\nline 1\nline 2\nline 3\n";
    let ours = "line 0\nline 1 OURS\nline 2\nline 3\n";
    let theirs = "line 0\nline 1\nline 2 THEIRS\nline 3\n";

    let res = merge_text_3way(base, ours, theirs, "HEAD", "theirs");
    assert!(
        !res.has_conflicts,
        "Adjacent independent line edits must not conflict"
    );
    assert_eq!(res.content, "line 0\nline 1 OURS\nline 2 THEIRS\nline 3\n");
}

#[test]
fn test_challenger_merge_delete_modify_scenarios() {
    // 1. Both delete the same line -> Clean delete
    let base1 = "line 1\nline 2\nline 3\n";
    let ours1 = "line 1\nline 3\n";
    let theirs1 = "line 1\nline 3\n";
    let res1 = merge_text_3way(base1, ours1, theirs1, "HEAD", "theirs");
    assert!(!res1.has_conflicts);
    assert_eq!(res1.content, "line 1\nline 3\n");

    // 2. Ours deletes, theirs modifies -> Conflict
    let base2 = "line 1\nline 2\nline 3\n";
    let ours2 = "line 1\nline 3\n";
    let theirs2 = "line 1\nline 2 MODIFIED\nline 3\n";
    let res2 = merge_text_3way(base2, ours2, theirs2, "HEAD", "theirs");
    assert!(res2.has_conflicts);
    assert!(res2
        .content
        .contains("<<<<<<< HEAD\n=======\nline 2 MODIFIED\n>>>>>>> theirs\n"));

    // 3. Ours deletes line 1, theirs modifies line 3 -> Clean
    let base3 = "line 1\nline 2\nline 3\n";
    let ours3 = "line 2\nline 3\n";
    let theirs3 = "line 1\nline 2\nline 3 MODIFIED\n";
    let res3 = merge_text_3way(base3, ours3, theirs3, "HEAD", "theirs");
    assert!(!res3.has_conflicts);
    assert_eq!(res3.content, "line 2\nline 3 MODIFIED\n");
}

#[test]
fn test_challenger_criss_cross_merge_divergent_changes() {
    // Construct real criss-cross commit DAG with divergent changes:
    //       A
    //      / \
    //     B   C
    //     |\ /|
    //     | X |
    //     |/ \|
    //     D   E
    //
    // A introduces initial common file.
    // B adds file_b.txt
    // C adds file_c.txt
    // D merges B and C (has both file_b.txt and file_c.txt)
    // E merges C and B (has both file_b.txt and file_c.txt)
    //
    // Then:
    // D branch modifies common.txt with "D change"
    // E branch modifies common.txt with "E change"
    let dir = tempdir().expect("tempdir");
    let repo = init(dir.path(), &InitOptions::default()).expect("repo init");
    let cas = repo.cas();
    let sig = Signature::now("Challenger Tester", "test@daft-vcs.org");

    // Common file
    let common_blob_a = cas
        .write_raw(&RawObject::new(
            ObjectType::Blob,
            b"common line 1\ncommon line 2\n".to_vec(),
        ))
        .unwrap();
    let tree_a = Tree::from_entries(vec![TreeEntry::new(
        FileMode::REGULAR,
        "common.txt",
        common_blob_a,
    )
    .unwrap()])
    .unwrap();
    let tree_a_oid = cas
        .write_raw(&RawObject::new(ObjectType::Tree, tree_a.serialize()))
        .unwrap();

    let commit_a = Commit::new(tree_a_oid, vec![], sig.clone(), sig.clone(), "A");
    let oid_a = cas
        .write_raw(&RawObject::new(ObjectType::Commit, commit_a.serialize()))
        .unwrap();

    // Tree B
    let blob_b = cas
        .write_raw(&RawObject::new(ObjectType::Blob, b"file B\n".to_vec()))
        .unwrap();
    let tree_b = Tree::from_entries(vec![
        TreeEntry::new(FileMode::REGULAR, "common.txt", common_blob_a).unwrap(),
        TreeEntry::new(FileMode::REGULAR, "file_b.txt", blob_b).unwrap(),
    ])
    .unwrap();
    let tree_b_oid = cas
        .write_raw(&RawObject::new(ObjectType::Tree, tree_b.serialize()))
        .unwrap();
    let commit_b = Commit::new(tree_b_oid, vec![oid_a], sig.clone(), sig.clone(), "B");
    let oid_b = cas
        .write_raw(&RawObject::new(ObjectType::Commit, commit_b.serialize()))
        .unwrap();

    // Tree C
    let blob_c = cas
        .write_raw(&RawObject::new(ObjectType::Blob, b"file C\n".to_vec()))
        .unwrap();
    let tree_c = Tree::from_entries(vec![
        TreeEntry::new(FileMode::REGULAR, "common.txt", common_blob_a).unwrap(),
        TreeEntry::new(FileMode::REGULAR, "file_c.txt", blob_c).unwrap(),
    ])
    .unwrap();
    let tree_c_oid = cas
        .write_raw(&RawObject::new(ObjectType::Tree, tree_c.serialize()))
        .unwrap();
    let commit_c = Commit::new(tree_c_oid, vec![oid_a], sig.clone(), sig.clone(), "C");
    let oid_c = cas
        .write_raw(&RawObject::new(ObjectType::Commit, commit_c.serialize()))
        .unwrap();

    // Commit D: merges B and C, modifies common.txt -> "D change"
    let common_blob_d = cas
        .write_raw(&RawObject::new(
            ObjectType::Blob,
            b"common line 1 D\ncommon line 2\n".to_vec(),
        ))
        .unwrap();
    let tree_d = Tree::from_entries(vec![
        TreeEntry::new(FileMode::REGULAR, "common.txt", common_blob_d).unwrap(),
        TreeEntry::new(FileMode::REGULAR, "file_b.txt", blob_b).unwrap(),
        TreeEntry::new(FileMode::REGULAR, "file_c.txt", blob_c).unwrap(),
    ])
    .unwrap();
    let tree_d_oid = cas
        .write_raw(&RawObject::new(ObjectType::Tree, tree_d.serialize()))
        .unwrap();
    let commit_d = Commit::new(
        tree_d_oid,
        vec![oid_b, oid_c],
        sig.clone(),
        sig.clone(),
        "D",
    );
    let oid_d = cas
        .write_raw(&RawObject::new(ObjectType::Commit, commit_d.serialize()))
        .unwrap();

    // Commit E: merges C and B, modifies common.txt -> "E change"
    let common_blob_e = cas
        .write_raw(&RawObject::new(
            ObjectType::Blob,
            b"common line 1 E\ncommon line 2\n".to_vec(),
        ))
        .unwrap();
    let tree_e = Tree::from_entries(vec![
        TreeEntry::new(FileMode::REGULAR, "common.txt", common_blob_e).unwrap(),
        TreeEntry::new(FileMode::REGULAR, "file_b.txt", blob_b).unwrap(),
        TreeEntry::new(FileMode::REGULAR, "file_c.txt", blob_c).unwrap(),
    ])
    .unwrap();
    let tree_e_oid = cas
        .write_raw(&RawObject::new(ObjectType::Tree, tree_e.serialize()))
        .unwrap();
    let commit_e = Commit::new(
        tree_e_oid,
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
    assert_eq!(bases.len(), 2, "Must find both B and C as merge bases");

    // Perform merge_commits with HEAD at D, merging E
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
        MergeOutcome::Conflict { conflicting_files } => {
            assert_eq!(conflicting_files, vec!["common.txt"]);
            let workdir = repo.workdir().unwrap();
            let merged_common = std::fs::read_to_string(workdir.join("common.txt")).unwrap();
            assert!(merged_common.contains(
                "<<<<<<< HEAD\ncommon line 1 D\n=======\ncommon line 1 E\n>>>>>>> branch-e\n"
            ));
            assert!(merged_common.contains("common line 2\n"));
        }
        _ => panic!("Expected conflicted merge on common.txt, got {:?}", outcome),
    }
}
