//! Table DDL: create, alter (a list of actions in one transaction) and drop.

use nelcota_api::query::ident;
use serde::Deserialize;
use serde_json::{Value, json};

use super::{
    DataType, GrantDef, Result, error, expression, grant, invalid, literal, qualified,
    validate_name,
};
use crate::tables::structure::{ColumnInfo, Structure};

/// Context for validating types: the exposed schema and its enums.
pub struct Context<'a> {
    pub schema: &'a str,
    pub enums: &'a [String],
}

#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, ts_rs::TS, schemars::JsonSchema,
)]
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

#[derive(Debug, Clone, Deserialize, ts_rs::TS, schemars::JsonSchema)]
pub struct ReferenceDef {
    pub table: String,
    pub column: String,
    #[serde(default)]
    pub on_delete: OnDelete,
}

const fn yes() -> bool {
    true
}

#[derive(Debug, Clone, Deserialize, ts_rs::TS, schemars::JsonSchema)]
pub struct ColumnDef {
    pub name: String,
    pub data_type: String,
    #[serde(default = "yes")]
    pub nullable: bool,
    /// SQL expression (`now()`, `0`, `'draft'`).
    pub default: Option<String>,
    #[serde(default)]
    pub primary_key: bool,
    /// `GENERATED ALWAYS AS IDENTITY` (integer types only).
    #[serde(default)]
    pub identity: bool,
    #[serde(default)]
    pub unique: bool,
    pub references: Option<ReferenceDef>,
    pub comment: Option<String>,
}

fn references_sql(schema: &str, def: &ReferenceDef) -> Result<String> {
    validate_name("referenced table", &def.table)?;
    validate_name("referenced column", &def.column)?;
    Ok(format!(
        "REFERENCES {} ({}) ON DELETE {}",
        qualified(schema, &def.table),
        ident(&def.column),
        def.on_delete.as_sql()
    ))
}

/// The column definition inside `CREATE TABLE`/`ADD COLUMN`.
/// `inline_pk`: the PK is this column alone (otherwise it is a table constraint).
fn column_sql(ctx: &Context, col: &ColumnDef, inline_pk: bool) -> Result<String> {
    validate_name("column", &col.name)?;
    let data_type = DataType::parse(&col.data_type, ctx.enums)?;
    let mut sql = format!("{} {}", ident(&col.name), data_type.to_sql(ctx.schema));
    if col.identity {
        if !data_type.is_integer() {
            return invalid(
                "identity_requires_integer",
                format!(
                    "column '{}': identity only on smallint, integer or bigint",
                    col.name
                ),
                json!({ "column": col.name }),
            );
        }
        if col.default.is_some() {
            return invalid(
                "identity_with_default",
                format!("column '{}': identity does not take a DEFAULT", col.name),
                json!({ "column": col.name }),
            );
        }
        sql.push_str(" GENERATED ALWAYS AS IDENTITY");
    }
    if inline_pk {
        sql.push_str(" PRIMARY KEY");
    } else if !col.nullable || col.identity || col.primary_key {
        sql.push_str(" NOT NULL");
    }
    if let Some(default) = &col.default {
        sql.push_str(&format!(" DEFAULT {}", expression("DEFAULT", default)?));
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

#[derive(Debug, Clone, Deserialize, ts_rs::TS, schemars::JsonSchema)]
pub struct CreateTable {
    pub name: String,
    pub comment: Option<String>,
    pub columns: Vec<ColumnDef>,
    /// RLS on from creation (the default): without policies, only service_role gets in.
    #[serde(default = "yes")]
    pub rls: bool,
    #[serde(default)]
    pub grants: Vec<GrantDef>,
}

pub fn create(ctx: &Context, spec: &CreateTable) -> Result<Vec<String>> {
    validate_name("table", &spec.name)?;
    if spec.columns.is_empty() {
        return invalid(
            "table_needs_columns",
            "the table needs at least one column",
            Value::Null,
        );
    }
    for (i, col) in spec.columns.iter().enumerate() {
        if spec.columns[..i].iter().any(|c| c.name == col.name) {
            return invalid(
                "duplicate_column",
                format!("column '{}' repeated", col.name),
                json!({ "column": col.name }),
            );
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

/// One change. The whole list runs in one transaction, in the order sent.
#[derive(Debug, Clone, Deserialize, ts_rs::TS, schemars::JsonSchema)]
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
    /// `using`: conversion expression; default `"column"::new_type`.
    SetType {
        column: String,
        data_type: String,
        #[ts(optional = nullable)]
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
    /// Replaces the role's privileges on the table.
    SetGrants {
        grant: GrantDef,
    },
}

/// Column just added to the working copy (constraint names do not exist yet:
/// Postgres only creates them when it runs).
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
) -> Result<&'a crate::tables::structure::ColumnInfo> {
    current.column(name).ok_or_else(|| {
        error(
            "column_not_found",
            format!("column '{name}' does not exist"),
            json!({ "column": name }),
        )
    })
}

/// `initial`: the current structure (validates columns and finds constraint names).
pub fn alter(ctx: &Context, initial: &Structure, actions: &[AlterAction]) -> Result<Vec<String>> {
    if actions.is_empty() {
        return invalid("no_changes", "no changes", Value::Null);
    }
    // Working copy updated after each action: once a column (or the table) is
    // renamed, the following actions use the new name.
    let mut current = initial.clone();
    let mut table = qualified(ctx.schema, &current.name);
    let mut statements = Vec::new();
    for action in actions {
        let target = table.clone();
        let alter = |clause: String| format!("ALTER TABLE {target} {clause}");
        match action {
            AlterAction::RenameTable { name } => {
                validate_name("table", name)?;
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
                    return invalid(
                        "primary_key_on_create_only",
                        "the primary key is set when the table is created",
                        Value::Null,
                    );
                }
                if current.column(&column.name).is_some() {
                    return invalid(
                        "column_already_exists",
                        format!("column '{}' already exists", column.name),
                        json!({ "column": column.name }),
                    );
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
                validate_name("column", to)?;
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
                    return invalid(
                        "column_rejects_default",
                        format!(
                            "column '{column}' is identity/generated: it does not take a DEFAULT"
                        ),
                        json!({ "column": column }),
                    );
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
    use crate::tables::structure::ForeignKeyRef;

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
            name: "notes".into(),
            comment: None,
            columns,
            rls: true,
            grants: vec![],
        }
    }

    #[test]
    fn creates_a_table_with_pk_identity_default_fk_rls_comments_and_grants() {
        let mut table = spec(vec![
            ColumnDef {
                primary_key: true,
                identity: true,
                ..col("id", "bigint")
            },
            ColumnDef {
                nullable: false,
                ..col("body", "text")
            },
            ColumnDef {
                default: Some("now()".into()),
                nullable: false,
                ..col("created_at", "timestamptz")
            },
            ColumnDef {
                unique: true,
                ..col("slug", "varchar(80)")
            },
            ColumnDef {
                references: Some(ReferenceDef {
                    table: "customers".into(),
                    column: "id".into(),
                    on_delete: OnDelete::Cascade,
                }),
                comment: Some("owner of the note".into()),
                ..col("customer_id", "bigint")
            },
        ]);
        table.comment = Some("The team's notes".into());
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
                "CREATE TABLE \"public\".\"notes\" (\n  \
                 \"id\" bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,\n  \
                 \"body\" text NOT NULL,\n  \
                 \"created_at\" timestamptz NOT NULL DEFAULT (now()),\n  \
                 \"slug\" varchar(80) UNIQUE,\n  \
                 \"customer_id\" bigint REFERENCES \"public\".\"customers\" (\"id\") ON DELETE CASCADE\n)",
                "ALTER TABLE \"public\".\"notes\" ENABLE ROW LEVEL SECURITY",
                "COMMENT ON TABLE \"public\".\"notes\" IS 'The team''s notes'",
                "COMMENT ON COLUMN \"public\".\"notes\".\"customer_id\" IS 'owner of the note'",
                "GRANT SELECT, INSERT ON TABLE \"public\".\"notes\" TO \"authenticated\"",
            ]
        );
    }

    #[test]
    fn composite_primary_key_becomes_a_table_constraint() {
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
    fn no_rls_when_asked() {
        let mut table = spec(vec![col("x", "text")]);
        table.rls = false;
        assert_eq!(create(&ctx(), &table).unwrap().len(), 1);
    }

    #[test]
    fn creation_refuses_invalid_specifications() {
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
        let err = create(&ctx(), &spec(vec![col("x", "text"), col("x", "integer")])).unwrap_err();
        assert_eq!(err.code, "duplicate_column");
        assert_eq!(err.params, json!({ "column": "x" }));
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
            name: "notes".into(),
            comment: None,
            rls_enabled: true,
            primary_key: vec!["id".into()],
            primary_key_constraint: Some("notes_pkey".into()),
            columns: vec![
                ColumnInfo {
                    identity: Some("always"),
                    primary_key: true,
                    ..info("id")
                },
                ColumnInfo {
                    unique: Some("notes_slug_key".into()),
                    ..info("slug")
                },
                ColumnInfo {
                    references: Some(ForeignKeyRef {
                        table: "customers".into(),
                        column: "id".into(),
                        on_delete: "no action",
                        constraint: "notes_customer_id_fkey".into(),
                    }),
                    ..info("customer_id")
                },
                info("body"),
            ],
            grants: vec![],
        }
    }

    #[test]
    fn column_changes() {
        let actions = vec![
            AlterAction::AddColumn {
                column: ColumnDef {
                    default: Some("0".into()),
                    ..col("votes", "integer")
                },
            },
            AlterAction::SetType {
                column: "body".into(),
                data_type: "varchar(200)".into(),
                using: None,
            },
            AlterAction::SetNullable {
                column: "body".into(),
                nullable: false,
            },
            AlterAction::SetDefault {
                column: "body".into(),
                default: Some("''".into()),
            },
            AlterAction::RenameColumn {
                from: "body".into(),
                to: "content".into(),
            },
            AlterAction::DropColumn {
                name: "slug".into(),
            },
        ];
        assert_eq!(
            alter(&ctx(), &structure(), &actions).unwrap(),
            [
                "ALTER TABLE \"public\".\"notes\" ADD COLUMN \"votes\" integer DEFAULT (0)",
                "ALTER TABLE \"public\".\"notes\" ALTER COLUMN \"body\" TYPE varchar(200) USING \"body\"::varchar(200)",
                "ALTER TABLE \"public\".\"notes\" ALTER COLUMN \"body\" SET NOT NULL",
                "ALTER TABLE \"public\".\"notes\" ALTER COLUMN \"body\" SET DEFAULT ('')",
                "ALTER TABLE \"public\".\"notes\" RENAME COLUMN \"body\" TO \"content\"",
                "ALTER TABLE \"public\".\"notes\" DROP COLUMN \"slug\"",
            ]
        );
    }

    #[test]
    fn later_actions_see_renamed_added_and_dropped_columns() {
        let actions = vec![
            AlterAction::RenameColumn {
                from: "body".into(),
                to: "content".into(),
            },
            AlterAction::SetNullable {
                column: "content".into(),
                nullable: false,
            },
            AlterAction::AddColumn {
                column: col("votes", "integer"),
            },
            AlterAction::SetDefault {
                column: "votes".into(),
                default: Some("0".into()),
            },
            AlterAction::DropColumn {
                name: "slug".into(),
            },
        ];
        assert_eq!(alter(&ctx(), &structure(), &actions).unwrap().len(), 5);

        let stale = vec![
            AlterAction::RenameColumn {
                from: "body".into(),
                to: "content".into(),
            },
            AlterAction::SetNullable {
                column: "body".into(),
                nullable: false,
            },
        ];
        assert!(
            alter(&ctx(), &structure(), &stale).is_err(),
            "the old name no longer applies"
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
    fn constraints_use_the_current_names() {
        let actions = vec![
            AlterAction::SetUnique {
                column: "slug".into(),
                unique: false,
            },
            AlterAction::SetUnique {
                column: "body".into(),
                unique: true,
            },
            // Already unique: nothing to do.
            AlterAction::SetUnique {
                column: "body".into(),
                unique: true,
            },
            AlterAction::SetReference {
                column: "customer_id".into(),
                reference: Some(ReferenceDef {
                    table: "customers".into(),
                    column: "id".into(),
                    on_delete: OnDelete::SetNull,
                }),
            },
        ];
        let sql = alter(&ctx(), &structure(), &actions).unwrap();
        assert_eq!(
            sql[0],
            "ALTER TABLE \"public\".\"notes\" DROP CONSTRAINT \"notes_slug_key\""
        );
        assert_eq!(
            sql[1],
            "ALTER TABLE \"public\".\"notes\" ADD UNIQUE (\"body\")"
        );
        assert_eq!(
            sql[3],
            "ALTER TABLE \"public\".\"notes\" DROP CONSTRAINT \"notes_customer_id_fkey\""
        );
        assert_eq!(
            sql[4],
            "ALTER TABLE \"public\".\"notes\" ADD FOREIGN KEY (\"customer_id\") REFERENCES \"public\".\"customers\" (\"id\") ON DELETE SET NULL"
        );
    }

    #[test]
    fn renaming_the_table_applies_to_the_following_statements() {
        let actions = vec![
            AlterAction::RenameTable {
                name: "annotations".into(),
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
                "ALTER TABLE \"public\".\"notes\" RENAME TO \"annotations\"",
                "ALTER TABLE \"public\".\"annotations\" DISABLE ROW LEVEL SECURITY",
                "REVOKE ALL ON TABLE \"public\".\"annotations\" FROM \"anon\"",
                "GRANT SELECT ON TABLE \"public\".\"annotations\" TO \"anon\"",
            ]
        );
    }

    #[test]
    fn alter_refuses_what_makes_no_sense() {
        let cases = vec![
            vec![],
            vec![AlterAction::DropColumn {
                name: "missing".into(),
            }],
            vec![AlterAction::AddColumn {
                column: col("body", "text"),
            }],
            vec![AlterAction::AddColumn {
                column: ColumnDef {
                    primary_key: true,
                    ..col("new", "text")
                },
            }],
            vec![AlterAction::SetDefault {
                column: "id".into(),
                default: Some("1".into()),
            }],
            vec![AlterAction::SetType {
                column: "body".into(),
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
    fn drop_with_and_without_cascade() {
        assert_eq!(
            drop("public", "notes", false),
            ["DROP TABLE \"public\".\"notes\""]
        );
        assert_eq!(
            drop("public", "notes", true),
            ["DROP TABLE \"public\".\"notes\" CASCADE"]
        );
    }
}
