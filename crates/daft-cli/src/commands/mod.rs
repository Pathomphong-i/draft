pub mod add;
pub mod bisect;
pub mod blame;
pub mod branch;
pub mod checkout;
pub mod cherry_pick;
pub mod clean;
pub mod clone;
pub mod commit;
pub mod config;
pub mod describe;
pub mod diff;
pub mod grep;
pub mod init;
pub mod layer2;
pub mod log;
pub mod maintenance;
pub mod merge;
pub mod mv;
pub mod plumbing;
pub mod rebase;
pub mod reflog;
pub mod reset;
pub mod restore;
pub mod rm;
pub mod shortlog;
pub mod show;
pub mod stash;
pub mod status;
pub mod switch;
pub mod tag;
pub mod ui;

use crate::cli::{Cli, Commands};
use crate::error::CliError;

pub fn dispatch(cli: Cli) -> Result<(), CliError> {
    match cli.command {
        Commands::Init(args) => init::execute(args, cli.quiet),
        Commands::Clone(args) => clone::execute(args),
        Commands::Config(args) => config::execute(args),
        Commands::Add(args) => add::execute(args),
        Commands::Status(args) => status::execute(args, cli.json),
        Commands::Commit(args) => commit::execute(args, cli.quiet),
        Commands::Reset(args) => reset::execute(args),
        Commands::Restore(args) => restore::execute(args),
        Commands::Rm(args) => rm::execute(args),
        Commands::Mv(args) => mv::execute(args),
        Commands::Clean(args) => clean::execute(args),
        Commands::Stash(args) => stash::execute(args),
        Commands::Branch(args) => branch::execute(args, cli.json),
        Commands::Checkout(args) => checkout::execute(args),
        Commands::Switch(args) => switch::execute(args),
        Commands::Merge(args) => merge::execute(args),
        Commands::Rebase(args) => rebase::execute(args),
        Commands::CherryPick(args) => cherry_pick::execute(args),
        Commands::Tag(args) => tag::execute(args, cli.json),
        Commands::Log(args) => log::execute(args, cli.json),
        Commands::Diff(args) => diff::execute(args, cli.json),
        Commands::Show(args) => show::execute(args),
        Commands::Blame(args) => blame::execute(args),
        Commands::Shortlog(args) => shortlog::execute(args),
        Commands::Describe(args) => describe::execute(args),
        Commands::Grep(args) => grep::execute(args),
        Commands::Bisect(args) => bisect::execute(args),
        Commands::Reflog(args) => reflog::execute(args),
        Commands::HashObject(args) => plumbing::execute_hash_object(args),
        Commands::CatFile(args) => plumbing::execute_cat_file(args),
        Commands::WriteTree(args) => plumbing::execute_write_tree(args),
        Commands::CommitTree(args) => plumbing::execute_commit_tree(args),
        Commands::LsTree(args) => plumbing::execute_ls_tree(args),
        Commands::LsFiles(args) => plumbing::execute_ls_files(args),
        Commands::RevParse(args) => plumbing::execute_rev_parse(args),
        Commands::UpdateRef(args) => plumbing::execute_update_ref(args),
        Commands::SymbolicRef(args) => plumbing::execute_symbolic_ref(args),
        Commands::Gc(args) => maintenance::execute_gc(args),
        Commands::Fsck(args) => maintenance::execute_fsck(args),
        Commands::Prune(args) => maintenance::execute_prune(args),
        Commands::Repack(args) => maintenance::execute_repack(args),
        Commands::CountObjects(args) => maintenance::execute_count_objects(args),
        Commands::Dimension(args) => layer2::dimension::execute(args, cli.json),
        Commands::Snapshot(args) => layer2::snapshot::execute(args, cli.json),
        Commands::Observe(args) => layer2::awareness::execute_observe(args),
        Commands::Radar(args) => layer2::awareness::execute_radar(args, cli.json),
        Commands::Foresee(args) => layer2::awareness::execute_foresee(args),
        Commands::Overlap(args) => layer2::awareness::execute_overlap(args),
        Commands::Entropy(args) => layer2::awareness::execute_entropy(args),
        Commands::Claim(args) => layer2::awareness::execute_claim(args),
        Commands::Yield(args) => layer2::awareness::execute_yield(args),
        Commands::Fence(args) => layer2::awareness::execute_fence(args),
        Commands::Territory(args) => layer2::awareness::execute_territory(args, cli.json),
        Commands::Collapse(args) => layer2::convergence::execute_collapse(args),
        Commands::Converge(args) => layer2::convergence::execute_converge(args),
        Commands::Cascade(args) => layer2::convergence::execute_cascade(args),
        Commands::Weave(args) => layer2::convergence::execute_weave(args),
        Commands::Splice(args) => layer2::convergence::execute_splice(args),
        Commands::Entangle(args) => layer2::sync::execute_entangle(args),
        Commands::Cronos(args) => layer2::sync::execute_cronos(args),
        Commands::Agent(args) => layer2::agent::execute(args, cli.json),
        Commands::Heartbeat(args) => {
            layer2::agent::execute_heartbeat(args.args.first().map(|s| s.as_str()), cli.json)
        }
        Commands::Timeline(args) => layer2::timeline::execute(args),
        Commands::Remote(args) => layer2::remote::execute_remote(args),
        Commands::Fetch(args) => layer2::remote::execute_fetch(args),
        Commands::Pull(args) => layer2::remote::execute_pull(args),
        Commands::Push(args) => layer2::remote::execute_push(args),
        Commands::Bundle(args) => layer2::remote::execute_bundle(args),
        Commands::Export(args) => layer2::remote::execute_export(args),
        Commands::Import(args) => layer2::remote::execute_import(args),
        Commands::Compat(args) => layer2::remote::execute_compat(args),
        Commands::Ui(args) => ui::execute(args),
    }
}
