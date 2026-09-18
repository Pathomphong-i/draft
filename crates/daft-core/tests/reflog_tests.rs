use daft_core::cas::ObjectId;
use daft_core::reflog::{ReflogEntry, ReflogManager};
use tempfile::tempdir;

#[test]
fn test_reflog_append_and_read() {
    let dir = tempdir().unwrap();
    let dft_dir = dir.path().join(".dft");
    let mgr = ReflogManager::new(&dft_dir);

    let old_oid = ObjectId::ZERO;
    let new_oid = ObjectId::hash(b"initial commit");

    let entry = ReflogEntry::new(
        old_oid,
        new_oid,
        "Author Name",
        "author@example.com",
        1600000000,
        "+0000",
        "commit (initial): Initial commit",
    );

    mgr.append("HEAD", &entry).expect("append reflog");

    let entries = mgr.read_all("HEAD").expect("read reflog");
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0], entry);
}

#[test]
fn test_reflog_reverse_iteration() {
    let dir = tempdir().unwrap();
    let dft_dir = dir.path().join(".dft");
    let mgr = ReflogManager::new(&dft_dir);

    for i in 0..10 {
        let entry = ReflogEntry::new(
            ObjectId::ZERO,
            ObjectId::ZERO,
            "Commiter",
            "c@example.com",
            1600000000 + i,
            "+0000",
            format!("action {}", i),
        );
        mgr.append("HEAD", &entry).unwrap();
    }

    let reverse_entries = mgr.read_reverse("HEAD").unwrap();
    assert_eq!(reverse_entries.len(), 10);
    assert_eq!(reverse_entries[0].message, "action 9");
    assert_eq!(reverse_entries[9].message, "action 0");
}
