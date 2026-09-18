use crate::cli::ConfigArgs;
use crate::error::CliError;
use daft_core::Repository;
use std::env;
use std::fs;

pub fn execute(args: ConfigArgs) -> Result<(), CliError> {
    let cwd = env::current_dir()?;
    let repo = Repository::discover(&cwd)?;
    let config_path = repo.dft_dir().join("dft.toml");

    let content = if config_path.exists() {
        fs::read_to_string(&config_path)?
    } else {
        String::new()
    };

    let mut doc: toml::Table = content.parse().unwrap_or_default();

    if let Some(pair) = &args.set {
        if pair.len() >= 2 {
            let key = &pair[0];
            let val = &pair[1];
            if let Some((section, subkey)) = key.split_once('.') {
                let entry = doc
                    .entry(section)
                    .or_insert_with(|| toml::Value::Table(toml::Table::new()));
                if let toml::Value::Table(t) = entry {
                    t.insert(subkey.to_string(), toml::Value::String(val.clone()));
                }
            } else {
                doc.insert(key.clone(), toml::Value::String(val.clone()));
            }
            fs::write(
                &config_path,
                toml::to_string_pretty(&doc).unwrap_or_default(),
            )?;
            return Ok(());
        }
    }

    if let Some(key) = &args.get {
        if let Some((section, subkey)) = key.split_once('.') {
            if let Some(toml::Value::Table(t)) = doc.get(section) {
                if let Some(v) = t.get(subkey) {
                    println!("{}", v.as_str().unwrap_or(&v.to_string()));
                    return Ok(());
                }
            }
        } else if let Some(v) = doc.get(key) {
            println!("{}", v.as_str().unwrap_or(&v.to_string()));
            return Ok(());
        }
        return Err(CliError::General(format!("Key '{}' not found", key)));
    }

    if let Some(key) = &args.unset {
        if let Some((section, subkey)) = key.split_once('.') {
            if let Some(toml::Value::Table(t)) = doc.get_mut(section) {
                t.remove(subkey);
            }
        } else {
            doc.remove(key);
        }
        fs::write(
            &config_path,
            toml::to_string_pretty(&doc).unwrap_or_default(),
        )?;
        return Ok(());
    }

    if args.list {
        print!("{}", content);
    }

    Ok(())
}
