use crate::error::RepoError;
use crate::repo::Repository;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct InitOptions {
    pub bare: bool,
    pub initial_branch: String,
    pub reinit: bool,
}

impl Default for InitOptions {
    fn default() -> Self {
        Self {
            bare: false,
            initial_branch: "main".to_string(),
            reinit: false,
        }
    }
}

pub fn init(target_dir: &Path, options: &InitOptions) -> Result<Repository, RepoError> {
    let dft_dir = if options.bare {
        target_dir.to_path_buf()
    } else {
        target_dir.join(".dft")
    };

    if dft_dir.exists() && dft_dir.join("HEAD").exists() && !options.reinit {
        return Repository::open(&dft_dir);
    }

    // Create required directory structure
    fs::create_dir_all(dft_dir.join("objects/pack"))?;
    fs::create_dir_all(dft_dir.join("objects/info"))?;
    fs::create_dir_all(dft_dir.join("objects/tmp"))?;
    fs::create_dir_all(dft_dir.join("refs/heads"))?;
    fs::create_dir_all(dft_dir.join("refs/tags"))?;
    fs::create_dir_all(dft_dir.join("refs/dimensions"))?;
    fs::create_dir_all(dft_dir.join("logs/refs/heads"))?;
    fs::create_dir_all(dft_dir.join("hooks"))?;

    // Write HEAD
    let head_content = format!("ref: refs/heads/{}\n", options.initial_branch);
    fs::write(dft_dir.join("HEAD"), head_content)?;

    // Write default dft.toml
    let default_config = format!(
        "[core]\nrepositoryformatversion = 0\nfilemode = true\nbare = {}\nlogallrefupdates = true\n\n[dimension]\ncurrent = \"mainline\"\n",
        options.bare
    );
    fs::write(dft_dir.join("dft.toml"), default_config)?;

    // Write description
    fs::write(
        dft_dir.join("description"),
        "Unnamed Daft repository; edit this file 'description' to name the repository.\n",
    )?;

    // Write current_dimension
    fs::write(dft_dir.join("current_dimension"), "mainline\n")?;

    Repository::open(&dft_dir)
}
