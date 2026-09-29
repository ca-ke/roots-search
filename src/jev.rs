use std::env;

use anyhow::{Context, Result, bail};
use serde_json::{Map, Value, json};

const ENDPOINT: &str = "https://openrouter.ai/api/alpha/decisions";
const DEFAULT_MODEL: &str = "typesafe/jev-1.13";

/// Pergunta ao Jev, numa única chamada, se cada arquivo é relevante para a query.
/// `files` descreve cada arquivo (caminho mais trechos ou conteúdo). O state é só a
/// query; cada arquivo vira uma pergunta `noul` independente, e o Jev responde todas
/// em paralelo. Devolve a probabilidade de relevância na mesma ordem de `files`.
pub fn score(query: &str, files: &[String]) -> Result<Vec<f64>> {
    let key = env::var("OPENROUTER_API_KEY")
        .context("OPENROUTER_API_KEY não definida (crie um .env, veja .env.example)")?;
    let model = env::var("JEV_MODEL").unwrap_or_else(|_| DEFAULT_MODEL.to_string());

    let mut questions = Map::new();
    for (i, file) in files.iter().enumerate() {
        questions.insert(
            format!("c{i}"),
            json!({
                "type": "noul",
                "instructions": format!(
                    "Does this file match what the user is looking for in `query`? {file}"
                ),
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

    (0..files.len())
        .map(|i| {
            payload["answers"][format!("c{i}")]["noul"]
                .as_f64()
                .with_context(|| format!("resposta ausente para o candidato {i}"))
        })
        .collect()
}
