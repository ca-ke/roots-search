use std::fs;
use std::path::{Path, PathBuf};

use ignore::WalkBuilder;

const MAX_FILE_BYTES: u64 = 1_000_000;
const MAX_SNIPPETS: usize = 3;
const MAX_SNIPPET_CHARS: usize = 160;

pub struct Candidate {
    pub path: PathBuf,
    pub snippets: Vec<String>,
    terms: usize,
    name_match: bool,
    hits: usize,
}

/// Coleta os arquivos que mencionam algum termo da query (no nome ou no conteúdo)
/// e devolve os `max` melhores. É um filtro barato: o Jev decide a relevância depois.
pub fn candidates(root: &Path, query: &str, all: bool, max: usize) -> Vec<Candidate> {
    let terms: Vec<String> = query
        .split_whitespace()
        .map(str::to_lowercase)
        .filter(|t| t.chars().count() >= 2)
        .collect();
    if terms.is_empty() {
        return Vec::new();
    }

    let walker = WalkBuilder::new(root)
        .hidden(!all)
        .ignore(!all)
        .git_ignore(!all)
        .git_global(!all)
        .git_exclude(!all)
        .parents(!all)
        .filter_entry(|e| e.file_name() != ".git" && e.file_name() != "target")
        .build();

    let mut found = Vec::new();
    for entry in walker.filter_map(Result::ok) {
        if !entry.file_type().is_some_and(|t| t.is_file()) {
            continue;
        }
        let path = entry.path();
        let name = path.file_name().unwrap_or_default().to_string_lossy().to_lowercase();
        let content = match entry.metadata() {
            Ok(m) if m.len() <= MAX_FILE_BYTES => fs::read_to_string(path).unwrap_or_default(),
            _ => String::new(),
        };
        let content_lower = content.to_lowercase();

        let matched = terms
            .iter()
            .filter(|t| name.contains(t.as_str()) || content_lower.contains(t.as_str()))
            .count();
        if matched == 0 {
            continue;
        }

        let mut hits = 0;
        let mut snippets = Vec::new();
        for line in content.lines() {
            let lower = line.to_lowercase();
            if terms.iter().any(|t| lower.contains(t.as_str())) {
                hits += 1;
                if snippets.len() < MAX_SNIPPETS {
                    snippets.push(line.trim().chars().take(MAX_SNIPPET_CHARS).collect());
                }
            }
        }

        found.push(Candidate {
            path: path.to_path_buf(),
            snippets,
            terms: matched,
            name_match: terms.iter().any(|t| name.contains(t.as_str())),
            hits,
        });
    }

    found.sort_by(|a, b| {
        b.terms
            .cmp(&a.terms)
            .then(b.name_match.cmp(&a.name_match))
            .then(b.hits.cmp(&a.hits))
    });
    found.truncate(max);
    found
}
