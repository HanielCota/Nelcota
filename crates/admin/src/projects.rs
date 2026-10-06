//! Projetos do host: lista pública (para o seletor do painel) e estado de cada
//! app.
//!
//! A lista vem de `shared/projects.json`, gerado pelo CLI e montado somente
//! leitura nos apps. Ela tem só nome e URL, nunca segredos, e é relida a cada
//! request: projetos novos aparecem sem reiniciar ninguém.

use std::{path::PathBuf, time::Duration};

use axum::{Json, extract::State};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
    time::timeout,
};

use crate::{AdminState, ApiError, sso::Sso};

/// Como este app se encaixa no host.
pub struct HostLink {
    /// Nome deste projeto.
    pub project: String,
    /// `shared/projects.json` (ausente fora de um host, ex.: `nelcota dev`).
    pub registry: Option<PathBuf>,
    /// Login único (handoff) habilitado.
    pub sso: Option<Sso>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RegistryProject {
    pub name: String,
    pub url: String,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Registry {
    #[serde(default)]
    pub panel_login: String,
    #[serde(default)]
    pub projects: Vec<RegistryProject>,
}

fn read_registry(host: &HostLink) -> Registry {
    host.registry
        .as_ref()
        .and_then(|path| std::fs::read(path).ok())
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}

/// Projeto pelo nome, na lista do host.
pub fn find(host: &HostLink, name: &str) -> Option<RegistryProject> {
    read_registry(host)
        .projects
        .into_iter()
        .find(|p| p.name == name)
}

/// Nomes de projeto válidos (o mesmo formato que o CLI aceita); usados para
/// montar o endereço interno `app-<nome>`.
fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 32
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

/// `GET /admin/api/projects`
pub async fn list(State(state): State<AdminState>) -> Json<Value> {
    let registry = read_registry(&state.host);
    let mut projects: Vec<Value> = registry
        .projects
        .iter()
        .map(|p| json!({ "name": p.name, "url": p.url, "current": p.name == state.host.project }))
        .collect();
    if projects.is_empty() {
        projects.push(json!({ "name": state.host.project, "url": null, "current": true }));
    }
    Json(json!({
        "current": state.host.project,
        "sso": state.host.sso.is_some(),
        "projects": projects,
    }))
}

/// Faz `GET /health` no app de um projeto pela rede interna do host.
async fn probe(name: &str) -> Value {
    let started = std::time::Instant::now();
    let result = timeout(Duration::from_secs(2), async {
        let mut stream = TcpStream::connect((format!("app-{name}").as_str(), 8000)).await?;
        stream
            .write_all(b"GET /health HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
            .await?;
        let mut buf = Vec::with_capacity(512);
        stream.read_to_end(&mut buf).await?;
        Ok::<_, std::io::Error>(buf)
    })
    .await;
    let Ok(Ok(bytes)) = result else {
        return json!({ "healthy": false, "version": null, "latency_ms": null });
    };
    let text = String::from_utf8_lossy(&bytes);
    let healthy = text.starts_with("HTTP/1.1 200");
    let version = text
        .split("\r\n\r\n")
        .nth(1)
        .and_then(|body| serde_json::from_str::<Value>(body).ok())
        .and_then(|v| v.get("version").cloned())
        .unwrap_or(Value::Null);
    json!({
        "healthy": healthy,
        "version": version,
        "latency_ms": started.elapsed().as_millis() as u64,
    })
}

/// `GET /admin/api/projects/status`: estado de cada projeto (em paralelo).
pub async fn status(State(state): State<AdminState>) -> Result<Json<Value>, ApiError> {
    let registry = read_registry(&state.host);
    let probes = registry.projects.iter().map(|p| {
        let name = p.name.clone();
        async move {
            if valid_name(&name) {
                probe(&name).await
            } else {
                json!({ "healthy": false, "version": null, "latency_ms": null })
            }
        }
    });
    let results = futures_util::future::join_all(probes).await;
    let projects: Vec<Value> = registry
        .projects
        .iter()
        .zip(results)
        .map(|(p, mut status)| {
            status["name"] = json!(p.name);
            status["url"] = json!(p.url);
            status["current"] = json!(p.name == state.host.project);
            status
        })
        .collect();
    Ok(Json(
        json!({ "current": state.host.project, "projects": projects }),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn so_nomes_seguros_viram_endereco_interno() {
        assert!(valid_name("loja"));
        assert!(valid_name("blog-2"));
        assert!(!valid_name("Loja"));
        assert!(!valid_name("a.b"));
        assert!(!valid_name("x:80"));
        assert!(!valid_name(""));
    }
}
