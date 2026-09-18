//! Tier 1 Test Suite Registry and Runner.

pub mod agent_timeline;
pub mod awareness;
pub mod convergence;
pub mod core_layer1;
pub mod dimensions;
pub mod git_remote;
pub mod sync_daemon;

pub struct TestCase {
    pub name: &'static str,
    pub func: fn(),
}

pub fn get_all_tests() -> Vec<TestCase> {
    vec![
        // Core & Layer 1
        TestCase {
            name: "t1::init_creates_dft_directory",
            func: core_layer1::test_init_creates_dft_directory,
        },
        TestCase {
            name: "t1::init_custom_default_branch",
            func: core_layer1::test_init_custom_default_branch,
        },
        TestCase {
            name: "t1::init_in_existing_directory_preserves_files",
            func: core_layer1::test_init_in_existing_directory_preserves_files,
        },
        TestCase {
            name: "t1::init_reinit_is_idempotent",
            func: core_layer1::test_init_reinit_is_idempotent,
        },
        TestCase {
            name: "t1::init_bare_repo",
            func: core_layer1::test_init_bare_repo,
        },
        TestCase {
            name: "t1::add_single_file",
            func: core_layer1::test_add_single_file,
        },
        TestCase {
            name: "t1::add_multiple_files",
            func: core_layer1::test_add_multiple_files,
        },
        TestCase {
            name: "t1::add_directory_recursive",
            func: core_layer1::test_add_directory_recursive,
        },
        TestCase {
            name: "t1::add_current_dir_dot",
            func: core_layer1::test_add_current_dir_dot,
        },
        TestCase {
            name: "t1::add_updated_file_updates_index",
            func: core_layer1::test_add_updated_file_updates_index,
        },
        TestCase {
            name: "t1::status_clean_worktree",
            func: core_layer1::test_status_clean_worktree,
        },
        TestCase {
            name: "t1::status_untracked_files",
            func: core_layer1::test_status_untracked_files,
        },
        TestCase {
            name: "t1::status_staged_changes",
            func: core_layer1::test_status_staged_changes,
        },
        TestCase {
            name: "t1::status_unstaged_modifications",
            func: core_layer1::test_status_unstaged_modifications,
        },
        TestCase {
            name: "t1::status_deleted_file",
            func: core_layer1::test_status_deleted_file,
        },
        TestCase {
            name: "t1::commit_basic",
            func: core_layer1::test_commit_basic,
        },
        TestCase {
            name: "t1::commit_custom_author",
            func: core_layer1::test_commit_custom_author,
        },
        TestCase {
            name: "t1::commit_empty_index_fails_without_flag",
            func: core_layer1::test_commit_empty_index_fails_without_flag,
        },
        TestCase {
            name: "t1::commit_allow_empty",
            func: core_layer1::test_commit_allow_empty,
        },
        TestCase {
            name: "t1::commit_sequence_advances_head",
            func: core_layer1::test_commit_sequence_advances_head,
        },
        TestCase {
            name: "t1::branch_create_and_list",
            func: core_layer1::test_branch_create_and_list,
        },
        TestCase {
            name: "t1::branch_delete_safe",
            func: core_layer1::test_branch_delete_safe,
        },
        TestCase {
            name: "t1::branch_rename",
            func: core_layer1::test_branch_rename,
        },
        TestCase {
            name: "t1::switch_creates_and_switches_with_c",
            func: core_layer1::test_switch_creates_and_switches_with_c,
        },
        TestCase {
            name: "t1::checkout_detached_head",
            func: core_layer1::test_checkout_detached_head,
        },
        TestCase {
            name: "t1::merge_fast_forward",
            func: core_layer1::test_merge_fast_forward,
        },
        TestCase {
            name: "t1::merge_three_way_clean",
            func: core_layer1::test_merge_three_way_clean,
        },
        TestCase {
            name: "t1::merge_conflict_detection",
            func: core_layer1::test_merge_conflict_detection,
        },
        TestCase {
            name: "t1::merge_abort",
            func: core_layer1::test_merge_abort,
        },
        TestCase {
            name: "t1::merge_preserves_two_parents",
            func: core_layer1::test_merge_preserves_two_parents,
        },
        TestCase {
            name: "t1::diff_working_tree",
            func: core_layer1::test_diff_working_tree,
        },
        TestCase {
            name: "t1::diff_staged",
            func: core_layer1::test_diff_staged,
        },
        TestCase {
            name: "t1::diff_commit_range",
            func: core_layer1::test_diff_commit_range,
        },
        TestCase {
            name: "t1::diff_no_changes_empty",
            func: core_layer1::test_diff_no_changes_empty,
        },
        TestCase {
            name: "t1::diff_binary_detection",
            func: core_layer1::test_diff_binary_detection,
        },
        TestCase {
            name: "t1::log_oneline_and_limit",
            func: core_layer1::test_log_oneline_and_limit,
        },
        TestCase {
            name: "t1::blame_line_attribution",
            func: core_layer1::test_blame_line_attribution,
        },
        TestCase {
            name: "t1::stash_push_and_pop",
            func: core_layer1::test_stash_push_and_pop,
        },
        TestCase {
            name: "t1::reset_soft_mixed_hard",
            func: core_layer1::test_reset_soft_mixed_hard,
        },
        TestCase {
            name: "t1::tag_lightweight_and_annotated",
            func: core_layer1::test_tag_lightweight_and_annotated,
        },
        TestCase {
            name: "t1::plumbing_hash_object_and_cat_file",
            func: core_layer1::test_plumbing_hash_object_and_cat_file,
        },
        TestCase {
            name: "t1::maintenance_gc_and_fsck",
            func: core_layer1::test_maintenance_gc_and_fsck,
        },
        // Parallel Dimensions
        TestCase {
            name: "t1::dimension_create_basic",
            func: dimensions::test_dimension_create_basic,
        },
        TestCase {
            name: "t1::dimension_create_from_ref",
            func: dimensions::test_dimension_create_from_ref,
        },
        TestCase {
            name: "t1::dimension_create_duplicate_fails",
            func: dimensions::test_dimension_create_duplicate_fails,
        },
        TestCase {
            name: "t1::dimension_create_initializes_isolated_index_and_head",
            func: dimensions::test_dimension_create_initializes_isolated_index_and_head,
        },
        TestCase {
            name: "t1::dimension_create_generates_metadata",
            func: dimensions::test_dimension_create_generates_metadata,
        },
        TestCase {
            name: "t1::dimension_list_shows_mainline_and_created",
            func: dimensions::test_dimension_list_shows_mainline_and_created,
        },
        TestCase {
            name: "t1::dimension_list_shows_branch_and_status",
            func: dimensions::test_dimension_list_shows_branch_and_status,
        },
        TestCase {
            name: "t1::dimension_list_json_format",
            func: dimensions::test_dimension_list_json_format,
        },
        TestCase {
            name: "t1::dimension_list_reflects_active_marker",
            func: dimensions::test_dimension_list_reflects_active_marker,
        },
        TestCase {
            name: "t1::dimension_list_shows_resource_usage",
            func: dimensions::test_dimension_list_shows_resource_usage,
        },
        TestCase {
            name: "t1::dimension_enter_updates_current_dimension",
            func: dimensions::test_dimension_enter_updates_current_dimension,
        },
        TestCase {
            name: "t1::dimension_enter_subsequent_commands_operate_in_dimension",
            func: dimensions::test_dimension_enter_subsequent_commands_operate_in_dimension,
        },
        TestCase {
            name: "t1::dimension_enter_nonexistent_fails",
            func: dimensions::test_dimension_enter_nonexistent_fails,
        },
        TestCase {
            name: "t1::dimension_enter_same_is_idempotent",
            func: dimensions::test_dimension_enter_same_is_idempotent,
        },
        TestCase {
            name: "t1::dimension_enter_with_status_verification",
            func: dimensions::test_dimension_enter_with_status_verification,
        },
        TestCase {
            name: "t1::dimension_destroy_reclaims_workspace",
            func: dimensions::test_dimension_destroy_reclaims_workspace,
        },
        TestCase {
            name: "t1::dimension_destroy_preserves_shared_cas_objects",
            func: dimensions::test_dimension_destroy_preserves_shared_cas_objects,
        },
        TestCase {
            name: "t1::dimension_destroy_active_requires_force_or_fails",
            func: dimensions::test_dimension_destroy_active_requires_force_or_fails,
        },
        TestCase {
            name: "t1::dimension_destroy_force_on_dirty",
            func: dimensions::test_dimension_destroy_force_on_dirty,
        },
        TestCase {
            name: "t1::dimension_destroy_nonexistent_fails",
            func: dimensions::test_dimension_destroy_nonexistent_fails,
        },
        TestCase {
            name: "t1::dimension_fork_captures_uncommitted_live_state",
            func: dimensions::test_dimension_fork_captures_uncommitted_live_state,
        },
        TestCase {
            name: "t1::dimension_fork_captures_staged_index",
            func: dimensions::test_dimension_fork_captures_staged_index,
        },
        TestCase {
            name: "t1::dimension_fork_modifications_do_not_affect_source",
            func: dimensions::test_dimension_fork_modifications_do_not_affect_source,
        },
        TestCase {
            name: "t1::dimension_fork_from_nonexistent_fails",
            func: dimensions::test_dimension_fork_from_nonexistent_fails,
        },
        TestCase {
            name: "t1::dimension_fork_preserves_parent_metadata",
            func: dimensions::test_dimension_fork_preserves_parent_metadata,
        },
        TestCase {
            name: "t1::dimension_snapshot_current",
            func: dimensions::test_dimension_snapshot_current,
        },
        TestCase {
            name: "t1::dimension_snapshot_all",
            func: dimensions::test_dimension_snapshot_all,
        },
        TestCase {
            name: "t1::dimension_rename_basic",
            func: dimensions::test_dimension_rename_basic,
        },
        TestCase {
            name: "t1::dimension_info_displays_details",
            func: dimensions::test_dimension_info_displays_details,
        },
        TestCase {
            name: "t1::dimension_info_json",
            func: dimensions::test_dimension_info_json,
        },
        // Awareness, Radar, Foresee, Territory
        TestCase {
            name: "t1::observe_read_remote_file_without_switching",
            func: awareness::test_observe_read_remote_file_without_switching,
        },
        TestCase {
            name: "t1::observe_diff_between_two_dimensions",
            func: awareness::test_observe_diff_between_two_dimensions,
        },
        TestCase {
            name: "t1::observe_log_remote",
            func: awareness::test_observe_log_remote,
        },
        TestCase {
            name: "t1::observe_status_remote",
            func: awareness::test_observe_status_remote,
        },
        TestCase {
            name: "t1::observe_nonexistent_dimension_fails",
            func: awareness::test_observe_nonexistent_dimension_fails,
        },
        TestCase {
            name: "t1::radar_overview_all_dimensions",
            func: awareness::test_radar_overview_all_dimensions,
        },
        TestCase {
            name: "t1::radar_specific_path_query",
            func: awareness::test_radar_specific_path_query,
        },
        TestCase {
            name: "t1::radar_hot_zones_detects_concurrent_edits",
            func: awareness::test_radar_hot_zones_detects_concurrent_edits,
        },
        TestCase {
            name: "t1::radar_zero_hot_zones_on_disjoint_edits",
            func: awareness::test_radar_zero_hot_zones_on_disjoint_edits,
        },
        TestCase {
            name: "t1::radar_json_export",
            func: awareness::test_radar_json_export,
        },
        TestCase {
            name: "t1::foresee_predicts_conflict_without_merging",
            func: awareness::test_foresee_predicts_conflict_without_merging,
        },
        TestCase {
            name: "t1::foresee_clean_prediction_on_disjoint_changes",
            func: awareness::test_foresee_clean_prediction_on_disjoint_changes,
        },
        TestCase {
            name: "t1::overlap_reports_concurrently_modified_files",
            func: awareness::test_overlap_reports_concurrently_modified_files,
        },
        TestCase {
            name: "t1::entropy_divergence_score",
            func: awareness::test_entropy_divergence_score,
        },
        TestCase {
            name: "t1::entropy_identical_dimensions_zero_or_minimal",
            func: awareness::test_entropy_identical_dimensions_zero_or_minimal,
        },
        TestCase {
            name: "t1::territory_claim_path",
            func: awareness::test_territory_claim_path,
        },
        TestCase {
            name: "t1::territory_yield_claim",
            func: awareness::test_territory_yield_claim,
        },
        TestCase {
            name: "t1::territory_fence_hard_blocks_edits",
            func: awareness::test_territory_fence_hard_blocks_edits,
        },
        TestCase {
            name: "t1::territory_audit_detects_violations",
            func: awareness::test_territory_audit_detects_violations,
        },
        TestCase {
            name: "t1::territory_json_export",
            func: awareness::test_territory_json_export,
        },
        // Convergence Operations
        TestCase {
            name: "t1::collapse_all_dimensions_into_mainline",
            func: convergence::test_collapse_all_dimensions_into_mainline,
        },
        TestCase {
            name: "t1::collapse_into_specific_target",
            func: convergence::test_collapse_into_specific_target,
        },
        TestCase {
            name: "t1::collapse_generates_synthesis_commit",
            func: convergence::test_collapse_generates_synthesis_commit,
        },
        TestCase {
            name: "t1::collapse_conflict_handling",
            func: convergence::test_collapse_conflict_handling,
        },
        TestCase {
            name: "t1::collapse_empty_repository_noop",
            func: convergence::test_collapse_empty_repository_noop,
        },
        TestCase {
            name: "t1::converge_two_dimensions_preserves_originals",
            func: convergence::test_converge_two_dimensions_preserves_originals,
        },
        TestCase {
            name: "t1::converge_three_dimensions",
            func: convergence::test_converge_three_dimensions,
        },
        TestCase {
            name: "t1::converge_into_new_dimension_name",
            func: convergence::test_converge_into_new_dimension_name,
        },
        TestCase {
            name: "t1::converge_identical_dimensions_clean",
            func: convergence::test_converge_identical_dimensions_clean,
        },
        TestCase {
            name: "t1::converge_invalid_dimension_fails",
            func: convergence::test_converge_invalid_dimension_fails,
        },
        TestCase {
            name: "t1::cascade_sequential_propagation",
            func: convergence::test_cascade_sequential_propagation,
        },
        TestCase {
            name: "t1::cascade_targeted_endpoint",
            func: convergence::test_cascade_targeted_endpoint,
        },
        TestCase {
            name: "t1::weave_interleaves_chronological_commits",
            func: convergence::test_weave_interleaves_chronological_commits,
        },
        TestCase {
            name: "t1::splice_commit_range",
            func: convergence::test_splice_commit_range,
        },
        TestCase {
            name: "t1::splice_single_commit",
            func: convergence::test_splice_single_commit,
        },
        // Entangle & Cronos
        TestCase {
            name: "t1::entangle_basic_linking",
            func: sync_daemon::test_entangle_basic_linking,
        },
        TestCase {
            name: "t1::entangle_path_filtered",
            func: sync_daemon::test_entangle_path_filtered,
        },
        TestCase {
            name: "t1::entangle_break_severance",
            func: sync_daemon::test_entangle_break_severance,
        },
        TestCase {
            name: "t1::entangle_log_history",
            func: sync_daemon::test_entangle_log_history,
        },
        TestCase {
            name: "t1::entangle_invalid_dimension_fails",
            func: sync_daemon::test_entangle_invalid_dimension_fails,
        },
        TestCase {
            name: "t1::cronos_status_when_stopped",
            func: sync_daemon::test_cronos_status_when_stopped,
        },
        TestCase {
            name: "t1::cronos_start_and_stop_lifecycle",
            func: sync_daemon::test_cronos_start_and_stop_lifecycle,
        },
        TestCase {
            name: "t1::cronos_log_inspection",
            func: sync_daemon::test_cronos_log_inspection,
        },
        TestCase {
            name: "t1::cronos_pause_and_resume_dimension",
            func: sync_daemon::test_cronos_pause_and_resume_dimension,
        },
        TestCase {
            name: "t1::cronos_config_rules",
            func: sync_daemon::test_cronos_config_rules,
        },
        TestCase {
            name: "t1::cronos_wal_log_integrity",
            func: sync_daemon::test_cronos_wal_log_integrity,
        },
        // Agent & Timeline
        TestCase {
            name: "t1::agent_register_ai_and_human",
            func: agent_timeline::test_agent_register_ai_and_human,
        },
        TestCase {
            name: "t1::agent_assign_dimension",
            func: agent_timeline::test_agent_assign_dimension,
        },
        TestCase {
            name: "t1::agent_status_activity",
            func: agent_timeline::test_agent_status_activity,
        },
        TestCase {
            name: "t1::agent_broadcast_and_inbox",
            func: agent_timeline::test_agent_broadcast_and_inbox,
        },
        TestCase {
            name: "t1::agent_heartbeat_reporting",
            func: agent_timeline::test_agent_heartbeat_reporting,
        },
        TestCase {
            name: "t1::agent_inbox_json_format",
            func: agent_timeline::test_agent_inbox_json_format,
        },
        TestCase {
            name: "t1::agent_register_duplicate_name_fails",
            func: agent_timeline::test_agent_register_duplicate_name_fails,
        },
        TestCase {
            name: "t1::timeline_multiverse_graph_rendering",
            func: agent_timeline::test_timeline_multiverse_graph_rendering,
        },
        TestCase {
            name: "t1::timeline_single_dimension_history",
            func: agent_timeline::test_timeline_single_dimension_history,
        },
        TestCase {
            name: "t1::timeline_ancestry_graph",
            func: agent_timeline::test_timeline_ancestry_graph,
        },
        TestCase {
            name: "t1::timeline_export_json",
            func: agent_timeline::test_timeline_export_json,
        },
        TestCase {
            name: "t1::timeline_export_dot",
            func: agent_timeline::test_timeline_export_dot,
        },
        // Git Bridge & Remote
        TestCase {
            name: "t1::git_export_basic",
            func: git_remote::test_git_export_basic,
        },
        TestCase {
            name: "t1::git_export_specific_dimension",
            func: git_remote::test_git_export_specific_dimension,
        },
        TestCase {
            name: "t1::git_import_preserves_history",
            func: git_remote::test_git_import_preserves_history,
        },
        TestCase {
            name: "t1::git_compat_bridge_command",
            func: git_remote::test_git_compat_bridge_command,
        },
        TestCase {
            name: "t1::git_map_lookup",
            func: git_remote::test_git_map_translation_lookup,
        },
        TestCase {
            name: "t1::remote_add_and_list",
            func: git_remote::test_remote_add_and_list,
        },
        TestCase {
            name: "t1::remote_remove",
            func: git_remote::test_remote_remove,
        },
        TestCase {
            name: "t1::fetch_pull_help",
            func: git_remote::test_fetch_and_pull_help,
        },
        TestCase {
            name: "t1::push_help",
            func: git_remote::test_push_help,
        },
        TestCase {
            name: "t1::bundle_create",
            func: git_remote::test_bundle_create_and_verify,
        },
    ]
}
