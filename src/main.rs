mod jev;
mod search;

use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;

/// Busca arquivos relevantes para uma pergunta, com o Jev decidindo a relevância.
#[derive(Parser)]
#[command(name = "rs", version)]
struct Cli {
    /// Inclui arquivos ocultos e ignora .gitignore
    #[arg(short, long)]
    all: bool,

    /// O que você procura
    query: String,

    /// Diretório inicial da busca
    #[arg(default_value = ".")]
    path: PathBuf,

    /// Máximo de candidatos enviados ao Jev
    #[arg(short, long, default_value_t = 20)]
    limit: usize,

    /// Relevância mínima (0 a 1) para exibir um resultado
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

    let scores = jev::score(&cli.query, &candidates)?;
    let mut ranked: Vec<_> = scores.into_iter().zip(&candidates).collect();
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
