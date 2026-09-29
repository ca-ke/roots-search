use std::env;

use anyhow::{Context, Result, bail};
use serde_json::{Map, Value, json};

use crate::search::Candidate;

const ENDPOINT: &str = "https://openrouter.ai/api/alpha/decisions";
const DEFAULT_MODEL: &str = "typesafe/jev-1.13";

/// Pergunta ao Jev, numa única chamada, se cada candidato é relevante para a query.
/// O state é só a query; cada candidato vira uma pergunta `noul` independente
/// ("ask everything at once"), e o Jev responde todas em paralelo.
/// Devolve a probabilidade de relevância na mesma ordem dos candidatos.
pub fn score(query: &str, candidates: &[Candidate]) -> Result<Vec<f64>> {
    let key = env::var("OPENROUTER_API_KEY")
        .context("OPENROUTER_API_KEY não definida (crie um .env, veja .env.example)")?;
    let model = env::var("JEV_MODEL").unwrap_or_else(|_| DEFAULT_MODEL.to_string());

    let mut questions = Map::new();
    for (i, c) in candidates.iter().enumerate() {
        questions.insert(
            format!("c{i}"),
            json!({
                "type": "noul",
                "instructions": instructions(c),
                "criteria": {
                    "true": "The file is relevant to the query",
                    "false": "The file is not relevant to the query"
                }
            }),
        );
    }
    let body = json!({ "model": model, "state": { "query": query }, "questions": questions });

    let resp = reqwest::blocking::Client::new()
        .post(ENDPOINT)
        .bearer_auth(key)
        .json(&body)
        .send()
        .context("falha ao chamar o OpenRouter")?;
    let status = resp.status();
    let payload: Value = resp.json().context("resposta inválida do OpenRouter")?;
    if !status.is_success() {
        bail!("OpenRouter retornou {status}: {payload}");
    }

    (0..candidates.len())
        .map(|i| {
            payload["answers"][format!("c{i}")]["noul"]
                .as_f64()
                .with_context(|| format!("resposta ausente para o candidato {i}"))
        })
        .collect()
}

fn instructions(c: &Candidate) -> String {
    let mut s = format!(
        "Does the file at path \"{}\" match what the user is looking for in `query`?",
        c.path.display()
    );
    if !c.snippets.is_empty() {
        s.push_str(" Lines from the file: ");
        s.push_str(&c.snippets.join(" | "));
    }
    s
}
