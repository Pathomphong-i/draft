//! Copy-on-Write (CoW) Storage Engine for Daft VCS.
//! Supports macOS APFS `clonefile`, Linux `ioctl(FICLONE)`, POSIX hardlinks with Break-on-Write (BoW),
//! and Rayon parallel copy fallback.

use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::Path;
use std::sync::OnceLock;
use walkdir::WalkDir;

/// Supported CoW cloning strategies ranked by efficiency.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CowStrategy {
    /// macOS APFS native clonefile (metadata-level copy-on-write extent sharing).
    Clonefile,
    /// Linux Btrfs / XFS / ZFS native ioctl(FICLONE) reflink.
    Ficlone,
    /// POSIX hardlink with Break-on-Write (BoW) semantics.
    Hardlink,
    /// Multi-threaded byte copy fallback using Rayon.
    Copy,
}

impl std::fmt::Display for CowStrategy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CowStrategy::Clonefile => write!(f, "clonefile"),
            CowStrategy::Ficlone => write!(f, "ficlone"),
            CowStrategy::Hardlink => write!(f, "hardlink"),
            CowStrategy::Copy => write!(f, "copy"),
        }
    }
}

#[cfg(target_os = "macos")]
pub mod darwin {
    use std::ffi::CString;
    use std::io;
    use std::os::unix::ffi::OsStrExt;
    use std::path::Path;

    extern "C" {
        pub fn clonefile(
            src: *const libc::c_char,
            dst: *const libc::c_char,
            flags: libc::c_int,
        ) -> libc::c_int;
    }

    pub const CLONE_NOFOLLOW: libc::c_int = 0x0001;
    pub const CLONE_NOOWNERCOPY: libc::c_int = 0x0002;

    pub fn clone_path(src: &Path, dst: &Path, no_follow: bool) -> io::Result<()> {
        if dst.exists() || dst.is_symlink() {
            if dst.is_dir() {
                let _ = std::fs::remove_dir_all(dst);
            } else {
                let _ = std::fs::remove_file(dst);
            }
        }
        let src_c = CString::new(src.as_os_str().as_bytes())
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;
        let dst_c = CString::new(dst.as_os_str().as_bytes())
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;

        let flags = if no_follow { CLONE_NOFOLLOW } else { 0 };
        let ret = unsafe { clonefile(src_c.as_ptr(), dst_c.as_ptr(), flags) };

        if ret != 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    }
}

#[cfg(target_os = "linux")]
pub mod linux {
    use std::fs::{File, OpenOptions};
    use std::io;
    use std::os::unix::io::AsRawFd;
    use std::path::Path;

    pub const FICLONE: libc::c_ulong = 0x40049409;

    pub fn clone_file(src: &Path, dst: &Path) -> io::Result<()> {
        if dst.exists() || dst.is_symlink() {
            let _ = std::fs::remove_file(dst);
        }
        let src_file = File::open(src)?;
        let dst_file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(dst)?;

        let ret = unsafe { libc::ioctl(dst_file.as_raw_fd(), FICLONE, src_file.as_raw_fd()) };

        if ret < 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    }
}

static DETECTED_STRATEGY: OnceLock<CowStrategy> = OnceLock::new();

/// Detects the optimal CoW strategy supported by the filesystem housing `probe_dir`.
pub fn detect_best_strategy(probe_dir: &Path) -> CowStrategy {
    *DETECTED_STRATEGY.get_or_init(|| probe_filesystem_capabilities(probe_dir))
}

fn probe_filesystem_capabilities(probe_dir: &Path) -> CowStrategy {
    let _ = fs::create_dir_all(probe_dir);
    let probe_src = probe_dir.join(format!(".dft_probe_src_{}", std::process::id()));
    let probe_dst = probe_dir.join(format!(".dft_probe_dst_{}", std::process::id()));

    let _ = fs::remove_file(&probe_src);
    let _ = fs::remove_file(&probe_dst);

    let created = File::create(&probe_src).and_then(|mut f| f.write_all(b"daft_cow_probe"));
    if created.is_err() {
        return CowStrategy::Copy;
    }

    // 1. Check macOS APFS clonefile
    #[cfg(target_os = "macos")]
    {
        if darwin::clone_path(&probe_src, &probe_dst, true).is_ok() {
            let _ = fs::remove_file(&probe_src);
            let _ = fs::remove_file(&probe_dst);
            return CowStrategy::Clonefile;
        }
    }

    // 2. Check Linux FICLONE ioctl
    #[cfg(target_os = "linux")]
    {
        if linux::clone_file(&probe_src, &probe_dst).is_ok() {
            let _ = fs::remove_file(&probe_src);
            let _ = fs::remove_file(&probe_dst);
            return CowStrategy::Ficlone;
        }
    }

    // 3. Check POSIX hardlinks
    if fs::hard_link(&probe_src, &probe_dst).is_ok() {
        let _ = fs::remove_file(&probe_src);
        let _ = fs::remove_file(&probe_dst);
        return CowStrategy::Hardlink;
    }

    let _ = fs::remove_file(&probe_src);
    let _ = fs::remove_file(&probe_dst);
    CowStrategy::Copy
}

/// Break-on-Write (BoW) enforcement: if a file has multiple links (`nlink > 1`),
/// unlink it before writing so that the other link remains unaltered.
pub fn break_on_write_check_and_unlink(path: &Path) -> io::Result<bool> {
    if let Ok(meta) = fs::symlink_metadata(path) {
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            if meta.nlink() > 1 {
                fs::remove_file(path)?;
                return Ok(true);
            }
        }
    }
    Ok(false)
}

/// Copy-on-Write engine providing file and directory cloning primitives with transparent fallbacks.
pub struct CowEngine;

impl CowEngine {
    /// Clones a single file from `src` to `dst` using the requested strategy, falling back gracefully.
    pub fn clone_file(src: &Path, dst: &Path, strategy: CowStrategy) -> io::Result<()> {
        if let Some(parent) = dst.parent() {
            fs::create_dir_all(parent)?;
        }

        if dst.exists() || dst.is_symlink() {
            let _ = fs::remove_file(dst);
        }

        match strategy {
            CowStrategy::Clonefile => {
                #[cfg(target_os = "macos")]
                {
                    match darwin::clone_path(src, dst, true) {
                        Ok(()) => Ok(()),
                        Err(_) => {
                            // Fallback to hardlink or copy
                            if fs::hard_link(src, dst).is_ok() {
                                Ok(())
                            } else {
                                fs::copy(src, dst).map(|_| ())
                            }
                        }
                    }
                }
                #[cfg(not(target_os = "macos"))]
                {
                    if fs::hard_link(src, dst).is_ok() {
                        Ok(())
                    } else {
                        fs::copy(src, dst).map(|_| ())
                    }
                }
            }
            CowStrategy::Ficlone => {
                #[cfg(target_os = "linux")]
                {
                    match linux::clone_file(src, dst) {
                        Ok(()) => Ok(()),
                        Err(_) => {
                            if fs::hard_link(src, dst).is_ok() {
                                Ok(())
                            } else {
                                fs::copy(src, dst).map(|_| ())
                            }
                        }
                    }
                }
                #[cfg(not(target_os = "linux"))]
                {
                    if fs::hard_link(src, dst).is_ok() {
                        Ok(())
                    } else {
                        fs::copy(src, dst).map(|_| ())
                    }
                }
            }
            CowStrategy::Hardlink => match fs::hard_link(src, dst) {
                Ok(()) => Ok(()),
                Err(_) => fs::copy(src, dst).map(|_| ()),
            },
            CowStrategy::Copy => fs::copy(src, dst).map(|_| ()),
        }
    }

    /// Recursively clones a directory from `src_dir` to `dst_dir`.
    /// On macOS APFS, tries native recursive directory cloning first.
    /// Falls back to parallel Rayon file-by-file cloning.
    pub fn clone_dir(src_dir: &Path, dst_dir: &Path, strategy: CowStrategy) -> io::Result<()> {
        if !src_dir.exists() {
            return Ok(());
        }

        #[cfg(target_os = "macos")]
        if strategy == CowStrategy::Clonefile && !dst_dir.exists() {
            if let Some(parent) = dst_dir.parent() {
                let _ = fs::create_dir_all(parent);
            }
            if darwin::clone_path(src_dir, dst_dir, false).is_ok() {
                return Ok(());
            }
        }

        Self::clone_dir_parallel(src_dir, dst_dir, strategy, None)
    }

    /// Clones a workspace directory, optionally ignoring a specified path prefix (e.g. `.dft`).
    pub fn clone_workspace(
        src_dir: &Path,
        dst_dir: &Path,
        strategy: CowStrategy,
        ignore_prefix: Option<&Path>,
    ) -> io::Result<()> {
        fs::create_dir_all(dst_dir)?;
        Self::clone_dir_parallel(src_dir, dst_dir, strategy, ignore_prefix)
    }

    fn clone_dir_parallel(
        src_dir: &Path,
        dst_dir: &Path,
        strategy: CowStrategy,
        ignore_prefix: Option<&Path>,
    ) -> io::Result<()> {
        let mut dirs = Vec::new();
        let mut files = Vec::new();

        for entry in WalkDir::new(src_dir).into_iter().filter_map(|e| e.ok()) {
            let path = entry.path();
            if let Some(ignore) = ignore_prefix {
                if path.starts_with(ignore) {
                    continue;
                }
            }

            if let Ok(rel) = path.strip_prefix(src_dir) {
                if rel.as_os_str().is_empty() {
                    continue;
                }
                let target = dst_dir.join(rel);
                if entry.file_type().is_dir() {
                    dirs.push(target);
                } else if entry.file_type().is_file() {
                    files.push((path.to_path_buf(), target));
                }
            }
        }

        // 1. Pre-create all target directories
        for d in dirs {
            fs::create_dir_all(d)?;
        }

        // 2. Clone all files in parallel via Rayon
        let results: Vec<io::Result<()>> = files
            .into_par_iter()
            .map(|(src, dst)| Self::clone_file(&src, &dst, strategy))
            .collect();

        for res in results {
            res?;
        }

        Ok(())
    }
}
