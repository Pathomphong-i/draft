//! Staging and Working Tree Operations for Daft VCS.

pub mod add;
pub mod clean;
pub mod ignore;
pub mod mv;
pub mod reset;
pub mod restore;
pub mod rm;
pub mod stash;
pub mod status;

pub use add::add_paths;
pub use clean::{clean_untracked, CleanOptions};
pub use ignore::DaftIgnore;
pub use mv::move_path;
pub use reset::{reset, resolve_commit, ResetMode};
pub use restore::restore;
pub use rm::remove_paths;
pub use stash::{stash_apply, stash_clear, stash_drop, stash_list, stash_pop, stash_push};
pub use status::{find_untracked_files, get_status, StatusReport};
