//! Configuração por variáveis de ambiente (`NELCOTA_*`) e, opcionalmente, por
//! um arquivo TOML (`nelcota.toml` ou o caminho em `NELCOTA_CONFIG`).
//! Variáveis de ambiente têm precedência sobre o arquivo.

use std::{fmt, net::SocketAddr, str::FromStr};

use figment::{
    Figment,
    providers::{Env, Format, Serialized, Toml},
};
use serde::{Deserialize, Serialize};

/// Valor sensível: nunca aparece em `Debug` (e portanto nunca vai para log).
#[derive(Clone, Default, Deserialize, Serialize)]
#[serde(transparent)]
pub struct Secret(String);

impl Secret {
    pub fn new(value: impl Into<String>) -> Self {
        Secret(value.into())
    }

    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Secret(***)")
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum LogFormat {
    #[default]
    Text,
    Json,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Config {
    /// URL do Postgres com uma role dona do schema (usada só para migrações e
    /// para definir a senha da role `authenticator`). A API conecta no mesmo
    /// host/banco, mas como `authenticator`.
    pub database_url: Secret,
    /// Senha da role `authenticator`.
    pub authenticator_password: Secret,
    /// Segredo HS256 dos JWTs (provisório: no Marco 2 entra EdDSA + JWKS).
    pub jwt_secret: Secret,
    pub listen: SocketAddr,
    pub db_pool_size: usize,
    pub request_timeout_secs: u64,
    pub log_format: LogFormat,
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error(transparent)]
    Load(#[from] Box<figment::Error>),
    #[error("{0}")]
    Invalid(&'static str),
    #[error("NELCOTA_DATABASE_URL inválida: {0}")]
    DatabaseUrl(tokio_postgres::Error),
}

impl Default for Config {
    fn default() -> Self {
        Config {
            database_url: Secret::default(),
            authenticator_password: Secret::default(),
            jwt_secret: Secret::default(),
            listen: SocketAddr::from(([0, 0, 0, 0], 8000)),
            db_pool_size: 10,
            request_timeout_secs: 15,
            log_format: LogFormat::Text,
        }
    }
}

impl Config {
    pub fn load() -> Result<Self, ConfigError> {
        let file = std::env::var("NELCOTA_CONFIG").unwrap_or_else(|_| "nelcota.toml".into());
        let config: Config = Figment::from(Serialized::defaults(Config::default()))
            .merge(Toml::file(file))
            .merge(Env::prefixed("NELCOTA_").ignore(&["config"]))
            .extract()
            .map_err(Box::new)?;
        config.validate()?;
        Ok(config)
    }

    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.database_url.expose().is_empty() {
            return Err(ConfigError::Invalid("NELCOTA_DATABASE_URL é obrigatória"));
        }
        if self.authenticator_password.expose().len() < 16 {
            return Err(ConfigError::Invalid(
                "NELCOTA_AUTHENTICATOR_PASSWORD precisa de ao menos 16 caracteres",
            ));
        }
        if self.jwt_secret.expose().len() < 32 {
            return Err(ConfigError::Invalid(
                "NELCOTA_JWT_SECRET precisa de ao menos 32 caracteres",
            ));
        }
        if self.db_pool_size == 0 {
            return Err(ConfigError::Invalid("NELCOTA_DB_POOL_SIZE precisa ser > 0"));
        }
        self.database_config()?;
        Ok(())
    }

    pub fn database_config(&self) -> Result<tokio_postgres::Config, ConfigError> {
        tokio_postgres::Config::from_str(self.database_url.expose())
            .map_err(ConfigError::DatabaseUrl)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secret_nao_aparece_no_debug() {
        let config = Config {
            jwt_secret: Secret::new("super-secreto"),
            ..Config::default()
        };
        assert!(!format!("{config:?}").contains("super-secreto"));
    }
}
