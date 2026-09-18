//! Shortlog author aggregation (`dft shortlog`).

use super::log::get_log;
use crate::error::DaftError;
use crate::repo::Repository;
use std::collections::BTreeMap;

#[derive(Debug, Clone)]
pub struct AuthorLog {
    pub author: String,
    pub count: usize,
    pub subjects: Vec<String>,
}

pub fn get_shortlog(
    repo: &Repository,
    numbered: bool,
    show_email: bool,
) -> Result<Vec<AuthorLog>, DaftError> {
    let logs = get_log(repo, None, None, false)?;
    let mut map: BTreeMap<String, Vec<String>> = BTreeMap::new();

    for entry in logs {
        let key = if show_email {
            format!("{} <{}>", entry.author.name, entry.author.email)
        } else {
            entry.author.name.clone()
        };
        let subject = entry.message.lines().next().unwrap_or("").to_string();
        map.entry(key).or_default().push(subject);
    }

    let mut result: Vec<AuthorLog> = map
        .into_iter()
        .map(|(author, subjects)| {
            let count = subjects.len();
            AuthorLog {
                author,
                count,
                subjects,
            }
        })
        .collect();

    if numbered {
        result.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.author.cmp(&b.author)));
    } else {
        result.sort_by(|a, b| a.author.cmp(&b.author));
    }

    Ok(result)
}
