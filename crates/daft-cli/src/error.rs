//! CLI error types and exit code mapping.

use thiserror::Error;

#[derive(Error, Debug)]
pub enum CliError {
    #[error("{0}")]
    General(String),

    #[error("Not a Daft repository (or any of the parent directories): .dft")]
    NotARepository,

    #[error("nothing to commit, working tree clean")]
    EmptyCommit,

    #[error("Aborting commit due to empty commit message.")]
    EmptyMessage,

    #[error("Automatic merge failed; fix conflicts and then commit the result.")]
    MergeConflict,

    #[error("Operation blocked by territory fence: FENCE on '{0}'")]
    FenceViolation(String),

    #[error("Dimension '{0}' already exists")]
    DimensionAlreadyExists(String),

    #[error("Dimension '{0}' does not exist")]
    DimensionNotFound(String),

    #[error("Cannot destroy dimension with uncommitted changes without --force")]
    DirtyDimensionDestroy,

    #[error("Core error: {0}")]
    Core(#[from] daft_core::DaftError),

    #[error("Repo error: {0}")]
    Repo(daft_core::error::RepoError),

    #[error("Ref error: {0}")]
    Ref(#[from] daft_core::error::RefError),

    #[error("Reflog error: {0}")]
    Reflog(#[from] daft_core::error::ReflogError),

    #[error("Index error: {0}")]
    Index(#[from] daft_core::error::IndexError),

    #[error("CAS error: {0}")]
    Cas(#[from] daft_core::cas::CasError),

    #[error("Object error: {0}")]
    Object(#[from] daft_core::object::ObjectError),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

impl From<daft_core::error::RepoError> for CliError {
    fn from(err: daft_core::error::RepoError) -> Self {
        match err {
            daft_core::error::RepoError::NotARepository(_) => CliError::NotARepository,
            other => CliError::Repo(other),
        }
    }
}

impl From<daft_dimension::DimensionError> for CliError {
    fn from(err: daft_dimension::DimensionError) -> Self {
        match err {
            daft_dimension::DimensionError::AlreadyExists(name) => {
                CliError::DimensionAlreadyExists(name)
            }
            daft_dimension::DimensionError::NotFound(name) => CliError::DimensionNotFound(name),
            daft_dimension::DimensionError::InvalidName(name) => {
                CliError::General(format!("Invalid dimension name: '{}'", name))
            }
            daft_dimension::DimensionError::CannotDestroyActive(_) => {
                CliError::General("Cannot destroy active dimension without --force".into())
            }
            daft_dimension::DimensionError::CannotDestroyDirty(_) => {
                CliError::DirtyDimensionDestroy
            }
            daft_dimension::DimensionError::MainlineReserved => {
                CliError::General("Cannot destroy mainline dimension".into())
            }
            daft_dimension::DimensionError::BareRepository => {
                CliError::General("Cannot enter dimension in bare repository".into())
            }
            daft_dimension::DimensionError::Core(c) => CliError::Core(c),
            daft_dimension::DimensionError::Index(i) => CliError::Index(i),
            daft_dimension::DimensionError::Ref(r) => CliError::Ref(r),
            daft_dimension::DimensionError::Cas(c) => CliError::Cas(c),
            daft_dimension::DimensionError::Io(i) => CliError::Io(i),
            daft_dimension::DimensionError::General(msg) => CliError::General(msg),
            other => CliError::General(other.to_string()),
        }
    }
}

impl CliError {
    pub fn exit_code(&self) -> u8 {
        match self {
            CliError::MergeConflict => 1,
            CliError::EmptyCommit => 1,
            CliError::EmptyMessage => 1,
            CliError::FenceViolation(_) => 1,
            CliError::DimensionAlreadyExists(_) => 1,
            CliError::DimensionNotFound(_) => 1,
            CliError::DirtyDimensionDestroy => 1,
            CliError::General(_) => 1,
            CliError::NotARepository => 1,
            CliError::Core(_) => 1,
            CliError::Repo(_) => 1,
            CliError::Ref(_) => 1,
            CliError::Reflog(_) => 1,
            CliError::Index(_) => 1,
            CliError::Cas(_) => 1,
            CliError::Object(_) => 1,
            CliError::Io(_) => 1,
        }
    }
}
