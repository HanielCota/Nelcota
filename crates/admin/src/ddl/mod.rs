//! Geração de DDL a partir de especificações tipadas vindas do painel.
//!
//! Só monta texto: não toca no banco (quem executa é o `apply`). Assim cada
//! regra é testável isoladamente e a prévia ("ver SQL") mostra exatamente o
//! que vai rodar.
//!
//! Identificadores sempre vão entre aspas e tipos são conferidos contra uma
//! lista. Expressões (`DEFAULT`, `USING`, `WITH CHECK`) são SQL por natureza
//! e passam como estão: o admin já é o dono do banco (tem o editor SQL), e o
//! `apply` executa cada comando pelo protocolo estendido, que recusa mais de
//! um comando por vez. Regras que o Postgres já valida (tabela referenciada
//! existe, cast possível) ficam com ele: a mensagem dele volta como 400.

pub mod policy;
pub mod table;

use nelcota_api::query::ident;
use serde::Deserialize;

/// Erro de validação da especificação. Vira 400 (sem `Display` de propósito:
/// assim não cai na conversão genérica de erros para 500 do `ApiError`).
#[derive(Debug, PartialEq, Eq)]
pub struct DdlError(pub String);

pub type Result<T> = std::result::Result<T, DdlError>;

impl From<DdlError> for crate::ApiError {
    fn from(err: DdlError) -> Self {
        crate::ApiError::bad_request(err.0)
    }
}

fn invalid<T>(message: impl Into<String>) -> Result<T> {
    Err(DdlError(message.into()))
}

/// Limite de identificadores do Postgres (NAMEDATALEN - 1), em bytes.
const MAX_IDENT_BYTES: usize = 63;

/// Nome de tabela, coluna ou policy: o Postgres aceita quase tudo entre
/// aspas, mas cortaria em silêncio o que passar de 63 bytes.
pub fn validate_name(kind: &str, name: &str) -> Result<()> {
    if name.trim().is_empty() {
        return invalid(format!("{kind}: nome vazio"));
    }
    if name != name.trim() {
        return invalid(format!("{kind} '{name}': sem espaços no começo ou no fim"));
    }
    if name.len() > MAX_IDENT_BYTES {
        return invalid(format!(
            "{kind} '{name}': máximo de {MAX_IDENT_BYTES} bytes"
        ));
    }
    if name.contains('\0') {
        return invalid(format!("{kind}: caractere inválido"));
    }
    Ok(())
}

/// `"schema"."nome"`
pub fn qualified(schema: &str, name: &str) -> String {
    format!("{}.{}", ident(schema), ident(name))
}

/// Literal de texto (`standard_conforming_strings` é padrão desde o 9.1).
pub fn literal(text: &str) -> String {
    format!("'{}'", text.replace('\'', "''"))
}

/// Expressão SQL obrigatória (não vazia), entre parênteses.
fn expression(kind: &str, sql: &str) -> Result<String> {
    let sql = sql.trim();
    if sql.is_empty() {
        return invalid(format!("{kind}: expressão vazia"));
    }
    Ok(format!("({sql})"))
}

/// Tipos que o painel oferece (além dos enums do schema). Nomes canônicos do
/// Postgres, na ordem em que aparecem no formulário.
pub const BASE_TYPES: [&str; 18] = [
    "text",
    "varchar",
    "uuid",
    "smallint",
    "integer",
    "bigint",
    "numeric",
    "real",
    "double precision",
    "boolean",
    "date",
    "time",
    "timestamp",
    "timestamptz",
    "interval",
    "jsonb",
    "json",
    "bytea",
];

const INTEGER_TYPES: [&str; 3] = ["smallint", "integer", "bigint"];
/// Tipos que aceitam modificador: `varchar(80)`, `numeric(10,2)`, `timestamptz(3)`.
const WITH_MODIFIER: [&str; 5] = ["varchar", "numeric", "time", "timestamp", "timestamptz"];

/// Tipo de coluna validado: um dos [`BASE_TYPES`] ou um enum do schema, com
/// modificador opcional e `[]` opcional.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataType {
    base: String,
    is_enum: bool,
    modifier: Option<String>,
    array: bool,
}

impl DataType {
    pub fn parse(text: &str, enums: &[String]) -> Result<Self> {
        let text = text.trim();
        let (rest, array) = match text.strip_suffix("[]") {
            Some(rest) => (rest.trim_end(), true),
            None => (text, false),
        };
        let (base, modifier) = match rest.split_once('(') {
            Some((base, args)) => {
                let args = args
                    .strip_suffix(')')
                    .ok_or_else(|| DdlError(format!("tipo '{text}': parêntese sem fechar")))?;
                (base.trim(), Some(args.replace(' ', "")))
            }
            None => (rest, None),
        };
        let lower = base.to_lowercase();
        let is_enum = enums.iter().any(|e| e == base);
        let base = if is_enum {
            base.to_owned()
        } else if let Some(known) = BASE_TYPES.iter().find(|t| **t == lower) {
            (*known).to_owned()
        } else {
            return invalid(format!("tipo desconhecido: '{base}'"));
        };
        if let Some(args) = &modifier {
            let ok = WITH_MODIFIER.contains(&base.as_str())
                && !args.is_empty()
                && args.split(',').count() <= 2
                && args
                    .split(',')
                    .all(|a| !a.is_empty() && a.bytes().all(|b| b.is_ascii_digit()));
            if !ok {
                return invalid(format!("tipo '{text}': modificador inválido"));
            }
        }
        Ok(DataType {
            base,
            is_enum,
            modifier,
            array,
        })
    }

    pub fn is_integer(&self) -> bool {
        !self.array && INTEGER_TYPES.contains(&self.base.as_str())
    }

    /// SQL do tipo; enums vão qualificados com o schema.
    pub fn to_sql(&self, schema: &str) -> String {
        let mut sql = if self.is_enum {
            qualified(schema, &self.base)
        } else if self.base == "timestamptz" && self.modifier.is_some() {
            // timestamptz(3) não existe na gramática: é timestamp(3) with time zone.
            "timestamp".to_owned()
        } else {
            self.base.clone()
        };
        if let Some(args) = &self.modifier {
            sql.push_str(&format!("({args})"));
        }
        if self.base == "timestamptz" && self.modifier.is_some() {
            sql.push_str(" with time zone");
        }
        if self.array {
            sql.push_str("[]");
        }
        sql
    }
}

/// Roles da API que o painel deixa configurar em GRANTs e policies.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApiRole {
    Anon,
    Authenticated,
    ServiceRole,
}

impl ApiRole {
    pub fn as_str(self) -> &'static str {
        match self {
            ApiRole::Anon => "anon",
            ApiRole::Authenticated => "authenticated",
            ApiRole::ServiceRole => "service_role",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Privilege {
    Select,
    Insert,
    Update,
    Delete,
}

impl Privilege {
    fn as_sql(self) -> &'static str {
        match self {
            Privilege::Select => "SELECT",
            Privilege::Insert => "INSERT",
            Privilege::Update => "UPDATE",
            Privilege::Delete => "DELETE",
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct GrantDef {
    pub role: ApiRole,
    pub privileges: Vec<Privilege>,
}

/// `GRANT ... TO role`, ou nada se a lista estiver vazia.
fn grant(table: &str, def: &GrantDef) -> Option<String> {
    let mut privileges = def.privileges.clone();
    privileges.sort();
    privileges.dedup();
    if privileges.is_empty() {
        return None;
    }
    let list: Vec<&str> = privileges.iter().map(|p| p.as_sql()).collect();
    Some(format!(
        "GRANT {} ON TABLE {table} TO {}",
        list.join(", "),
        ident(def.role.as_str())
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn enums() -> Vec<String> {
        vec!["prioridade".into()]
    }

    #[test]
    fn nomes_validos_e_invalidos() {
        assert!(validate_name("tabela", "pedidos_2026").is_ok());
        assert!(validate_name("tabela", "descrição com espaço").is_ok());
        assert!(validate_name("tabela", "").is_err());
        assert!(validate_name("tabela", " pedidos").is_err());
        assert!(validate_name("tabela", &"x".repeat(64)).is_err());
        // 63 bytes, não 63 caracteres: "ç" ocupa 2.
        assert!(validate_name("tabela", &"ç".repeat(32)).is_err());
    }

    #[test]
    fn tipos_da_lista_com_modificador_e_array() {
        let sql = |t: &str| DataType::parse(t, &enums()).unwrap().to_sql("public");
        assert_eq!(sql("text"), "text");
        assert_eq!(sql("BIGINT"), "bigint");
        assert_eq!(sql("varchar(80)"), "varchar(80)");
        assert_eq!(sql("numeric(10, 2)"), "numeric(10,2)");
        assert_eq!(sql("text[]"), "text[]");
        assert_eq!(sql("double precision"), "double precision");
        assert_eq!(sql("timestamptz(3)"), "timestamp(3) with time zone");
        assert_eq!(sql("prioridade"), "\"public\".\"prioridade\"");
        assert_eq!(sql("prioridade[]"), "\"public\".\"prioridade\"[]");
    }

    #[test]
    fn tipos_recusados() {
        for bad in [
            "texto",
            "int; drop table x",
            "text(10)",
            "numeric(a)",
            "varchar(1,2,3)",
            "varchar(",
            "numeric()",
        ] {
            assert!(
                DataType::parse(bad, &enums()).is_err(),
                "{bad} deveria ser recusado"
            );
        }
    }

    #[test]
    fn literal_e_identificadores_escapados() {
        assert_eq!(literal("d'água"), "'d''água'");
        assert_eq!(qualified("public", "a\"b"), "\"public\".\"a\"\"b\"");
    }

    #[test]
    fn grant_ordena_e_remove_repetidos() {
        let def = GrantDef {
            role: ApiRole::Authenticated,
            privileges: vec![Privilege::Update, Privilege::Select, Privilege::Select],
        };
        assert_eq!(
            grant("\"public\".\"t\"", &def).unwrap(),
            "GRANT SELECT, UPDATE ON TABLE \"public\".\"t\" TO \"authenticated\""
        );
        let empty = GrantDef {
            role: ApiRole::Anon,
            privileges: vec![],
        };
        assert_eq!(grant("t", &empty), None);
    }
}
