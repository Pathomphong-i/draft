//! Tier 2 Test Suite Registry and Runner.

pub mod boundary_cases;

pub struct TestCase {
    pub name: &'static str,
    pub func: fn(),
}

pub fn get_all_tests() -> Vec<TestCase> {
    vec![
        TestCase {
            name: "t2::empty_repo_status",
            func: boundary_cases::test_empty_repo_status,
        },
        TestCase {
            name: "t2::empty_repo_log",
            func: boundary_cases::test_empty_repo_log,
        },
        TestCase {
            name: "t2::empty_repo_diff",
            func: boundary_cases::test_empty_repo_diff,
        },
        TestCase {
            name: "t2::empty_repo_branch",
            func: boundary_cases::test_empty_repo_branch,
        },
        TestCase {
            name: "t2::huge_file_storage",
            func: boundary_cases::test_huge_file_storage,
        },
        TestCase {
            name: "t2::binary_null_bytes",
            func: boundary_cases::test_binary_null_bytes,
        },
        TestCase {
            name: "t2::deep_directory_hierarchy",
            func: boundary_cases::test_deep_directory_hierarchy,
        },
        TestCase {
            name: "t2::unicode_filenames_and_paths",
            func: boundary_cases::test_unicode_filenames_and_paths,
        },
        TestCase {
            name: "t2::spaces_and_quotes_in_filename",
            func: boundary_cases::test_spaces_and_quotes_in_filename,
        },
        TestCase {
            name: "t2::invalid_syntax_unknown_subcommand",
            func: boundary_cases::test_invalid_syntax_unknown_subcommand,
        },
        TestCase {
            name: "t2::invalid_dimension_name_path_traversal",
            func: boundary_cases::test_invalid_dimension_name_path_traversal,
        },
        TestCase {
            name: "t2::invalid_dimension_name_empty",
            func: boundary_cases::test_invalid_dimension_name_empty,
        },
        TestCase {
            name: "t2::destroy_dimension_with_dirty_uncommitted_prevented",
            func: boundary_cases::test_destroy_dimension_with_dirty_uncommitted_prevented,
        },
        TestCase {
            name: "t2::rapid_consecutive_dimension_lifecycle",
            func: boundary_cases::test_rapid_consecutive_dimension_lifecycle,
        },
        TestCase {
            name: "t2::commit_without_message_fails",
            func: boundary_cases::test_commit_without_message_fails,
        },
    ]
}
