use crate::cli::{
    CatFileArgs, CommitTreeArgs, HashObjectArgs, LsFilesArgs, LsTreeArgs, RevParseArgs,
    SymbolicRefArgs, UpdateRefArgs, WriteTreeArgs,
};
use crate::error::CliError;
use daft_core::cas::{ObjectId, ObjectType};
use daft_core::plumbing::{
    cat_file, commit_tree, hash_object, ls_files, ls_tree, rev_parse, symbolic_ref, update_ref,
    write_tree,
};
use daft_core::Repository;
use std::env;

pub fn execute_hash_object(args: HashObjectArgs) -> Result<(), CliError> {
    let cwd = env::current_dir()?;
    let repo = Repository::discover(&cwd)?;
    let obj_type = match args.object_type.as_str() {
        "tree" => ObjectType::Tree,
        "commit" => ObjectType::Commit,
        "tag" => ObjectType::Tag,
        _ => ObjectType::Blob,
    };
    let oid = hash_object(&args.file, obj_type, args.write, repo.cas().as_ref())?;
    println!("{}", oid.to_hex());
    Ok(())
}

pub fn execute_cat_file(args: CatFileArgs) -> Result<(), CliError> {
    let cwd = env::current_dir()?;
    let repo = Repository::discover(&cwd)?;
    let raw = cat_file(&repo, &args.object)?;

    if args.show_type {
        println!("{}", raw.object_type);
    } else if args.show_size {
        println!("{}", raw.data.len());
    } else {
        match raw.object_type {
            ObjectType::Blob => {
                if let Ok(text) = std::str::from_utf8(&raw.data) {
                    print!("{}", text);
                } else {
                    use std::io::Write;
                    std::io::stdout().write_all(&raw.data)?;
                }
            }
            _ => {
                if let Ok(text) = std::str::from_utf8(&raw.data) {
                    print!("{}", text);
                } else {
                    println!("[binary {} bytes]", raw.data.len());
                }
            }
        }
    }
    Ok(())
}

pub fn execute_write_tree(_args: WriteTreeArgs) -> Result<(), CliError> {
    let cwd = env::current_dir()?;
    let repo = Repository::discover(&cwd)?;
    let oid = write_tree(&repo)?;
    println!("{}", oid.to_hex());
    Ok(())
}

pub fn execute_commit_tree(args: CommitTreeArgs) -> Result<(), CliError> {
    let cwd = env::current_dir()?;
    let repo = Repository::discover(&cwd)?;
    let tree_oid = ObjectId::from_hex(&args.tree)?;
    let mut parent_oids = Vec::new();
    for p in args.parents {
        parent_oids.push(ObjectId::from_hex(&p)?);
    }

    let oid = commit_tree(&repo, &tree_oid, &parent_oids, &args.message)?;
    println!("{}", oid.to_hex());
    Ok(())
}

pub fn execute_ls_tree(args: LsTreeArgs) -> Result<(), CliError> {
    let cwd = env::current_dir()?;
    let repo = Repository::discover(&cwd)?;
    let entries = ls_tree(&repo, &args.tree, args.recursive)?;

    for e in entries {
        println!(
            "{:06o} {} {}\t{}",
            e.mode.0,
            e.object_type,
            e.oid.to_hex(),
            e.path
        );
    }
    Ok(())
}

pub fn execute_ls_files(args: LsFilesArgs) -> Result<(), CliError> {
    let cwd = env::current_dir()?;
    let repo = Repository::discover(&cwd)?;
    let files = ls_files(&repo, args.stage, args.unmerged, args.others)?;

    for f in files {
        println!("{}", f);
    }
    Ok(())
}

pub fn execute_rev_parse(args: RevParseArgs) -> Result<(), CliError> {
    let cwd = env::current_dir()?;
    let repo = Repository::discover(&cwd)?;

    if args.show_toplevel {
        if let Some(workdir) = repo.workdir() {
            println!("{}", workdir.display());
            return Ok(());
        }
    }

    if args.is_inside_work_tree {
        println!("{}", !repo.is_bare());
        return Ok(());
    }

    if let Some(rev) = &args.rev {
        let oid = rev_parse(&repo, rev)?;
        println!("{}", oid.to_hex());
    }
    Ok(())
}

pub fn execute_update_ref(args: UpdateRefArgs) -> Result<(), CliError> {
    let cwd = env::current_dir()?;
    let repo = Repository::discover(&cwd)?;
    let new_oid = ObjectId::from_hex(&args.new_oid)?;
    let old_oid = if let Some(old) = &args.old_oid {
        Some(ObjectId::from_hex(old)?)
    } else {
        None
    };

    update_ref(&repo, &args.ref_name, &new_oid, old_oid.as_ref())?;
    Ok(())
}

pub fn execute_symbolic_ref(args: SymbolicRefArgs) -> Result<(), CliError> {
    let cwd = env::current_dir()?;
    let repo = Repository::discover(&cwd)?;
    let res = symbolic_ref(&repo, &args.name, args.target.as_deref())?;
    if args.target.is_none() {
        println!("{}", res);
    }
    Ok(())
}
