//! DDL de tabelas: criar, alterar (lista de ações numa transação) e apagar.

use nelcota_api::query::ident;
use serde::Deserialize;

use super::{
    DataType, GrantDef, Result, expression, grant, invalid, literal, qualified, validate_name,
};
use crate::structure::{ColumnInfo, Structure};

/// Contexto para validar tipos: schema exposto e enums dele.
pub struct Context<'a> {
    pub schema: &'a str,
    pub enums: &'a [String],
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OnDelete {
    #[default]
    NoAction,
    Restrict,
    Cascade,
    SetNull,
    SetDefault,
}

impl OnDelete {
    fn as_sql(self) -> &'static str {
        match self {
            OnDelete::NoAction => "NO ACTION",
            OnDelete::Restrict => "RESTRICT",
            OnDelete::Cascade => "CASCADE",
            OnDelete::SetNull => "SET NULL",
            OnDelete::SetDefault => "SET DEFAULT",
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct ReferenceDef {
    pub table: String,
    pub column: String,
    #[serde(default)]
    pub on_delete: OnDelete,
}

const fn yes() -> bool {
    true
}

#[derive(Debug, Clone, Deserialize)]
pub struct ColumnDef {
    pub name: String,
    pub data_type: String,
    #[serde(default = "yes")]
    pub nullable: bool,
    /// Expressão SQL (`now()`, `0`, `'rascunho'`).
    pub default: Option<String>,
    #[serde(default)]
    pub primary_key: bool,
    /// `GENERATED ALWAYS AS IDENTITY` (só tipos inteiros).
    #[serde(default)]
    pub identity: bool,
    #[serde(default)]
    pub unique: bool,
    pub references: Option<ReferenceDef>,
    pub comment: Option<String>,
}

fn references_sql(schema: &str, def: &ReferenceDef) -> Result<String> {
    validate_name("tabela referenciada", &def.table)?;
    validate_name("coluna referenciada", &def.column)?;
    Ok(format!(
        "REFERENCES {} ({}) ON DELETE {}",
        qualified(schema, &def.table),
        ident(&def.column),
        def.on_delete.as_sql()
    ))
}

/// Definição da coluna dentro de `CREATE TABLE`/`ADD COLUMN`.
/// `inline_pk`: a PK é desta coluna só (senão vai como constraint da tabela).
fn column_sql(ctx: &Context, col: &ColumnDef, inline_pk: bool) -> Result<String> {
    validate_name("coluna", &col.name)?;
    let data_type = DataType::parse(&col.data_type, ctx.enums)?;
    let mut sql = format!("{} {}", ident(&col.name), data_type.to_sql(ctx.schema));
    if col.identity {
        if !data_type.is_integer() {
            return invalid(format!(
                "coluna '{}': identity só em smallint, integer ou bigint",
                col.name
            ));
        }
        if col.default.is_some() {
            return invalid(format!(
                "coluna '{}': identity não aceita DEFAULT",
                col.name
            ));
        }
        sql.push_str(" GENERATED ALWAYS AS IDENTITY");
    }
    if inline_pk {
        sql.push_str(" PRIMARY KEY");
    } else if !col.nullable || col.identity || col.primary_key {
        sql.push_str(" NOT NULL");
    }
    if let Some(default) = &col.default {
        sql.push_str(&format!(
            " DEFAULT {}",
            expression(&format!("default de '{}'", col.name), default)?
        ));
    }
    if col.unique && !inline_pk {
        sql.push_str(" UNIQUE");
    }
    if let Some(reference) = &col.references {
        sql.push(' ');
        sql.push_str(&references_sql(ctx.schema, reference)?);
    }
    Ok(sql)
}

fn comment_on_column(table: &str, column: &str, comment: Option<&str>) -> String {
    format!(
        "COMMENT ON COLUMN {table}.{} IS {}",
        ident(column),
        comment
            .filter(|c| !c.trim().is_empty())
            .map_or("NULL".to_owned(), literal)
    )
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateTable {
    pub name: String,
    pub comment: Option<String>,
    pub columns: Vec<ColumnDef>,
    /// RLS ligado já na criação (padrão): sem policies, só service_role acessa.
    #[serde(default = "yes")]
    pub rls: bool,
    #[serde(default)]
    pub grants: Vec<GrantDef>,
}

pub fn create(ctx: &Context, spec: &CreateTable) -> Result<Vec<String>> {
    validate_name("tabela", &spec.name)?;
    if spec.columns.is_empty() {
        return invalid("a tabela precisa de pelo menos uma coluna");
    }
    for (i, col) in spec.columns.iter().enumerate() {
        if spec.columns[..i].iter().any(|c| c.name == col.name) {
            return invalid(format!("coluna '{}' repetida", col.name));
        }
    }
    let table = qualified(ctx.schema, &spec.name);
    let pk: Vec<&ColumnDef> = spec.columns.iter().filter(|c| c.primary_key).collect();
    let mut parts = spec
        .columns
        .iter()
        .map(|c| column_sql(ctx, c, c.primary_key && pk.len() == 1))
        .collect::<Result<Vec<_>>>()?;
    if pk.len() > 1 {
        let cols: Vec<String> = pk.iter().map(|c| ident(&c.name)).collect();
        parts.push(format!("PRIMARY KEY ({})", cols.join(", ")));
    }

    let mut statements = vec![format!(
        "CREATE TABLE {table} (\n  {}\n)",
        parts.join(",\n  ")
    )];
    if spec.rls {
        statements.push(format!("ALTER TABLE {table} ENABLE ROW LEVEL SECURITY"));
    }
    if let Some(comment) = spec.comment.as_deref().filter(|c| !c.trim().is_empty()) {
        statements.push(format!("COMMENT ON TABLE {table} IS {}", literal(comment)));
    }
    for col in &spec.columns {
        if let Some(comment) = col.comment.as_deref().filter(|c| !c.trim().is_empty()) {
            statements.push(comment_on_column(&table, &col.name, Some(comment)));
        }
    }
    statements.extend(spec.grants.iter().filter_map(|g| grant(&table, g)));
    Ok(statements)
}

/// Uma alteração. A lista inteira roda numa transação, na ordem enviada.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum AlterAction {
    RenameTable {
        name: String,
    },
    SetComment {
        comment: Option<String>,
    },
    SetRls {
        enabled: bool,
    },
    AddColumn {
        column: ColumnDef,
    },
    DropColumn {
        name: String,
    },
    RenameColumn {
        from: String,
        to: String,
    },
    /// `using`: expressão de conversão; padrão `"coluna"::novo_tipo`.
    SetType {
        column: String,
        data_type: String,
        using: Option<String>,
    },
    SetNullable {
        column: String,
        nullable: bool,
    },
    SetDefault {
        column: String,
        default: Option<String>,
    },
    SetUnique {
        column: String,
        unique: bool,
    },
    SetReference {
        column: String,
        reference: Option<ReferenceDef>,
    },
    SetColumnComment {
        column: String,
        comment: Option<String>,
    },
    /// Substitui os privilégios da role na tabela.
    SetGrants {
        grant: GrantDef,
    },
}

/// Coluna recém-adicionada na cópia de trabalho (nomes de constraints ainda
/// não existem: o Postgres só os cria ao executar).
fn added_column(col: &ColumnDef) -> ColumnInfo {
    ColumnInfo {
        name: col.name.clone(),
        data_type: col.data_type.clone(),
        nullable: col.nullable && !col.identity,
        default: col.default.clone(),
        identity: col.identity.then_some("always"),
        generated: false,
        primary_key: false,
        unique: None,
        references: None,
        comment: col.comment.clone(),
    }
}

fn column_exists<'a>(
    current: &'a Structure,
    name: &str,
) -> Result<&'a crate::structure::ColumnInfo> {
    current
        .column(name)
        .ok_or_else(|| super::DdlError(format!("coluna '{name}' não existe")))
}

/// `current`: estrutura atual (valida colunas e acha nomes de constraints).
pub fn alter(ctx: &Context, initial: &Structure, actions: &[AlterAction]) -> Result<Vec<String>> {
    if actions.is_empty() {
        return invalid("nenhuma alteração");
    }
    // Cópia de trabalho atualizada a cada ação: depois de renomear uma
    // coluna (ou a tabela), as ações seguintes usam o nome novo.
    let mut current = initial.clone();
    let mut table = qualified(ctx.schema, &current.name);
    let mut statements = Vec::new();
    for action in actions {
        let target = table.clone();
        let alter = |clause: String| format!("ALTER TABLE {target} {clause}");
        match action {
            AlterAction::RenameTable { name } => {
                validate_name("tabela", name)?;
                statements.push(alter(format!("RENAME TO {}", ident(name))));
                table = qualified(ctx.schema, name);
            }
            AlterAction::SetComment { comment } => statements.push(format!(
                "COMMENT ON TABLE {table} IS {}",
                comment
                    .as_deref()
                    .filter(|c| !c.trim().is_empty())
                    .map_or("NULL".to_owned(), literal)
            )),
            AlterAction::SetRls { enabled } => statements.push(alter(
                if *enabled {
                    "ENABLE ROW LEVEL SECURITY"
                } else {
                    "DISABLE ROW LEVEL SECURITY"
                }
                .to_owned(),
            )),
            AlterAction::AddColumn { column } => {
                if column.primary_key {
                    return invalid("a chave primária é definida na criação da tabela");
                }
                if current.column(&column.name).is_some() {
                    return invalid(format!("coluna '{}' já existe", column.name));
                }
                statements.push(alter(format!(
                    "ADD COLUMN {}",
                    column_sql(ctx, column, false)?
                )));
                if let Some(comment) = column.comment.as_deref().filter(|c| !c.trim().is_empty()) {
                    statements.push(comment_on_column(&table, &column.name, Some(comment)));
                }
                current.columns.push(added_column(column));
            }
            AlterAction::DropColumn { name } => {
                column_exists(&current, name)?;
                statements.push(alter(format!("DROP COLUMN {}", ident(name))));
                current.columns.retain(|c| c.name != *name);
            }
            AlterAction::RenameColumn { from, to } => {
                column_exists(&current, from)?;
                validate_name("coluna", to)?;
                statements.push(alter(format!(
                    "RENAME COLUMN {} TO {}",
                    ident(from),
                    ident(to)
                )));
                if let Some(info) = current.columns.iter_mut().find(|c| c.name == *from) {
                    info.name.clone_from(to);
                }
            }
            AlterAction::SetType {
                column,
                data_type,
                using,
            } => {
                column_exists(&current, column)?;
                let data_type = DataType::parse(data_type, ctx.enums)?.to_sql(ctx.schema);
                let using = match using {
                    Some(expr) => expression("USING", expr)?,
                    None => format!("{}::{data_type}", ident(column)),
                };
                statements.push(alter(format!(
                    "ALTER COLUMN {} TYPE {data_type} USING {using}",
                    ident(column)
                )));
            }
            AlterAction::SetNullable { column, nullable } => {
                column_exists(&current, column)?;
                let change = if *nullable {
                    "DROP NOT NULL"
                } else {
                    "SET NOT NULL"
                };
                statements.push(alter(format!("ALTER COLUMN {} {change}", ident(column))));
            }
            AlterAction::SetDefault { column, default } => {
                let info = column_exists(&current, column)?;
                if info.identity.is_some() || info.generated {
                    return invalid(format!(
                        "coluna '{column}' é identity/gerada: não aceita DEFAULT"
                    ));
                }
                let change = match default {
                    Some(expr) => format!("SET DEFAULT {}", expression("DEFAULT", expr)?),
                    None => "DROP DEFAULT".to_owned(),
                };
                statements.push(alter(format!("ALTER COLUMN {} {change}", ident(column))));
            }
            AlterAction::SetUnique { column, unique } => {
                let info = column_exists(&current, column)?;
                match (&info.unique, unique) {
                    (None, true) => {
                        statements.push(alter(format!("ADD UNIQUE ({})", ident(column))))
                    }
                    (Some(name), false) => {
                        statements.push(alter(format!("DROP CONSTRAINT {}", ident(name))))
                    }
                    _ => {}
                }
            }
            AlterAction::SetReference { column, reference } => {
                let info = column_exists(&current, column)?;
                if let Some(existing) = &info.references {
                    statements.push(alter(format!(
                        "DROP CONSTRAINT {}",
                        ident(&existing.constraint)
                    )));
                }
                if let Some(reference) = reference {
                    statements.push(alter(format!(
                        "ADD FOREIGN KEY ({}) {}",
                        ident(column),
                        references_sql(ctx.schema, reference)?
                    )));
                }
            }
            AlterAction::SetColumnComment { column, comment } => {
                column_exists(&current, column)?;
                statements.push(comment_on_column(&table, column, comment.as_deref()));
            }
            AlterAction::SetGrants { grant: def } => {
                let role = ident(def.role.as_str());
                statements.push(format!("REVOKE ALL ON TABLE {table} FROM {role}"));
                statements.extend(grant(&table, def));
            }
        }
    }
    Ok(statements)
}

pub fn drop(schema: &str, name: &str, cascade: bool) -> Vec<String> {
    let mut sql = format!("DROP TABLE {}", qualified(schema, name));
    if cascade {
        sql.push_str(" CASCADE");
    }
    vec![sql]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ddl::{ApiRole, Privilege};
    use crate::structure::ForeignKeyRef;

    const ENUMS: &[String] = &[];

    fn ctx() -> Context<'static> {
        Context {
            schema: "public",
            enums: ENUMS,
        }
    }

    fn col(name: &str, data_type: &str) -> ColumnDef {
        ColumnDef {
            name: name.into(),
            data_type: data_type.into(),
            nullable: true,
            default: None,
            primary_key: false,
            identity: false,
            unique: false,
            references: None,
            comment: None,
        }
    }

    fn spec(columns: Vec<ColumnDef>) -> CreateTable {
        CreateTable {
            name: "notas".into(),
            comment: None,
            columns,
            rls: true,
            grants: vec![],
        }
    }

    #[test]
    fn cria_tabela_com_pk_identity_default_fk_rls_comentarios_e_grants() {
        let mut table = spec(vec![
            ColumnDef {
                primary_key: true,
                identity: true,
                ..col("id", "bigint")
            },
            ColumnDef {
                nullable: false,
                ..col("texto", "text")
            },
            ColumnDef {
                default: Some("now()".into()),
                nullable: false,
                ..col("criada_em", "timestamptz")
            },
            ColumnDef {
                unique: true,
                ..col("slug", "varchar(80)")
            },
            ColumnDef {
                references: Some(ReferenceDef {
                    table: "clientes".into(),
                    column: "id".into(),
                    on_delete: OnDelete::Cascade,
                }),
                comment: Some("dona da nota".into()),
                ..col("cliente_id", "bigint")
            },
        ]);
        table.comment = Some("Notas d'equipe".into());
        table.grants = vec![
            GrantDef {
                role: ApiRole::Authenticated,
                privileges: vec![Privilege::Select, Privilege::Insert],
            },
            GrantDef {
                role: ApiRole::Anon,
                privileges: vec![],
            },
        ];
        assert_eq!(
            create(&ctx(), &table).unwrap(),
            [
                "CREATE TABLE \"public\".\"notas\" (\n  \
                 \"id\" bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,\n  \
                 \"texto\" text NOT NULL,\n  \
                 \"criada_em\" timestamptz NOT NULL DEFAULT (now()),\n  \
                 \"slug\" varchar(80) UNIQUE,\n  \
                 \"cliente_id\" bigint REFERENCES \"public\".\"clientes\" (\"id\") ON DELETE CASCADE\n)",
                "ALTER TABLE \"public\".\"notas\" ENABLE ROW LEVEL SECURITY",
                "COMMENT ON TABLE \"public\".\"notas\" IS 'Notas d''equipe'",
                "COMMENT ON COLUMN \"public\".\"notas\".\"cliente_id\" IS 'dona da nota'",
                "GRANT SELECT, INSERT ON TABLE \"public\".\"notas\" TO \"authenticated\"",
            ]
        );
    }

    #[test]
    fn chave_primaria_composta_vira_constraint_da_tabela() {
        let table = spec(vec![
            ColumnDef {
                primary_key: true,
                ..col("a", "integer")
            },
            ColumnDef {
                primary_key: true,
                ..col("b", "integer")
            },
        ]);
        let sql = &create(&ctx(), &table).unwrap()[0];
        assert!(sql.contains("\"a\" integer NOT NULL"), "{sql}");
        assert!(sql.ends_with("PRIMARY KEY (\"a\", \"b\")\n)"), "{sql}");
    }

    #[test]
    fn sem_rls_quando_pedido() {
        let mut table = spec(vec![col("x", "text")]);
        table.rls = false;
        assert_eq!(create(&ctx(), &table).unwrap().len(), 1);
    }

    #[test]
    fn criacao_recusa_especificacoes_invalidas() {
        let cases = [
            spec(vec![]),
            spec(vec![col("x", "text"), col("x", "integer")]),
            spec(vec![ColumnDef {
                identity: true,
                ..col("id", "text")
            }]),
            spec(vec![ColumnDef {
                identity: true,
                default: Some("1".into()),
                ..col("id", "bigint")
            }]),
            spec(vec![ColumnDef {
                default: Some("  ".into()),
                ..col("x", "text")
            }]),
            spec(vec![col("x", "money")]),
        ];
        for case in cases {
            assert!(create(&ctx(), &case).is_err(), "{case:?}");
        }
    }

    fn structure() -> Structure {
        let info = |name: &str| ColumnInfo {
            name: name.into(),
            data_type: "text".into(),
            nullable: true,
            default: None,
            identity: None,
            generated: false,
            primary_key: false,
            unique: None,
            references: None,
            comment: None,
        };
        Structure {
            name: "notas".into(),
            comment: None,
            rls_enabled: true,
            primary_key: vec!["id".into()],
            primary_key_constraint: Some("notas_pkey".into()),
            columns: vec![
                ColumnInfo {
                    identity: Some("always"),
                    primary_key: true,
                    ..info("id")
                },
                ColumnInfo {
                    unique: Some("notas_slug_key".into()),
                    ..info("slug")
                },
                ColumnInfo {
                    references: Some(ForeignKeyRef {
                        table: "clientes".into(),
                        column: "id".into(),
                        on_delete: "no action",
                        constraint: "notas_cliente_id_fkey".into(),
                    }),
                    ..info("cliente_id")
                },
                info("texto"),
            ],
            grants: vec![],
        }
    }

    #[test]
    fn alteracoes_de_coluna() {
        let actions = vec![
            AlterAction::AddColumn {
                column: ColumnDef {
                    default: Some("0".into()),
                    ..col("votos", "integer")
                },
            },
            AlterAction::SetType {
                column: "texto".into(),
                data_type: "varchar(200)".into(),
                using: None,
            },
            AlterAction::SetNullable {
                column: "texto".into(),
                nullable: false,
            },
            AlterAction::SetDefault {
                column: "texto".into(),
                default: Some("''".into()),
            },
            AlterAction::RenameColumn {
                from: "texto".into(),
                to: "conteudo".into(),
            },
            AlterAction::DropColumn {
                name: "slug".into(),
            },
        ];
        assert_eq!(
            alter(&ctx(), &structure(), &actions).unwrap(),
            [
                "ALTER TABLE \"public\".\"notas\" ADD COLUMN \"votos\" integer DEFAULT (0)",
                "ALTER TABLE \"public\".\"notas\" ALTER COLUMN \"texto\" TYPE varchar(200) USING \"texto\"::varchar(200)",
                "ALTER TABLE \"public\".\"notas\" ALTER COLUMN \"texto\" SET NOT NULL",
                "ALTER TABLE \"public\".\"notas\" ALTER COLUMN \"texto\" SET DEFAULT ('')",
                "ALTER TABLE \"public\".\"notas\" RENAME COLUMN \"texto\" TO \"conteudo\"",
                "ALTER TABLE \"public\".\"notas\" DROP COLUMN \"slug\"",
            ]
        );
    }

    #[test]
    fn acoes_seguintes_enxergam_colunas_renomeadas_adicionadas_e_removidas() {
        let actions = vec![
            AlterAction::RenameColumn {
                from: "texto".into(),
                to: "conteudo".into(),
            },
            AlterAction::SetNullable {
                column: "conteudo".into(),
                nullable: false,
            },
            AlterAction::AddColumn {
                column: col("votos", "integer"),
            },
            AlterAction::SetDefault {
                column: "votos".into(),
                default: Some("0".into()),
            },
            AlterAction::DropColumn {
                name: "slug".into(),
            },
        ];
        assert_eq!(alter(&ctx(), &structure(), &actions).unwrap().len(), 5);

        let stale = vec![
            AlterAction::RenameColumn {
                from: "texto".into(),
                to: "conteudo".into(),
            },
            AlterAction::SetNullable {
                column: "texto".into(),
                nullable: false,
            },
        ];
        assert!(
            alter(&ctx(), &structure(), &stale).is_err(),
            "nome antigo não vale mais"
        );
        let dropped = vec![
            AlterAction::DropColumn {
                name: "slug".into(),
            },
            AlterAction::SetUnique {
                column: "slug".into(),
                unique: true,
            },
        ];
        assert!(alter(&ctx(), &structure(), &dropped).is_err());
    }

    #[test]
    fn constraints_usam_os_nomes_atuais() {
        let actions = vec![
            AlterAction::SetUnique {
                column: "slug".into(),
                unique: false,
            },
            AlterAction::SetUnique {
                column: "texto".into(),
                unique: true,
            },
            // Já é unique: nada a fazer.
            AlterAction::SetUnique {
                column: "texto".into(),
                unique: true,
            },
            AlterAction::SetReference {
                column: "cliente_id".into(),
                reference: Some(ReferenceDef {
                    table: "clientes".into(),
                    column: "id".into(),
                    on_delete: OnDelete::SetNull,
                }),
            },
        ];
        let sql = alter(&ctx(), &structure(), &actions).unwrap();
        assert_eq!(
            sql[0],
            "ALTER TABLE \"public\".\"notas\" DROP CONSTRAINT \"notas_slug_key\""
        );
        assert_eq!(
            sql[1],
            "ALTER TABLE \"public\".\"notas\" ADD UNIQUE (\"texto\")"
        );
        assert_eq!(
            sql[3],
            "ALTER TABLE \"public\".\"notas\" DROP CONSTRAINT \"notas_cliente_id_fkey\""
        );
        assert_eq!(
            sql[4],
            "ALTER TABLE \"public\".\"notas\" ADD FOREIGN KEY (\"cliente_id\") REFERENCES \"public\".\"clientes\" (\"id\") ON DELETE SET NULL"
        );
    }

    #[test]
    fn renomear_tabela_vale_para_os_comandos_seguintes() {
        let actions = vec![
            AlterAction::RenameTable {
                name: "anotacoes".into(),
            },
            AlterAction::SetRls { enabled: false },
            AlterAction::SetGrants {
                grant: GrantDef {
                    role: ApiRole::Anon,
                    privileges: vec![Privilege::Select],
                },
            },
        ];
        assert_eq!(
            alter(&ctx(), &structure(), &actions).unwrap(),
            [
                "ALTER TABLE \"public\".\"notas\" RENAME TO \"anotacoes\"",
                "ALTER TABLE \"public\".\"anotacoes\" DISABLE ROW LEVEL SECURITY",
                "REVOKE ALL ON TABLE \"public\".\"anotacoes\" FROM \"anon\"",
                "GRANT SELECT ON TABLE \"public\".\"anotacoes\" TO \"anon\"",
            ]
        );
    }

    #[test]
    fn alteracao_recusa_o_que_nao_faz_sentido() {
        let cases = vec![
            vec![],
            vec![AlterAction::DropColumn {
                name: "nao_existe".into(),
            }],
            vec![AlterAction::AddColumn {
                column: col("texto", "text"),
            }],
            vec![AlterAction::AddColumn {
                column: ColumnDef {
                    primary_key: true,
                    ..col("novo", "text")
                },
            }],
            vec![AlterAction::SetDefault {
                column: "id".into(),
                default: Some("1".into()),
            }],
            vec![AlterAction::SetType {
                column: "texto".into(),
                data_type: "money".into(),
                using: None,
            }],
        ];
        for actions in cases {
            assert!(
                alter(&ctx(), &structure(), &actions).is_err(),
                "{actions:?}"
            );
        }
    }

    #[test]
    fn apagar_com_e_sem_cascade() {
        assert_eq!(
            drop("public", "notas", false),
            ["DROP TABLE \"public\".\"notas\""]
        );
        assert_eq!(
            drop("public", "notas", true),
            ["DROP TABLE \"public\".\"notas\" CASCADE"]
        );
    }
}
