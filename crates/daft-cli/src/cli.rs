//! CLI hierarchy and argument definitions using Clap v4.

use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(
    name = "dft",
    author = "Daft Contributors",
    version = env!("CARGO_PKG_VERSION"),
    about = "Daft - Distributed VCS for Parallel Timelines & Multi-Agent Workflows",
    long_about = "Daft ('dft') is a high-performance version control system designed for parallel AI agent workflows and human-agent collaboration."
)]
pub struct Cli {
    #[arg(
        short = 'v',
        long,
        global = true,
        help = "Show verbose output and debug traces"
    )]
    pub verbose: bool,

    #[arg(short = 'q', long, global = true, help = "Suppress non-error output")]
    pub quiet: bool,

    #[arg(long, global = true, help = "Format command output as structured JSON")]
    pub json: bool,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    // ------------------------------------------------------------------------
    // Layer 1: Setup & Config
    // ------------------------------------------------------------------------
    #[command(about = "Initialize a new Daft repository")]
    Init(InitArgs),

    #[command(about = "Clone a repository into a new directory")]
    Clone(CloneArgs),

    #[command(about = "Get and set repository or global options")]
    Config(ConfigArgs),

    // ------------------------------------------------------------------------
    // Layer 1: Staging & Working Tree
    // ------------------------------------------------------------------------
    #[command(about = "Add file contents to the staging index")]
    Add(AddArgs),

    #[command(about = "Show the working tree and staging status")]
    Status(StatusArgs),

    #[command(about = "Record staged changes to the repository")]
    Commit(CommitArgs),

    #[command(about = "Reset current HEAD to the specified state")]
    Reset(ResetArgs),

    #[command(about = "Restore working tree or staged files")]
    Restore(RestoreArgs),

    #[command(about = "Remove files from the working tree and index")]
    Rm(RmArgs),

    #[command(about = "Move or rename a file, directory, or symlink")]
    Mv(MvArgs),

    #[command(about = "Remove untracked files from the working tree")]
    Clean(CleanArgs),

    #[command(about = "Stash the changes in a dirty working directory away")]
    Stash(StashArgs),

    // ------------------------------------------------------------------------
    // Layer 1: Branching & Merging
    // ------------------------------------------------------------------------
    #[command(about = "List, create, or delete branches")]
    Branch(BranchArgs),

    #[command(about = "Switch branches or restore working tree files")]
    Checkout(CheckoutArgs),

    #[command(about = "Switch branches")]
    Switch(SwitchArgs),

    #[command(about = "Join two or more development histories together")]
    Merge(MergeArgs),

    #[command(about = "Reapply commits on top of another base tip")]
    Rebase(RebaseArgs),

    #[command(
        name = "cherry-pick",
        about = "Apply the changes introduced by some existing commits"
    )]
    CherryPick(CherryPickArgs),

    #[command(about = "Create, list, delete or verify a tag object")]
    Tag(TagArgs),

    // ------------------------------------------------------------------------
    // Layer 1: History & Inspection
    // ------------------------------------------------------------------------
    #[command(about = "Show commit logs")]
    Log(LogArgs),

    #[command(about = "Show changes between commits, commit and working tree, etc.")]
    Diff(DiffArgs),

    #[command(about = "Show various types of objects")]
    Show(ShowArgs),

    #[command(about = "Show what revision and author last modified each line of a file")]
    Blame(BlameArgs),

    #[command(about = "Summarize 'dft log' output by author")]
    Shortlog(ShortlogArgs),

    #[command(about = "Give an object a human readable name based on an available ref")]
    Describe(DescribeArgs),

    #[command(about = "Print lines matching a pattern in tracked files")]
    Grep(GrepArgs),

    #[command(about = "Use binary search to find the commit that introduced a bug")]
    Bisect(BisectArgs),

    #[command(about = "Manage reflog information")]
    Reflog(ReflogArgs),

    // ------------------------------------------------------------------------
    // Layer 1: Plumbing Commands
    // ------------------------------------------------------------------------
    #[command(
        name = "hash-object",
        about = "Compute object ID and optionally create a blob"
    )]
    HashObject(HashObjectArgs),

    #[command(
        name = "cat-file",
        about = "Provide content or type and size information for repository objects"
    )]
    CatFile(CatFileArgs),

    #[command(
        name = "write-tree",
        about = "Create a tree object from the current index"
    )]
    WriteTree(WriteTreeArgs),

    #[command(name = "commit-tree", about = "Create a new commit object")]
    CommitTree(CommitTreeArgs),

    #[command(name = "ls-tree", about = "List the contents of a tree object")]
    LsTree(LsTreeArgs),

    #[command(
        name = "ls-files",
        about = "Show information about files in the index and working tree"
    )]
    LsFiles(LsFilesArgs),

    #[command(name = "rev-parse", about = "Pick out and massage parameters")]
    RevParse(RevParseArgs),

    #[command(
        name = "update-ref",
        about = "Update the object name stored in a ref safely"
    )]
    UpdateRef(UpdateRefArgs),

    #[command(name = "symbolic-ref", about = "Read, modify and delete symbolic refs")]
    SymbolicRef(SymbolicRefArgs),

    // ------------------------------------------------------------------------
    // Layer 1: Maintenance Commands
    // ------------------------------------------------------------------------
    #[command(about = "Cleanup unnecessary files and optimize the local repository")]
    Gc(GcArgs),

    #[command(about = "Verify the connectivity and validity of the objects in the database")]
    Fsck(FsckArgs),

    #[command(about = "Prune all unreachable objects from the object database")]
    Prune(PruneArgs),

    #[command(about = "Pack unpacked objects in a repository")]
    Repack(RepackArgs),

    #[command(
        name = "count-objects",
        about = "Count unpacked objects and display disk consumption"
    )]
    CountObjects(CountObjectsArgs),

    // ------------------------------------------------------------------------
    // Layer 2: Multiverse, Awareness, Convergence, Sync & Agent Commands
    // ------------------------------------------------------------------------
    #[command(about = "Manage parallel dimension workspaces")]
    Dimension(DimensionArgs),

    #[command(about = "Manage immutable dimension snapshots")]
    Snapshot(SnapshotArgs),

    #[command(about = "Inspect other dimensions non-destructively")]
    Observe(ObserveArgs),

    #[command(about = "Cross-dimension activity radar and hot-zone detection")]
    Radar(RadarArgs),

    #[command(about = "Predict merge conflicts before merging")]
    Foresee(ForeseeArgs),

    #[command(about = "Detect concurrently modified files across dimensions")]
    Overlap(OverlapArgs),

    #[command(about = "Measure mathematical divergence entropy between dimensions")]
    Entropy(EntropyArgs),

    #[command(about = "Claim exclusive ownership of a file or directory path")]
    Claim(ClaimArgs),

    #[command(name = "yield", about = "Release a claimed path")]
    Yield(YieldArgs),

    #[command(about = "Place a hard modification barrier on a file or directory path")]
    Fence(FenceArgs),

    #[command(about = "Display or audit the territory ownership map")]
    Territory(TerritoryArgs),

    #[command(about = "Collapse parallel dimensions into a single mainline history")]
    Collapse(CollapseArgs),

    #[command(about = "Merge multiple selected dimensions together into a unified target")]
    Converge(ConvergeArgs),

    #[command(about = "Propagate changes through a chain of dependent dimensions")]
    Cascade(CascadeArgs),

    #[command(about = "Interleave commits from two dimensions in chronological order")]
    Weave(WeaveArgs),

    #[command(about = "Transplant a slice of commits from one dimension to another")]
    Splice(SpliceArgs),

    #[command(about = "Manage live entanglement sync rules between dimensions")]
    Entangle(EntangleArgs),

    #[command(about = "Manage the Cronos autonomous background synchronization daemon")]
    Cronos(CronosArgs),

    #[command(about = "Manage multi-agent identities, assignments, mailboxes, and heartbeats")]
    Agent(AgentArgs),

    #[command(about = "Report agent heartbeat to record liveness")]
    Heartbeat(HeartbeatArgs),

    #[command(about = "Visualize and export the multiverse timeline graph")]
    Timeline(TimelineArgs),

    #[command(about = "Manage tracked remote repositories")]
    Remote(RemoteArgs),

    #[command(about = "Download objects and refs from another repository")]
    Fetch(FetchArgs),

    #[command(about = "Fetch from and integrate with another repository or branch")]
    Pull(PullArgs),

    #[command(about = "Update remote refs along with associated objects")]
    Push(PushArgs),

    #[command(about = "Move objects and refs by archive")]
    Bundle(BundleArgs),

    #[command(about = "Export Daft dimensions or repositories to external formats")]
    Export(ExportArgs),

    #[command(about = "Import external repositories into Daft")]
    Import(ImportArgs),

    #[command(about = "Manage backward compatibility and bridge daemons")]
    Compat(CompatArgs),

    #[command(about = "Launch the interactive real-time Daft Multiverse Web GUI")]
    Ui(UiArgs),
}

// ----------------------------------------------------------------------------
// Args Definitions
// ----------------------------------------------------------------------------

#[derive(Args, Debug)]
pub struct InitArgs {
    #[arg(help = "Directory where the repository will be created")]
    pub directory: Option<PathBuf>,

    #[arg(long, help = "Create a bare repository without a working directory")]
    pub bare: bool,

    #[arg(
        long = "initial-branch",
        short = 'b',
        default_value = "main",
        help = "Use the specified name for the initial branch"
    )]
    pub initial_branch: String,
}

#[derive(Args, Debug)]
pub struct CloneArgs {
    #[arg(required = true, help = "The repository to clone from")]
    pub repository: String,

    #[arg(help = "The name of a new directory to clone into")]
    pub directory: Option<PathBuf>,
}

#[derive(Args, Debug)]
pub struct ConfigArgs {
    #[arg(long, help = "Get the value for a given key")]
    pub get: Option<String>,

    #[arg(long, num_args = 2, value_names = ["KEY", "VALUE"], help = "Set the value for a given key")]
    pub set: Option<Vec<String>>,

    #[arg(long, help = "List all variables set in config file")]
    pub list: bool,

    #[arg(long, help = "Remove a variable from config file")]
    pub unset: Option<String>,
}

#[derive(Args, Debug)]
pub struct AddArgs {
    #[arg(help = "Files or directories to stage (use '.' for all in current directory)")]
    pub pathspecs: Vec<PathBuf>,

    #[arg(
        short = 'A',
        long = "all",
        help = "Stage all tracked and untracked changes"
    )]
    pub all: bool,

    #[arg(
        short = 'u',
        long = "update",
        help = "Stage only modified and deleted tracked files"
    )]
    pub update: bool,
}

#[derive(Args, Debug)]
pub struct StatusArgs {
    #[arg(short = 's', long = "short", help = "Give output in short format")]
    pub short: bool,
}

#[derive(Args, Debug)]
pub struct CommitArgs {
    #[arg(short = 'm', long = "message", help = "Commit message")]
    pub message: Option<String>,

    #[arg(
        short = 'a',
        long = "all",
        help = "Automatically stage files that have been modified and deleted"
    )]
    pub all: bool,

    #[arg(
        long = "allow-empty",
        help = "Record commit even when index has no staged changes"
    )]
    pub allow_empty: bool,

    #[arg(
        long = "author",
        help = "Override the commit author (format: 'Name <email>')"
    )]
    pub author: Option<String>,
}

#[derive(Args, Debug)]
pub struct ResetArgs {
    #[arg(default_value = "HEAD", help = "Commit to reset to")]
    pub target: String,

    #[arg(
        long = "soft",
        help = "Do not touch the index file or the working tree at all"
    )]
    pub soft: bool,

    #[arg(
        long = "mixed",
        help = "Resets the index but not the working tree (default)"
    )]
    pub mixed: bool,

    #[arg(long = "hard", help = "Resets the index and working tree")]
    pub hard: bool,
}

#[derive(Args, Debug)]
pub struct RestoreArgs {
    #[arg(required = true, help = "Files to restore")]
    pub paths: Vec<PathBuf>,

    #[arg(long = "staged", short = 'S', help = "Restore the index")]
    pub staged: bool,

    #[arg(long = "worktree", short = 'W', help = "Restore the working tree")]
    pub worktree: bool,

    #[arg(
        short = 's',
        long = "source",
        help = "Restore the working tree files with the content from the given tree"
    )]
    pub source: Option<String>,
}

#[derive(Args, Debug)]
pub struct RmArgs {
    #[arg(required = true, help = "Files to remove")]
    pub paths: Vec<PathBuf>,

    #[arg(short = 'f', long = "force", help = "Override the up-to-date check")]
    pub force: bool,

    #[arg(
        short = 'r',
        help = "Allow recursive removal when a leading directory name is given"
    )]
    pub recursive: bool,

    #[arg(long = "cached", help = "Unstage and remove paths only from the index")]
    pub cached: bool,
}

#[derive(Args, Debug)]
pub struct MvArgs {
    #[arg(required = true, help = "Source file/directory")]
    pub source: PathBuf,

    #[arg(required = true, help = "Destination file/directory")]
    pub destination: PathBuf,

    #[arg(
        short = 'f',
        long = "force",
        help = "Force move/rename even if target exists"
    )]
    pub force: bool,
}

#[derive(Args, Debug)]
pub struct CleanArgs {
    #[arg(
        short = 'f',
        long = "force",
        help = "Required to actually delete files"
    )]
    pub force: bool,

    #[arg(
        short = 'n',
        long = "dry-run",
        help = "Don't actually remove anything, just show what would be done"
    )]
    pub dry_run: bool,

    #[arg(
        short = 'd',
        help = "Remove untracked directories in addition to untracked files"
    )]
    pub directories: bool,
}

#[derive(Args, Debug)]
pub struct StashArgs {
    #[command(subcommand)]
    pub action: Option<StashAction>,

    #[arg(help = "Optional message for stash push")]
    pub message: Option<String>,
}

#[derive(Subcommand, Debug)]
pub enum StashAction {
    #[command(about = "Save your local modifications to a new stash")]
    Push {
        #[arg(short = 'm', long = "message", help = "Stash message")]
        message: Option<String>,
    },
    #[command(
        about = "Remove a single stashed state from the stash list and apply it on top of the current working tree"
    )]
    Pop {
        #[arg(default_value = "0", help = "Index of stash to pop")]
        index: usize,
    },
    #[command(about = "Apply a stashed state without dropping it")]
    Apply {
        #[arg(default_value = "0", help = "Index of stash to apply")]
        index: usize,
    },
    #[command(about = "List the stashes that you currently have")]
    List,
    #[command(about = "Remove a single stashed state from the stash list")]
    Drop {
        #[arg(default_value = "0", help = "Index of stash to drop")]
        index: usize,
    },
    #[command(about = "Remove all the stashed states")]
    Clear,
}

#[derive(Args, Debug)]
pub struct BranchArgs {
    #[arg(help = "Branch name to create")]
    pub name: Option<String>,

    #[arg(short = 'd', long = "delete", help = "Delete a branch")]
    pub delete: Option<String>,

    #[arg(short = 'D', help = "Force delete a branch")]
    pub force_delete: Option<String>,

    #[arg(short = 'm', long = "move", num_args = 2, value_names = ["OLD", "NEW"], help = "Move/rename a branch")]
    pub rename: Option<Vec<String>>,

    #[arg(
        short = 'a',
        long = "all",
        help = "List both remote and local branches"
    )]
    pub all: bool,
}

#[derive(Args, Debug)]
pub struct CheckoutArgs {
    #[arg(help = "Branch or revision to checkout")]
    pub target: Option<String>,

    #[arg(short = 'b', help = "Create and checkout a new branch")]
    pub create_branch: Option<String>,

    #[arg(last = true, help = "Paths to restore from tree")]
    pub paths: Vec<PathBuf>,
}

#[derive(Args, Debug)]
pub struct SwitchArgs {
    #[arg(help = "Target branch name")]
    pub branch: Option<String>,

    #[arg(
        short = 'c',
        long = "create",
        help = "Create and switch to a new branch"
    )]
    pub create: Option<String>,

    #[arg(
        short = 'd',
        long = "detach",
        help = "Switch to commit in detached HEAD state"
    )]
    pub detach: bool,
}

#[derive(Args, Debug)]
pub struct MergeArgs {
    #[arg(help = "Branch or commit to merge into current branch")]
    pub target: Option<String>,

    #[arg(
        short = 'm',
        long = "message",
        help = "Commit message for merge commit"
    )]
    pub message: Option<String>,

    #[arg(long = "abort", help = "Abort the current conflict resolution process")]
    pub abort: bool,

    #[arg(
        long = "strategy",
        default_value = "recursive",
        help = "Merge strategy ('recursive', 'ours', 'theirs')"
    )]
    pub strategy: String,
}

#[derive(Args, Debug)]
pub struct RebaseArgs {
    #[arg(required = true, help = "Upstream branch to compare against")]
    pub upstream: String,

    #[arg(help = "Working branch to rebase")]
    pub branch: Option<String>,
}

#[derive(Args, Debug)]
pub struct CherryPickArgs {
    #[arg(required = true, help = "Commit to cherry-pick")]
    pub commit: String,
}

#[derive(Args, Debug)]
pub struct TagArgs {
    #[arg(help = "Tag name to create")]
    pub name: Option<String>,

    #[arg(help = "Target commit (defaults to HEAD)")]
    pub target: Option<String>,

    #[arg(
        short = 'a',
        long = "annotate",
        help = "Make an unsigned, annotated tag object"
    )]
    pub annotate: bool,

    #[arg(short = 'm', long = "message", help = "Tag message")]
    pub message: Option<String>,

    #[arg(short = 'd', long = "delete", help = "Delete a tag")]
    pub delete: Option<String>,

    #[arg(short = 'l', long = "list", help = "List tags")]
    pub list: bool,
}

#[derive(Args, Debug)]
pub struct LogArgs {
    #[arg(help = "Revision or range to inspect (e.g. 'HEAD', 'main', 'A..B')")]
    pub revision_range: Option<String>,

    #[arg(
        long = "oneline",
        help = "Shorthand for '--pretty=oneline --abbrev-commit'"
    )]
    pub oneline: bool,

    #[arg(
        short = 'n',
        long = "max-count",
        help = "Limit the number of commits to output"
    )]
    pub max_count: Option<usize>,

    #[arg(
        long = "graph",
        help = "Draw a graphical representation of the commit history"
    )]
    pub graph: bool,
}

#[derive(Args, Debug)]
pub struct DiffArgs {
    #[arg(help = "Revisions to compare (e.g. 'HEAD~1 HEAD')")]
    pub revisions: Vec<String>,

    #[arg(
        long = "staged",
        short = 's',
        alias = "cached",
        help = "View changes staged for next commit"
    )]
    pub staged: bool,

    #[arg(long = "stat", help = "Generate a diffstat")]
    pub stat: bool,
}

#[derive(Args, Debug)]
pub struct ShowArgs {
    #[arg(help = "Object to show (defaults to HEAD)")]
    pub object: Option<String>,
}

#[derive(Args, Debug)]
pub struct BlameArgs {
    #[arg(required = true, help = "Path to file to blame")]
    pub file: PathBuf,

    #[arg(short = 'L', help = "Annotate only the given line range <start>,<end>")]
    pub line_range: Option<String>,
}

#[derive(Args, Debug)]
pub struct ShortlogArgs {
    #[arg(
        short = 's',
        long = "summary",
        help = "Suppress commit descriptions and provide a commit count summary only"
    )]
    pub summary: bool,

    #[arg(
        short = 'n',
        long = "numbered",
        help = "Sort output according to the number of commits per author"
    )]
    pub numbered: bool,
}

#[derive(Args, Debug)]
pub struct DescribeArgs {
    #[arg(help = "Commit expression to describe (defaults to HEAD)")]
    pub commit: Option<String>,
}

#[derive(Args, Debug)]
pub struct GrepArgs {
    #[arg(required = true, help = "Search pattern")]
    pub pattern: String,

    #[arg(help = "Path limits")]
    pub pathspecs: Vec<PathBuf>,

    #[arg(
        short = 'n',
        long = "line-number",
        help = "Prefix the line number to matching lines"
    )]
    pub line_number: bool,

    #[arg(
        short = 'i',
        long = "ignore-case",
        help = "Ignore case differences between pattern and files"
    )]
    pub ignore_case: bool,
}

#[derive(Args, Debug)]
pub struct BisectArgs {
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub args: Vec<String>,
}

#[derive(Args, Debug)]
pub struct ReflogArgs {
    #[arg(help = "Reference name (defaults to HEAD)")]
    pub ref_name: Option<String>,

    #[arg(help = "Number of entries to show")]
    pub count: Option<usize>,
}

// ----------------------------------------------------------------------------
// Plumbing Args
// ----------------------------------------------------------------------------

#[derive(Args, Debug)]
pub struct HashObjectArgs {
    #[arg(required = true, help = "File to hash")]
    pub file: PathBuf,

    #[arg(
        short = 'w',
        help = "Actually write the object into the object database"
    )]
    pub write: bool,

    #[arg(
        short = 't',
        default_value = "blob",
        help = "Object type ('blob', 'tree', 'commit', 'tag')"
    )]
    pub object_type: String,
}

#[derive(Args, Debug)]
pub struct CatFileArgs {
    #[arg(
        short = 'p',
        help = "Pretty-print the contents of <object> based on its type"
    )]
    pub pretty: bool,

    #[arg(short = 't', help = "Show the object type")]
    pub show_type: bool,

    #[arg(short = 's', help = "Show the object size")]
    pub show_size: bool,

    #[arg(required = true, help = "The name of the object to show")]
    pub object: String,
}

#[derive(Args, Debug)]
pub struct WriteTreeArgs {}

#[derive(Args, Debug)]
pub struct CommitTreeArgs {
    #[arg(required = true, help = "Tree object ID")]
    pub tree: String,

    #[arg(short = 'm', required = true, help = "Commit message")]
    pub message: String,

    #[arg(short = 'p', help = "Parent commit ID(s)")]
    pub parents: Vec<String>,
}

#[derive(Args, Debug)]
pub struct LsTreeArgs {
    #[arg(required = true, help = "Tree object ID")]
    pub tree: String,

    #[arg(short = 'r', help = "Recurse into sub-trees")]
    pub recursive: bool,
}

#[derive(Args, Debug)]
pub struct LsFilesArgs {
    #[arg(
        short = 's',
        long = "stage",
        help = "Show staged contents' mode bits, object name and stage number"
    )]
    pub stage: bool,

    #[arg(
        short = 'u',
        long = "unmerged",
        help = "Show unmerged files in the output"
    )]
    pub unmerged: bool,

    #[arg(
        short = 'o',
        long = "others",
        help = "Show other (untracked) files in the output"
    )]
    pub others: bool,
}

#[derive(Args, Debug)]
pub struct RevParseArgs {
    #[arg(help = "Revision specification to parse")]
    pub rev: Option<String>,

    #[arg(
        long = "show-toplevel",
        help = "Show the absolute path of the top-level working tree"
    )]
    pub show_toplevel: bool,

    #[arg(long = "is-inside-work-tree", help = "Check if inside a working tree")]
    pub is_inside_work_tree: bool,
}

#[derive(Args, Debug)]
pub struct UpdateRefArgs {
    #[arg(required = true, help = "Reference name")]
    pub ref_name: String,

    #[arg(required = true, help = "New commit OID")]
    pub new_oid: String,

    #[arg(help = "Old commit OID (optional CAS check)")]
    pub old_oid: Option<String>,
}

#[derive(Args, Debug)]
pub struct SymbolicRefArgs {
    #[arg(required = true, help = "Symbolic reference name")]
    pub name: String,

    #[arg(help = "Target reference to point to")]
    pub target: Option<String>,
}

// ----------------------------------------------------------------------------
// Maintenance Args
// ----------------------------------------------------------------------------

#[derive(Args, Debug)]
pub struct GcArgs {
    #[arg(long = "prune", help = "Prune loose objects")]
    pub prune: bool,
}

#[derive(Args, Debug)]
pub struct FsckArgs {
    #[arg(long = "full", help = "Check all objects, not just unreachable ones")]
    pub full: bool,
}

#[derive(Args, Debug)]
pub struct PruneArgs {}

#[derive(Args, Debug)]
pub struct RepackArgs {}

#[derive(Args, Debug)]
pub struct CountObjectsArgs {}

// ----------------------------------------------------------------------------
// Layer 2 Args
// ----------------------------------------------------------------------------

#[derive(Args, Debug)]
pub struct DimensionArgs {
    #[command(subcommand)]
    pub action: DimensionAction,
}

#[derive(Subcommand, Debug)]
pub enum DimensionAction {
    #[command(about = "Create a new parallel dimension workspace")]
    Create {
        #[arg(required = true, help = "Name of dimension")]
        name: String,
        #[arg(long = "from", help = "Source branch, tag, or commit OID")]
        from: Option<String>,
    },
    #[command(about = "List all active dimensions")]
    List,
    #[command(
        about = "Switch active context to specified dimension",
        alias = "switch"
    )]
    Enter {
        #[arg(required = true, help = "Name of dimension to enter")]
        name: String,
    },
    #[command(about = "Destroy dimension workspace", alias = "delete")]
    Destroy {
        #[arg(required = true, help = "Name of dimension to destroy")]
        name: String,
        #[arg(
            short = 'f',
            long = "force",
            help = "Force destruction of active or dirty dimension"
        )]
        force: bool,
    },
    #[command(about = "Fork live uncommitted state into new dimension")]
    Fork {
        #[arg(required = true, help = "New dimension name")]
        name: String,
        #[arg(long = "from", required = true, help = "Source dimension name")]
        from: String,
    },
    #[command(about = "Take point-in-time snapshot of dimension")]
    Snapshot {
        #[arg(long = "all", help = "Snapshot all dimensions")]
        all: bool,
        #[arg(long = "name", help = "Dimension to snapshot")]
        name: Option<String>,
        #[arg(short = 'm', long = "message", help = "Annotation description")]
        message: Option<String>,
    },
    #[command(about = "Rename an existing dimension")]
    Rename {
        #[arg(required = true, help = "Old dimension name")]
        old_name: String,
        #[arg(required = true, help = "New dimension name")]
        new_name: String,
    },
    #[command(about = "Display detailed dimension metadata and resource usage")]
    Info {
        #[arg(required = true, help = "Dimension name")]
        name: String,
    },
}

#[derive(Args, Debug)]
pub struct SnapshotArgs {
    #[command(subcommand)]
    pub action: SnapshotAction,
}

#[derive(Subcommand, Debug)]
pub enum SnapshotAction {
    #[command(about = "Create a new snapshot of dimension state")]
    Create {
        #[arg(required = true, help = "Name of the snapshot")]
        name: String,
        #[arg(
            short = 'd',
            long = "dimension",
            help = "Target dimension (defaults to active)"
        )]
        dimension: Option<String>,
        #[arg(short = 'm', long = "message", help = "Description / annotation")]
        message: Option<String>,
    },
    #[command(about = "List snapshots")]
    List {
        #[arg(
            short = 'd',
            long = "dimension",
            help = "Filter by dimension (shows all if omitted)"
        )]
        dimension: Option<String>,
        #[arg(
            long = "porcelain",
            help = "Output machine-readable tab-separated values"
        )]
        porcelain: bool,
    },
    #[command(about = "Restore dimension workspace to snapshot state")]
    Restore {
        #[arg(required = true, help = "Name or ID of snapshot to restore")]
        name: String,
        #[arg(
            short = 'd',
            long = "dimension",
            help = "Target dimension (defaults to active)"
        )]
        dimension: Option<String>,
        #[arg(
            short = 'f',
            long = "force",
            help = "Force restore even if working tree is dirty"
        )]
        force: bool,
    },
    #[command(about = "Delete a snapshot")]
    Delete {
        #[arg(required = true, help = "Name or ID of snapshot to delete")]
        name: String,
        #[arg(
            short = 'd',
            long = "dimension",
            help = "Target dimension (defaults to active)"
        )]
        dimension: Option<String>,
    },
}

#[derive(Args, Debug)]
pub struct ObserveArgs {
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub args: Vec<String>,
}

#[derive(Args, Debug)]
pub struct RadarArgs {
    #[arg(help = "Specific path to inspect")]
    pub path: Option<String>,

    #[arg(long = "hot", help = "Show concurrently modified hot zones")]
    pub hot: bool,
}

#[derive(Args, Debug)]
pub struct ForeseeArgs {
    #[arg(help = "First dimension")]
    pub dim1: Option<String>,

    #[arg(help = "Second dimension")]
    pub dim2: Option<String>,
}

#[derive(Args, Debug)]
pub struct OverlapArgs {}

#[derive(Args, Debug)]
pub struct EntropyArgs {
    #[arg(required = true, help = "First dimension")]
    pub dim1: String,

    #[arg(required = true, help = "Second dimension")]
    pub dim2: String,
}

#[derive(Args, Debug)]
pub struct ClaimArgs {
    #[arg(required = true, help = "Path to claim exclusive ownership")]
    pub path: String,

    #[arg(long = "dimension", help = "Claiming dimension")]
    pub dimension: Option<String>,
}

#[derive(Args, Debug)]
pub struct YieldArgs {
    #[arg(required = true, help = "Path to release ownership")]
    pub path: String,
}

#[derive(Args, Debug)]
pub struct FenceArgs {
    #[arg(required = true, help = "Path to place a barrier on")]
    pub path: String,

    #[arg(long = "hard", help = "Hard barrier blocking edits")]
    pub hard: bool,
}

#[derive(Args, Debug)]
pub struct TerritoryArgs {
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub args: Vec<String>,
}

#[derive(Args, Debug)]
pub struct CollapseArgs {
    #[arg(long = "into", help = "Target dimension to collapse into")]
    pub into: Option<String>,

    #[arg(long = "strategy", help = "Merge strategy")]
    pub strategy: Option<String>,
}

#[derive(Args, Debug)]
pub struct ConvergeArgs {
    #[arg(required = true, help = "Dimensions to converge")]
    pub dimensions: Vec<String>,

    #[arg(long = "into", help = "Target dimension name")]
    pub into: Option<String>,
}

#[derive(Args, Debug)]
pub struct CascadeArgs {
    #[arg(required = true, help = "Source dimension")]
    pub dimension: String,

    #[arg(long = "to", help = "Target dimension")]
    pub to: Option<String>,
}

#[derive(Args, Debug)]
pub struct WeaveArgs {
    #[arg(required = true, help = "First dimension")]
    pub dim1: String,

    #[arg(required = true, help = "Second dimension")]
    pub dim2: String,
}

#[derive(Args, Debug)]
pub struct SpliceArgs {
    #[arg(required = true, help = "Source dimension")]
    pub dimension: String,

    #[arg(required = true, help = "Commit range")]
    pub range: String,

    #[arg(long = "into", required = true, help = "Target dimension")]
    pub into: String,
}

#[derive(Args, Debug)]
pub struct EntangleArgs {
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub args: Vec<String>,

    #[arg(long = "paths", help = "Path pattern filter")]
    pub paths: Option<String>,
}

#[derive(Args, Debug)]
pub struct CronosArgs {
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub args: Vec<String>,
}

#[derive(Args, Debug)]
pub struct AgentArgs {
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub args: Vec<String>,

    #[arg(long = "type", help = "Agent type: 'ai' or 'human'")]
    pub agent_type: Option<String>,
}

#[derive(Args, Debug)]
pub struct HeartbeatArgs {
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub args: Vec<String>,
}

#[derive(Args, Debug)]
pub struct TimelineArgs {
    #[arg(help = "Dimension name")]
    pub dimension: Option<String>,

    #[arg(long = "ancestry", help = "Show ancestry links")]
    pub ancestry: bool,

    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub args: Vec<String>,

    #[arg(long = "format", help = "Export format: dot, json, svg")]
    pub format: Option<String>,
}

#[derive(Args, Debug)]
pub struct RemoteArgs {
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub args: Vec<String>,

    #[arg(short = 'v', long = "verbose", help = "Be more verbose")]
    pub verbose: bool,
}

#[derive(Args, Debug)]
pub struct FetchArgs {
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub args: Vec<String>,
}

#[derive(Args, Debug)]
pub struct PullArgs {
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub args: Vec<String>,
}

#[derive(Args, Debug)]
pub struct PushArgs {
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub args: Vec<String>,
}

#[derive(Args, Debug)]
pub struct BundleArgs {
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub args: Vec<String>,
}

#[derive(Args, Debug)]
pub struct ExportArgs {
    #[arg(required = true, help = "Target format, e.g. 'git'")]
    pub format: String,

    #[arg(long = "dimension", help = "Dimension to export")]
    pub dimension: Option<String>,
}

#[derive(Args, Debug)]
pub struct ImportArgs {
    #[arg(required = true, help = "Source format, e.g. 'git'")]
    pub format: String,
}

#[derive(Args, Debug)]
pub struct CompatArgs {
    #[arg(required = true, help = "Bridge type, e.g. 'git-bridge'")]
    pub bridge: String,
}

#[derive(Args, Debug)]
pub struct UiArgs {
    #[arg(short = 'p', long = "port", default_value = "3333", env = "DFT_PORT", help = "Port to listen on")]
    pub port: u16,

    #[arg(long = "host", default_value = "127.0.0.1", env = "DFT_HOST", help = "Host/IP address to bind to")]
    pub host: String,

    #[arg(long = "no-browser", help = "Do not automatically open the browser")]
    pub no_browser: bool,
}
