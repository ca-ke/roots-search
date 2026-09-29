mod jev;
mod search;

use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;

use search::Candidate;

const MAX_CONTENT_CHARS: usize = 8_000;

/// Find the files relevant to a question, with Jev judging relevance.
#[derive(Parser)]
#[command(name = "rs", version)]
struct Cli {
    /// Include hidden files and ignore .gitignore
    #[arg(short, long)]
    all: bool,

    /// What you are looking for
    query: String,

    /// Directory to search from
    #[arg(default_value = ".")]
    path: PathBuf,

    /// How many files Jev reads (the top ones by BM25)
    #[arg(short, long, default_value_t = 20)]
    limit: usize,

    /// Minimum relevance (0 to 1) to show a result
    #[arg(short, long, default_value_t = 0.5)]
    min: f64,
}

fn main() -> ExitCode {
    dotenvy::dotenv().ok();
    let cli = Cli::parse();

    match run(&cli) {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::from(1),
        Err(e) => {
            eprintln!("rs: {e:#}");
            ExitCode::from(2)
        }
    }
}

fn run(cli: &Cli) -> anyhow::Result<bool> {
    let candidates = search::candidates(&cli.path, &cli.query, cli.all, cli.limit);
    if candidates.is_empty() {
        return Ok(false);
    }

    let files: Vec<String> = candidates.iter().map(describe).collect();
    let mut ranked: Vec<_> = jev::score(&cli.query, &files)?.into_iter().zip(candidates).collect();
    ranked.sort_by(|a, b| b.0.total_cmp(&a.0));

    let mut found = false;
    for (score, c) in ranked.into_iter().filter(|(s, _)| *s >= cli.min) {
        found = true;
        println!("{score:.2}  {}", c.path.display());
        for line in &c.snippets {
            println!("      {line}");
        }
    }
    Ok(found)
}

fn describe(c: &Candidate) -> String {
    let text: String = c.content.chars().take(MAX_CONTENT_CHARS).collect();
    format!("Path: \"{}\". Content:\n{text}", c.path.display())
}
