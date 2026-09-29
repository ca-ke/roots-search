mod jev;
mod search;

use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;

use search::Candidate;

/// Quantos dos melhores da 1ª passada o Jev relê com o conteúdo inteiro.
const FINALISTS: usize = 10;
/// Limite de conteúdo por arquivo na 2ª passada.
const MAX_CONTENT_CHARS: usize = 8_000;

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
    #[arg(short, long, default_value_t = 50)]
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

    // 1ª passada: caminho e linhas que casam, barato, só para achar os arquivos certos.
    let previews: Vec<String> = candidates.iter().map(preview).collect();
    let mut ranked = rank(jev::score(&cli.query, &previews)?, candidates);
    ranked.truncate(FINALISTS);

    // 2ª passada: o Jev lê o conteúdo dos finalistas e dá a nota final.
    let finalists: Vec<Candidate> = ranked.into_iter().map(|(_, c)| c).collect();
    let contents: Vec<String> = finalists.iter().map(content).collect();
    let ranked = rank(jev::score(&cli.query, &contents)?, finalists);

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

fn rank(scores: Vec<f64>, candidates: Vec<Candidate>) -> Vec<(f64, Candidate)> {
    let mut ranked: Vec<_> = scores.into_iter().zip(candidates).collect();
    ranked.sort_by(|a, b| b.0.total_cmp(&a.0));
    ranked
}

fn preview(c: &Candidate) -> String {
    format!("Path: \"{}\". Matching lines: {}", c.path.display(), c.snippets.join(" | "))
}

fn content(c: &Candidate) -> String {
    let text: String = search::read_text(&c.path).chars().take(MAX_CONTENT_CHARS).collect();
    format!("Path: \"{}\". Content:\n{text}", c.path.display())
}
