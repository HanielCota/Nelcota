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
    /// Chave privada Ed25519 (PKCS#8, em PEM ou base64 de uma linha) usada para
    /// assinar os JWTs (EdDSA). A chave pública é publicada no JWKS.
    pub jwt_private_key: Option<Secret>,
    /// Segredo HS256 (modo simples/legado). Se houver chave EdDSA, só é usado
    /// para VERIFICAR tokens antigos; senão também assina.
    pub jwt_secret: Option<Secret>,
    /// Claim `iss` dos tokens emitidos.
    pub jwt_issuer: String,
    /// Validade do JWT de acesso, em segundos.
    pub jwt_expiry_secs: u64,
    /// Validade do refresh token (renovada a cada rotação), em dias.
    pub refresh_token_ttl_days: u32,
    pub signup_enabled: bool,
    /// Limite de requests por minuto, por IP, nos endpoints de login/cadastro.
    pub auth_rate_limit_per_minute: u32,
    /// Confiar no `X-Forwarded-For` (só atrás de um proxy como o Caddy).
    pub trust_proxy: bool,
    /// SMTP dos emails do auth, no formato `smtps://usuario:senha@host:465`
    /// (ou `smtp://...:587?tls=required`). Sem ele, a recuperação de senha
    /// fica desligada.
    pub smtp_url: Option<Secret>,
    /// Remetente dos emails: `Loja <nao-responda@loja.com>`.
    pub smtp_from: Option<String>,
    /// Página do app que recebe o link de recuperação de senha. O token vai no
    /// fragmento (`#type=recovery&token=...`), que não chega a nenhum servidor.
    pub password_recovery_url: Option<String>,
    /// Login do painel (separado dos usuários finais). Sem os dois, o painel
    /// fica desligado.
    pub admin_email: Option<String>,
    /// Hash PHC argon2id da senha do admin (gerado pelo `nelcota init`).
    pub admin_password_hash: Option<Secret>,
    /// Segredo compartilhado entre os projetos do mesmo host para o login
    /// único do painel (handoff de SSO). Ausente = login por projeto.
    pub admin_sso_secret: Option<Secret>,
    /// Nome deste projeto no host (`nelcota init --project`).
    pub project_name: Option<String>,
    /// Lista pública de projetos do host (nome + URL), para o seletor do painel.
    pub host_registry: Option<std::path::PathBuf>,
    /// Pasta `migrations/` do projeto, lida pelo painel ao gerar migrações.
    /// No container do projeto, `/migrations` é detectada sozinha.
    pub migrations_dir: Option<std::path::PathBuf>,
    /// Schema exposto pela API REST.
    pub db_schema: String,
    /// Teto de linhas por leitura na API (`None` = sem teto).
    pub max_rows: Option<i64>,
    pub listen: SocketAddr,
    pub db_pool_size: usize,
    pub request_timeout_secs: u64,
    /// `statement_timeout` das conexões da API.
    pub statement_timeout_secs: u64,
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
            jwt_private_key: None,
            jwt_secret: None,
            jwt_issuer: "nelcota".into(),
            jwt_expiry_secs: 900,
            refresh_token_ttl_days: 30,
            signup_enabled: true,
            auth_rate_limit_per_minute: 30,
            trust_proxy: false,
            smtp_url: None,
            smtp_from: None,
            password_recovery_url: None,
            admin_email: None,
            admin_password_hash: None,
            admin_sso_secret: None,
            project_name: None,
            host_registry: None,
            migrations_dir: None,
            db_schema: "public".into(),
            max_rows: None,
            listen: SocketAddr::from(([0, 0, 0, 0], 8000)),
            db_pool_size: 10,
            request_timeout_secs: 15,
            statement_timeout_secs: 10,
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
        match (self.jwt_private_key(), self.jwt_secret()) {
            (None, None) => {
                return Err(ConfigError::Invalid(
                    "defina NELCOTA_JWT_PRIVATE_KEY (recomendado) ou NELCOTA_JWT_SECRET",
                ));
            }
            (_, Some(secret)) if secret.len() < 32 => {
                return Err(ConfigError::Invalid(
                    "NELCOTA_JWT_SECRET precisa de ao menos 32 caracteres",
                ));
            }
            _ => {}
        }
        if self.jwt_expiry_secs == 0 || self.refresh_token_ttl_days == 0 {
            return Err(ConfigError::Invalid(
                "NELCOTA_JWT_EXPIRY_SECS e NELCOTA_REFRESH_TOKEN_TTL_DAYS precisam ser > 0",
            ));
        }
        if self.db_pool_size == 0 {
            return Err(ConfigError::Invalid("NELCOTA_DB_POOL_SIZE precisa ser > 0"));
        }
        self.mail()?;
        self.database_config()?;
        Ok(())
    }

    /// Chave EdDSA configurada (variável vazia conta como ausente).
    pub fn jwt_private_key(&self) -> Option<&str> {
        non_empty(&self.jwt_private_key)
    }

    /// Segredo HS256 configurado (variável vazia conta como ausente).
    pub fn jwt_secret(&self) -> Option<&str> {
        non_empty(&self.jwt_secret)
    }

    /// Email do auth: `None` se não configurado; erro se configurado pela
    /// metade (melhor falhar na subida do que descobrir no primeiro "esqueci
    /// minha senha").
    pub fn mail(&self) -> Result<Option<MailConfig<'_>>, ConfigError> {
        let url = non_empty(&self.smtp_url);
        let from = self
            .smtp_from
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty());
        let recovery = self
            .password_recovery_url
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty());
        let (url, from) = match (url, from) {
            (None, None) if recovery.is_none() => return Ok(None),
            (Some(url), Some(from)) => (url, from),
            _ => {
                return Err(ConfigError::Invalid(
                    "defina NELCOTA_SMTP_URL e NELCOTA_SMTP_FROM juntos (ou nenhum dos dois)",
                ));
            }
        };
        let Some(recovery_url) = recovery else {
            return Err(ConfigError::Invalid(
                "com SMTP configurado, defina NELCOTA_PASSWORD_RECOVERY_URL (a página do app que recebe o link)",
            ));
        };
        let http = recovery_url.starts_with("https://") || recovery_url.starts_with("http://");
        if !http || recovery_url.contains('#') {
            return Err(ConfigError::Invalid(
                "NELCOTA_PASSWORD_RECOVERY_URL precisa ser uma URL http(s) sem fragmento (#)",
            ));
        }
        Ok(Some(MailConfig {
            smtp_url: url,
            from,
            recovery_url,
        }))
    }

    pub fn database_config(&self) -> Result<tokio_postgres::Config, ConfigError> {
        tokio_postgres::Config::from_str(self.database_url.expose())
            .map_err(ConfigError::DatabaseUrl)
    }
}

/// Email do auth já validado (ver [`Config::mail`]).
#[derive(Debug)]
pub struct MailConfig<'a> {
    pub smtp_url: &'a str,
    pub from: &'a str,
    pub recovery_url: &'a str,
}

fn non_empty(value: &Option<Secret>) -> Option<&str> {
    value
        .as_ref()
        .map(|s| s.expose().trim())
        .filter(|s| !s.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secret_nao_aparece_no_debug() {
        let config = Config {
            jwt_secret: Some(Secret::new("super-secreto")),
            ..Config::default()
        };
        assert!(!format!("{config:?}").contains("super-secreto"));
    }

    fn with_mail(url: Option<&str>, from: Option<&str>, recovery: Option<&str>) -> Config {
        Config {
            smtp_url: url.map(Secret::new),
            smtp_from: from.map(Into::into),
            password_recovery_url: recovery.map(Into::into),
            ..Config::default()
        }
    }

    #[test]
    fn email_desligado_ou_completo() {
        assert!(with_mail(None, None, None).mail().unwrap().is_none());
        assert!(
            with_mail(Some(""), Some(" "), None)
                .mail()
                .unwrap()
                .is_none()
        );
        let config = with_mail(
            Some("smtps://u:p@smtp.x.com"),
            Some("a@x.com"),
            Some("https://app.x.com/nova-senha"),
        );
        let mail = config.mail().unwrap().unwrap();
        assert_eq!(mail.recovery_url, "https://app.x.com/nova-senha");
    }

    #[test]
    fn email_pela_metade_falha_na_subida() {
        for config in [
            with_mail(
                Some("smtps://u:p@smtp.x.com"),
                None,
                Some("https://app.x.com"),
            ),
            with_mail(None, Some("a@x.com"), Some("https://app.x.com")),
            with_mail(None, None, Some("https://app.x.com")),
            with_mail(Some("smtps://u:p@smtp.x.com"), Some("a@x.com"), None),
            with_mail(
                Some("smtps://u:p@smtp.x.com"),
                Some("a@x.com"),
                Some("app.x.com"),
            ),
            with_mail(
                Some("smtps://u:p@smtp.x.com"),
                Some("a@x.com"),
                Some("https://x.com/#/senha"),
            ),
        ] {
            assert!(config.mail().is_err(), "{config:?}");
        }
    }
}
