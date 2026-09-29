use std::fs;
use std::path::{Path, PathBuf};

use ignore::WalkBuilder;

const MAX_FILE_BYTES: u64 = 1_000_000;
const MAX_SNIPPETS: usize = 3;
const MAX_SNIPPET_CHARS: usize = 160;
const PREFIX_CHARS: usize = 4;
const PREFIX_WEIGHT: f64 = 0.5;
const NAME_WEIGHT: f64 = 3.0;
const K1: f64 = 1.2;
const B: f64 = 0.75;

const STOPWORDS: &[&str] = &[
    "a", "o", "as", "os", "um", "uma", "de", "da", "do", "das", "dos", "em", "no", "na", "nos",
    "nas", "por", "para", "pra", "com", "sem", "que", "qual", "quais", "onde", "como", "quando",
    "e", "ou", "se", "ao", "aos", "é", "ser", "está", "tem", "isso", "esse", "essa", "meu",
    "minha", "algum", "alguma", "arquivo", "arquivos", "código", "codigo", "achar", "buscar",
    "procurar", "mostrar", "existe", "the", "an", "of", "in", "on", "at", "to", "for", "from",
    "by", "with", "and", "or", "is", "are", "be", "it", "this", "that", "these", "those",
    "what", "which", "where", "how", "when", "why", "does", "my", "any", "some", "file",
    "files", "code", "find", "show", "search", "look", "looking", "there",
];

const SKIP_DIRS: &[&str] = &[
    ".git", "target", "node_modules", "dist", "vendor", ".next", ".venv", "__pycache__",
];

pub struct Candidate {
    pub path: PathBuf,
    pub content: String,
    pub snippets: Vec<String>,
}

struct Term {
    full: String,
    stem: String,
}

impl Term {
    fn new(full: String) -> Self {
        let stem = full.chars().take(PREFIX_CHARS).collect();
        Term { full, stem }
    }

    fn weight(&self, token: &str) -> f64 {
        if token == self.full {
            1.0
        } else if token.starts_with(&self.stem) {
            PREFIX_WEIGHT
        } else {
            0.0
        }
    }

    fn count(&self, tokens: &[String]) -> f64 {
        tokens.iter().map(|t| self.weight(t)).sum()
    }
}

struct Doc {
    path: PathBuf,
    tf: Vec<f64>,
    len: f64,
}

pub fn candidates(root: &Path, query: &str, all: bool, max: usize) -> Vec<Candidate> {
    let terms = query_terms(query);
    if terms.is_empty() {
        return Vec::new();
    }

    let (docs, corpus_size, avg_len) = read_docs(root, &terms, all);
    let idf: Vec<f64> = (0..terms.len())
        .map(|i| {
            let df = docs.iter().filter(|d| d.tf[i] > 0.0).count() as f64;
            (1.0 + (corpus_size - df + 0.5) / (df + 0.5)).ln()
        })
        .collect();

    let mut scored: Vec<(f64, Doc)> = docs
        .into_iter()
        .map(|d| {
            let norm = K1 * (1.0 - B + B * d.len / avg_len);
            let score = d.tf.iter().zip(&idf).map(|(f, idf)| idf * f * (K1 + 1.0) / (f + norm));
            (score.sum(), d)
        })
        .collect();
    scored.sort_by(|a, b| b.0.total_cmp(&a.0));
    scored.truncate(max);

    scored
        .into_iter()
        .map(|(_, d)| {
            let content = read_text(&d.path);
            Candidate { snippets: best_snippets(&content, &terms), content, path: d.path }
        })
        .collect()
}

fn query_terms(query: &str) -> Vec<Term> {
    let words: Vec<String> = tokenize(query).into_iter().filter(|w| w.chars().count() >= 2).collect();
    let useful: Vec<String> =
        words.iter().filter(|w| !STOPWORDS.contains(&w.as_str())).cloned().collect();
    let chosen = if useful.is_empty() { words } else { useful };
    chosen.into_iter().map(Term::new).collect()
}

fn read_docs(root: &Path, terms: &[Term], all: bool) -> (Vec<Doc>, f64, f64) {
    let walker = WalkBuilder::new(root)
        .standard_filters(!all)
        .filter_entry(|e| {
            let name = e.file_name().to_string_lossy();
            let is_dir = e.file_type().is_some_and(|t| t.is_dir());
            !(is_dir && SKIP_DIRS.contains(&name.as_ref())) && !is_secret(&name)
        })
        .build();

    let mut docs = Vec::new();
    let (mut files, mut total_len) = (0usize, 0usize);
    for entry in walker.filter_map(Result::ok) {
        if !entry.file_type().is_some_and(|t| t.is_file()) {
            continue;
        }
        let path = entry.path();
        let content = read_text(path);
        let content_tokens = tokenize(&content);
        let name_tokens = tokenize(&path.file_name().unwrap_or_default().to_string_lossy());

        let len = content_tokens.len() + name_tokens.len();
        files += 1;
        total_len += len;

        let tf: Vec<f64> = terms
            .iter()
            .map(|t| t.count(&content_tokens) + NAME_WEIGHT * t.count(&name_tokens))
            .collect();
        if tf.iter().any(|&f| f > 0.0) {
            docs.push(Doc { path: path.to_path_buf(), tf, len: len as f64 });
        }
    }

    let corpus_size = files as f64;
    let avg_len = (total_len as f64 / corpus_size.max(1.0)).max(1.0);
    (docs, corpus_size, avg_len)
}

fn read_text(path: &Path) -> String {
    match fs::metadata(path) {
        Ok(m) if m.len() <= MAX_FILE_BYTES => fs::read_to_string(path).unwrap_or_default(),
        _ => String::new(),
    }
}

fn best_snippets(content: &str, terms: &[Term]) -> Vec<String> {
    let mut lines: Vec<(f64, usize, &str)> = content
        .lines()
        .enumerate()
        .map(|(i, line)| {
            let tokens = tokenize(line);
            let score = terms.iter().map(|t| t.count(&tokens).min(1.0)).sum();
            (score, i, line)
        })
        .filter(|&(score, ..)| score > 0.0)
        .collect();
    lines.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
    lines.truncate(MAX_SNIPPETS);
    lines.sort_by_key(|&(_, i, _)| i);

    lines
        .into_iter()
        .map(|(.., line)| line.trim().chars().take(MAX_SNIPPET_CHARS).collect())
        .collect()
}

fn is_secret(name: &str) -> bool {
    (name == ".env" || name.starts_with(".env.")) && !name.ends_with(".example")
}

fn tokenize(text: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut cur = String::new();
    let mut prev_lower = false;
    for ch in text.chars() {
        if !ch.is_alphanumeric() {
            if !cur.is_empty() {
                tokens.push(std::mem::take(&mut cur));
            }
            prev_lower = false;
            continue;
        }
        if ch.is_uppercase() && prev_lower && !cur.is_empty() {
            tokens.push(std::mem::take(&mut cur));
        }
        prev_lower = ch.is_lowercase();
        cur.extend(ch.to_lowercase());
    }
    if !cur.is_empty() {
        tokens.push(cur);
    }
    tokens
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokenize_splits_camel_and_snake_case() {
        assert_eq!(tokenize("signInWithPassword next_review"), vec!["sign", "in", "with", "password", "next", "review"]);
    }

    #[test]
    fn env_files_are_secret_but_example_is_not() {
        assert!(is_secret(".env") && is_secret(".env.local"));
        assert!(!is_secret(".env.example") && !is_secret("env.ts"));
    }
}
