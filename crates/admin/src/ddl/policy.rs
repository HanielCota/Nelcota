//! DDL de policies de RLS.
//!
//! Editar = `DROP` + `CREATE` na mesma transação: `ALTER POLICY` não muda o
//! comando (`FOR SELECT` → `FOR UPDATE`) nem o tipo (permissiva/restritiva).

use nelcota_api::query::ident;
use serde::Deserialize;

use super::{Result, expression, invalid, qualified, validate_name};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Command {
    All,
    Select,
    Insert,
    Update,
    Delete,
}

impl Command {
    fn as_sql(self) -> &'static str {
        match self {
            Command::All => "ALL",
            Command::Select => "SELECT",
            Command::Insert => "INSERT",
            Command::Update => "UPDATE",
            Command::Delete => "DELETE",
        }
    }

    /// INSERT só tem linha nova (WITH CHECK); SELECT/DELETE só a existente (USING).
    fn accepts_using(self) -> bool {
        self != Command::Insert
    }

    fn accepts_check(self) -> bool {
        !matches!(self, Command::Select | Command::Delete)
    }
}

/// `public` = todas as roles.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PolicyRole {
    Public,
    Anon,
    Authenticated,
    ServiceRole,
}

impl PolicyRole {
    fn as_sql(self) -> String {
        match self {
            // PUBLIC é palavra-chave, não role: vai sem aspas.
            PolicyRole::Public => "PUBLIC".to_owned(),
            PolicyRole::Anon => ident("anon"),
            PolicyRole::Authenticated => ident("authenticated"),
            PolicyRole::ServiceRole => ident("service_role"),
        }
    }
}

const fn yes() -> bool {
    true
}

#[derive(Debug, Clone, Deserialize)]
pub struct PolicyDef {
    pub name: String,
    pub command: Command,
    /// Vazio = `PUBLIC`.
    #[serde(default)]
    pub roles: Vec<PolicyRole>,
    #[serde(default = "yes")]
    pub permissive: bool,
    /// Quais linhas existentes a role enxerga/altera.
    pub using: Option<String>,
    /// Quais linhas novas/alteradas são aceitas.
    pub check: Option<String>,
}

fn non_empty(text: &Option<String>) -> Option<&str> {
    text.as_deref().map(str::trim).filter(|t| !t.is_empty())
}

pub fn create(schema: &str, table: &str, def: &PolicyDef) -> Result<Vec<String>> {
    validate_name("policy", &def.name)?;
    let using = non_empty(&def.using);
    let check = non_empty(&def.check);
    if using.is_some() && !def.command.accepts_using() {
        return invalid(
            "policy de INSERT não usa USING: a linha ainda não existe (use WITH CHECK)",
        );
    }
    if check.is_some() && !def.command.accepts_check() {
        return invalid(format!(
            "policy de {} não usa WITH CHECK: nenhuma linha é gravada (use USING)",
            def.command.as_sql()
        ));
    }
    match def.command {
        Command::Insert if check.is_none() => {
            return invalid("policy de INSERT precisa de WITH CHECK");
        }
        Command::Insert => {}
        _ if using.is_none() => return invalid("a policy precisa da expressão USING"),
        _ => {}
    }

    let mut roles: Vec<String> = Vec::new();
    for role in &def.roles {
        let sql = role.as_sql();
        if !roles.contains(&sql) {
            roles.push(sql);
        }
    }
    if roles.is_empty() || def.roles.contains(&PolicyRole::Public) {
        roles = vec!["PUBLIC".to_owned()];
    }

    let mut sql = format!(
        "CREATE POLICY {} ON {} AS {} FOR {} TO {}",
        ident(&def.name),
        qualified(schema, table),
        if def.permissive {
            "PERMISSIVE"
        } else {
            "RESTRICTIVE"
        },
        def.command.as_sql(),
        roles.join(", ")
    );
    if let Some(using) = using {
        sql.push_str(&format!(" USING {}", expression("USING", using)?));
    }
    if let Some(check) = check {
        sql.push_str(&format!(" WITH CHECK {}", expression("WITH CHECK", check)?));
    }
    Ok(vec![sql])
}

pub fn drop(schema: &str, table: &str, name: &str) -> Vec<String> {
    vec![format!(
        "DROP POLICY {} ON {}",
        ident(name),
        qualified(schema, table)
    )]
}

/// Troca a policy `original` pela definição nova (pode mudar o nome).
pub fn replace(schema: &str, table: &str, original: &str, def: &PolicyDef) -> Result<Vec<String>> {
    let mut statements = drop(schema, table, original);
    statements.extend(create(schema, table, def)?);
    Ok(statements)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn def(command: Command, using: Option<&str>, check: Option<&str>) -> PolicyDef {
        PolicyDef {
            name: "dono".into(),
            command,
            roles: vec![PolicyRole::Authenticated],
            permissive: true,
            using: using.map(Into::into),
            check: check.map(Into::into),
        }
    }

    #[test]
    fn cria_policy_com_using_e_check() {
        let policy = def(
            Command::All,
            Some("dono = auth.uid()"),
            Some("dono = auth.uid()"),
        );
        assert_eq!(
            create("public", "notas", &policy).unwrap(),
            [
                "CREATE POLICY \"dono\" ON \"public\".\"notas\" AS PERMISSIVE FOR ALL TO \"authenticated\" \
              USING (dono = auth.uid()) WITH CHECK (dono = auth.uid())"
            ]
        );
    }

    #[test]
    fn roles_restritiva_e_public() {
        let mut policy = def(Command::Select, Some("true"), None);
        policy.permissive = false;
        policy.roles = vec![
            PolicyRole::Anon,
            PolicyRole::Authenticated,
            PolicyRole::Anon,
        ];
        assert!(
            create("public", "t", &policy).unwrap()[0]
                .contains("AS RESTRICTIVE FOR SELECT TO \"anon\", \"authenticated\" USING (true)")
        );
        policy.roles = vec![];
        assert!(create("public", "t", &policy).unwrap()[0].contains(" TO PUBLIC "));
        policy.roles = vec![PolicyRole::Anon, PolicyRole::Public];
        assert!(create("public", "t", &policy).unwrap()[0].contains(" TO PUBLIC "));
    }

    #[test]
    fn expressoes_conforme_o_comando() {
        assert!(create("public", "t", &def(Command::Insert, None, Some("true"))).is_ok());
        assert!(
            create(
                "public",
                "t",
                &def(Command::Insert, Some("true"), Some("true"))
            )
            .is_err()
        );
        assert!(create("public", "t", &def(Command::Insert, None, None)).is_err());
        assert!(
            create(
                "public",
                "t",
                &def(Command::Select, Some("true"), Some("true"))
            )
            .is_err()
        );
        assert!(create("public", "t", &def(Command::Delete, None, None)).is_err());
        assert!(create("public", "t", &def(Command::Update, Some("true"), None)).is_ok());
        assert!(create("public", "t", &def(Command::Select, Some("   "), None)).is_err());
    }

    #[test]
    fn editar_e_apagar_e_recriar() {
        let policy = PolicyDef {
            name: "novo nome".into(),
            ..def(Command::Select, Some("true"), None)
        };
        let sql = replace("public", "t", "antigo", &policy).unwrap();
        assert_eq!(sql[0], "DROP POLICY \"antigo\" ON \"public\".\"t\"");
        assert!(sql[1].starts_with("CREATE POLICY \"novo nome\" ON \"public\".\"t\""));
    }
}
