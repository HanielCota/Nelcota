//! Policies panel handlers.
use super::*;

pub async fn policies(
    State(state): State<AdminState>,
) -> ApiResult<crate::contracts::PoliciesData> {
    let catalog = state.catalog.get();
    let client = state.db.get().await?;
    let rows = client
        .query(
            "SELECT tablename::text, policyname::text, permissive, roles::text[], cmd,
                    qual, with_check
             FROM pg_policies WHERE schemaname = $1 ORDER BY tablename, policyname",
            &[&catalog.schema],
        )
        .await?;
    let mut by_table: HashMap<String, Vec<Value>> = HashMap::new();
    for row in &rows {
        by_table.entry(row.get(0)).or_default().push(json!({
            "name": row.get::<_, String>(1),
            "permissive": row.get::<_, String>(2) == "PERMISSIVE",
            "roles": row.get::<_, Vec<String>>(3),
            "command": row.get::<_, String>(4),
            "using": row.get::<_, Option<String>>(5),
            "check": row.get::<_, Option<String>>(6),
        }));
    }
    let tables: Vec<Value> = catalog
        .tables
        .values()
        .filter(|t| t.kind == TableKind::Table)
        .map(|t| {
            let list = by_table.remove(&t.name).unwrap_or_default();
            json!({
                "name": t.name,
                "rls": rls_json(t, list.len()),
                "exposed_without_rls": t.exposed_without_rls(),
                "policies": list,
            })
        })
        .collect();
    let anon_functions: Vec<&str> = catalog
        .functions
        .values()
        .flatten()
        .filter(|f| f.executable_by(nelcota_core::Role::Anon))
        .map(|f| f.name.as_str())
        .collect();
    crate::contracts::response::<crate::contracts::PoliciesData>(json!({
        "schema": catalog.schema,
        "exposed_without_rls": exposed(&catalog),
        "tables": tables,
        "anon_functions": anon_functions,
    }))
}
