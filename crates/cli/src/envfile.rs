//! Arquivos `CHAVE=valor` (`.env` dos projetos e `host.env`), sempre 0600.

use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::Context;

pub struct EnvFile {
    path: PathBuf,
}

impl EnvFile {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        EnvFile { path: path.into() }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn read(&self) -> anyhow::Result<Vec<(String, String)>> {
        let text = fs::read_to_string(&self.path)
            .with_context(|| format!("não foi possível ler {}", self.path.display()))?;
        Ok(parse(&text))
    }

    /// Valor de uma chave (vazio conta como ausente).
    pub fn get(&self, key: &str) -> anyhow::Result<Option<String>> {
        Ok(self
            .read()?
            .into_iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v)
            .filter(|v| !v.is_empty()))
    }

    /// Troca (ou acrescenta) uma chave, preservando o resto do arquivo.
    /// Valores com `$` vão entre aspas simples (o Compose não interpola).
    pub fn set(&self, key: &str, value: &str) -> anyhow::Result<()> {
        let line = if value.contains('$') {
            format!("{key}='{value}'")
        } else {
            format!("{key}={value}")
        };
        let text = fs::read_to_string(&self.path).unwrap_or_default();
        let mut found = false;
        let mut lines: Vec<String> = text
            .lines()
            .map(|l| {
                if key_of(l) == Some(key) {
                    found = true;
                    line.clone()
                } else {
                    l.to_owned()
                }
            })
            .collect();
        if !found {
            lines.push(line);
        }
        write_private(&self.path, &(lines.join("\n") + "\n"))
    }

    pub fn remove(&self, key: &str) -> anyhow::Result<()> {
        let text = fs::read_to_string(&self.path).unwrap_or_default();
        let lines: Vec<&str> = text.lines().filter(|l| key_of(l) != Some(key)).collect();
        write_private(&self.path, &(lines.join("\n") + "\n"))
    }
}

fn key_of(line: &str) -> Option<&str> {
    let line = line.trim();
    if line.starts_with('#') {
        return None;
    }
    line.split_once('=').map(|(k, _)| k.trim())
}

/// `CHAVE=valor`, ignorando comentários; aspas simples/duplas são removidas.
pub fn parse(text: &str) -> Vec<(String, String)> {
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .filter_map(|l| l.split_once('='))
        .map(|(k, v)| {
            let v = v.trim();
            let v = v
                .strip_prefix('\'')
                .and_then(|v| v.strip_suffix('\''))
                .or_else(|| v.strip_prefix('"').and_then(|v| v.strip_suffix('"')))
                .unwrap_or(v);
            (k.trim().to_owned(), v.to_owned())
        })
        .collect()
}

/// Grava um arquivo legível só pelo dono (0600 em Unix).
pub fn write_private(path: &Path, content: &str) -> anyhow::Result<()> {
    fs::write(path, content)
        .with_context(|| format!("não foi possível gravar {}", path.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn le_env_com_aspas_e_comentarios() {
        let env = parse("# comentário\nA=1\nB='$argon2id$v=19$x'\nC=\"com espaço\"\n\nD=\n");
        assert_eq!(
            env,
            vec![
                ("A".into(), "1".into()),
                ("B".into(), "$argon2id$v=19$x".into()),
                ("C".into(), "com espaço".into()),
                ("D".into(), String::new()),
            ]
        );
    }

    #[test]
    fn set_e_remove_preservam_o_resto() {
        let dir = std::env::temp_dir().join(format!("nelcota-env-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let file = EnvFile::new(dir.join("teste.env"));
        fs::write(file.path(), "# topo\nA=1\nB=2\n").unwrap();
        file.set("B", "$hash$").unwrap();
        file.set("C", "3").unwrap();
        file.remove("A").unwrap();
        assert_eq!(
            fs::read_to_string(file.path()).unwrap(),
            "# topo\nB='$hash$'\nC=3\n"
        );
        assert_eq!(file.get("B").unwrap().as_deref(), Some("$hash$"));
        fs::remove_dir_all(dir).unwrap();
    }
}
