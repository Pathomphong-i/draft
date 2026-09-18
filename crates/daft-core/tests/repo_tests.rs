use daft_core::error::RepoError;
use daft_core::init::{init, InitOptions};
use daft_core::repo::Repository;
use std::fs;
use tempfile::tempdir;

#[test]
fn test_init_non_bare() {
    let dir = tempdir().unwrap();
    let repo = init(dir.path(), &InitOptions::default()).expect("init non-bare");

    assert!(!repo.is_bare());
    assert_eq!(
        repo.workdir().unwrap().canonicalize().unwrap(),
        dir.path().canonicalize().unwrap()
    );
    assert_eq!(
        repo.dft_dir().canonicalize().unwrap(),
        dir.path().join(".dft").canonicalize().unwrap()
    );

    let dft_dir = repo.dft_dir();
    assert!(dft_dir.join("objects/tmp").is_dir());
    assert!(dft_dir.join("refs/heads").is_dir());
    assert!(dft_dir.join("HEAD").is_file());
    assert!(dft_dir.join("dft.toml").is_file());
    assert!(dft_dir.join("current_dimension").is_file());

    let head = repo.head().expect("read HEAD");
    assert!(head.is_symbolic());
    assert_eq!(head.name, "HEAD");
}

#[test]
fn test_init_bare() {
    let dir = tempdir().unwrap();
    let repo = init(
        dir.path(),
        &InitOptions {
            bare: true,
            initial_branch: "trunk".to_string(),
            reinit: false,
        },
    )
    .expect("init bare");

    assert!(repo.is_bare());
    assert_eq!(repo.workdir(), None);
    assert_eq!(
        repo.dft_dir().canonicalize().unwrap(),
        dir.path().canonicalize().unwrap()
    );

    let head_content = fs::read_to_string(dir.path().join("HEAD")).unwrap();
    assert_eq!(head_content, "ref: refs/heads/trunk\n");
}

#[test]
fn test_discover_from_nested_subdir() {
    let dir = tempdir().unwrap();
    init(dir.path(), &InitOptions::default()).expect("init repo");

    let deep_nested = dir.path().join("a").join("b").join("c").join("d");
    fs::create_dir_all(&deep_nested).unwrap();

    let discovered = Repository::discover(&deep_nested).expect("discover");
    assert_eq!(
        discovered.workdir().unwrap().canonicalize().unwrap(),
        dir.path().canonicalize().unwrap()
    );
}

#[test]
fn test_discover_outside_repo() {
    let dir = tempdir().unwrap();
    let res = Repository::discover(dir.path());
    assert!(matches!(res, Err(RepoError::NotARepository(_))));
}
