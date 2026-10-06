//! Páginas do painel: tabelas (listar/editar), usuários e policies.
//!
//! Escritas em tabelas reutilizam o mesmo construtor de SQL da API
//! (`nelcota_api::query`): identificadores só do catálogo, valores sempre
//! parâmetros.

use std::collections::HashMap;

use axum::{
    Form,
    extract::{Path, Query, State},
    http::StatusCode,
    response::{Html, IntoResponse, Redirect, Response},
};
use nelcota_api::{
    catalog::{Table, TableKind},
    query,
};
use serde_json::{Map, Value, value::RawValue};

type Row = HashMap<String, Box<RawValue>>;

/// Formulário separado em valores da chave primária e dados da linha.
struct RowForm {
    pk: HashMap<String, String>,
    data: Map<String, Value>,
}
use uuid_like::is_uuid;

use crate::{
    AdminState,
    html::{self, esc, notice, page, url},
};

const PAGE_SIZE: i64 = 50;

/// Erro de página: o admin vê a mensagem (é o dono do banco).
pub struct PanelError(String);

impl<E: std::fmt::Display> From<E> for PanelError {
    fn from(err: E) -> Self {
        PanelError(err.to_string())
    }
}

impl IntoResponse for PanelError {
    fn into_response(self) -> Response {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            page("Erro", "", &notice("error", &self.0)),
        )
            .into_response()
    }
}

type PageResult = Result<Html<String>, PanelError>;

fn flash(params: &HashMap<String, String>) -> String {
    let mut out = String::new();
    if let Some(ok) = params.get("ok") {
        out.push_str(&notice("ok", ok));
    }
    if let Some(err) = params.get("erro") {
        out.push_str(&notice("error", err));
    }
    out
}

fn table_or_404(state: &AdminState, name: &str) -> Result<Table, PanelError> {
    state
        .catalog
        .get()
        .table(name)
        .cloned()
        .ok_or_else(|| PanelError(format!("tabela '{name}' não existe no schema exposto")))
}

fn rls_badge(table: &Table, policies: usize) -> String {
    if table.kind != TableKind::Table {
        return "<span class=\"badge\">view</span>".into();
    }
    match (table.rls_enabled, policies) {
        (false, _) if table.exposed_without_rls() => {
            "<span class=\"badge danger\">⚠ sem RLS</span>".into()
        }
        (false, _) => "<span class=\"badge\">sem RLS (sem GRANT)</span>".into(),
        (true, 0) => "<span class=\"badge warn\">RLS sem policies</span>".into(),
        (true, n) => format!("<span class=\"badge ok\">RLS · {n} policies</span>"),
    }
}

async fn policy_counts(
    state: &AdminState,
    schema: &str,
) -> Result<HashMap<String, usize>, PanelError> {
    let client = state.db.get().await?;
    let rows = client
        .query(
            "SELECT tablename::text, count(*) FROM pg_policies WHERE schemaname = $1 GROUP BY 1",
            &[&schema],
        )
        .await?;
    Ok(rows
        .iter()
        .map(|r| (r.get(0), usize::try_from(r.get::<_, i64>(1)).unwrap_or(0)))
        .collect())
}

pub async fn dashboard(State(state): State<AdminState>) -> PageResult {
    let catalog = state.catalog.get();
    let policies = policy_counts(&state, &catalog.schema).await?;
    let client = state.db.get().await?;
    let estimates: HashMap<String, i64> = client
        .query(
            "SELECT c.relname::text, greatest(c.reltuples, 0)::int8 FROM pg_class c
             JOIN pg_namespace n ON n.oid = c.relnamespace WHERE n.nspname = $1",
            &[&catalog.schema],
        )
        .await?
        .iter()
        .map(|r| (r.get(0), r.get(1)))
        .collect();

    let exposed: Vec<&Table> = catalog
        .tables
        .values()
        .filter(|t| t.exposed_without_rls())
        .collect();
    let mut body = format!(
        "<h1>Tabelas <small>schema {}</small></h1>",
        esc(&catalog.schema)
    );
    if !exposed.is_empty() {
        let names: Vec<String> = exposed
            .iter()
            .map(|t| format!("<code>{}</code>", esc(&t.name)))
            .collect();
        body.push_str(&format!(
            "<div class=\"alert\"><strong>⚠ Tabelas expostas sem RLS:</strong> {}. \
             Qualquer role com GRANT (inclusive <code>anon</code>, se concedido) lê/escreve <em>todas</em> as linhas. \
             Ative com <code>ALTER TABLE … ENABLE ROW LEVEL SECURITY</code> e crie policies, ou revogue os GRANTs.</div>",
            names.join(", ")
        ));
    }
    body.push_str("<table><thead><tr><th>Tabela</th><th>Linhas (estim.)</th><th>RLS</th><th>anon</th><th>authenticated</th></tr></thead><tbody>");
    for table in catalog.tables.values() {
        let privs = |i: usize| {
            let p = table.privileges[i];
            let letters: String = [
                (p.select, 'S'),
                (p.insert, 'I'),
                (p.update, 'U'),
                (p.delete, 'D'),
            ]
            .iter()
            .filter(|(on, _)| *on)
            .map(|(_, c)| *c)
            .collect();
            if letters.is_empty() {
                "—".to_owned()
            } else {
                letters
            }
        };
        body.push_str(&format!(
            "<tr><td><a href=\"/admin/tables/{}\">{}</a></td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
            url(&table.name),
            esc(&table.name),
            estimates.get(&table.name).copied().unwrap_or(0),
            rls_badge(table, policies.get(&table.name).copied().unwrap_or(0)),
            privs(0),
            privs(1),
        ));
    }
    body.push_str("</tbody></table><p class=\"muted\">S/I/U/D = GRANTs de SELECT, INSERT, UPDATE, DELETE.</p>");
    Ok(page("Tabelas", "/admin/", &body))
}

#[derive(serde::Deserialize)]
pub struct TableQuery {
    #[serde(default)]
    page: i64,
    ok: Option<String>,
    erro: Option<String>,
}

/// Pares `coluna=eq.valor` da chave primária, a partir de um mapa de valores.
fn pk_filters(
    table: &Table,
    values: &HashMap<String, String>,
) -> Result<Vec<(String, String)>, PanelError> {
    if table.primary_key.is_empty() {
        return Err(PanelError(
            "tabela sem chave primária: edição pelo painel indisponível (use o editor SQL)".into(),
        ));
    }
    table
        .primary_key
        .iter()
        .map(|col| {
            values
                .get(col)
                .map(|v| (col.clone(), format!("eq.{v}")))
                .ok_or_else(|| PanelError(format!("valor da chave '{col}' ausente")))
        })
        .collect()
}

fn pk_query(table: &Table, row: &Row) -> String {
    table
        .primary_key
        .iter()
        .map(|col| {
            let value = html::raw_text(row.get(col).map(AsRef::as_ref)).unwrap_or_default();
            format!("{}={}", url(col), url(&value))
        })
        .collect::<Vec<_>>()
        .join("&")
}

pub async fn table(
    State(state): State<AdminState>,
    Path(name): Path<String>,
    Query(params): Query<TableQuery>,
) -> PageResult {
    let table = table_or_404(&state, &name)?;
    let schema = state.catalog.get().schema.clone();
    let page_number = params.page.max(0);
    let mut pairs = vec![
        ("limit".to_owned(), (PAGE_SIZE + 1).to_string()),
        ("offset".to_owned(), (page_number * PAGE_SIZE).to_string()),
    ];
    if !table.primary_key.is_empty() {
        pairs.push(("order".into(), table.primary_key.join(",")));
    }
    let request = query::parse_request(&pairs, &table)?;
    let sql = query::select(&schema, &table, &request, None);
    let client = state.db.get().await?;
    let row = client
        .query_one(sql.text.as_str(), &sql.param_refs())
        .await?;
    let rows: Vec<Row> = serde_json::from_str(&row.get::<_, String>(0))?;
    let has_next = rows.len() as i64 > PAGE_SIZE;
    let policies = policy_counts(&state, &schema).await?;

    let mut flash_params = HashMap::new();
    if let Some(ok) = params.ok {
        flash_params.insert("ok".to_owned(), ok);
    }
    if let Some(err) = params.erro {
        flash_params.insert("erro".to_owned(), err);
    }
    let mut body = format!(
        "<h1>{} {}</h1>{}",
        esc(&table.name),
        rls_badge(&table, policies.get(&table.name).copied().unwrap_or(0)),
        flash(&flash_params)
    );
    if table.exposed_without_rls() {
        body.push_str("<div class=\"alert\">⚠ Esta tabela está exposta sem RLS: quem tem GRANT vê todas as linhas.</div>");
    }
    let editable = !table.primary_key.is_empty() && table.kind != TableKind::MaterializedView;
    if table.kind != TableKind::MaterializedView {
        body.push_str(&format!(
            "<p><a class=\"button\" href=\"/admin/tables/{}/new\">+ Nova linha</a></p>",
            url(&table.name)
        ));
    }
    body.push_str("<div class=\"scroll\"><table><thead><tr>");
    for col in &table.columns {
        body.push_str(&format!(
            "<th>{}<br><small>{}</small></th>",
            esc(&col.name),
            esc(&col.full_type)
        ));
    }
    if editable {
        body.push_str("<th></th>");
    }
    body.push_str("</tr></thead><tbody>");
    for map in rows.iter().take(PAGE_SIZE as usize) {
        body.push_str("<tr>");
        for col in &table.columns {
            body.push_str(&format!(
                "<td>{}</td>",
                html::cell(map.get(&col.name).map(AsRef::as_ref), 120)
            ));
        }
        if editable {
            let hidden: String = table
                .primary_key
                .iter()
                .map(|col| {
                    format!(
                        "<input type=\"hidden\" name=\"__pk__{}\" value=\"{}\">",
                        esc(col),
                        html::input_value(map.get(col).map(AsRef::as_ref))
                    )
                })
                .collect();
            body.push_str(&format!(
                "<td class=\"actions\"><a href=\"/admin/tables/{name}/edit?{pk}\">editar</a>\
                 <form method=\"post\" action=\"/admin/tables/{name}/delete\" data-confirm=\"Apagar esta linha?\">{hidden}\
                 <button class=\"link danger\">apagar</button></form></td>",
                name = url(&table.name),
                pk = esc(&pk_query(&table, map)),
            ));
        }
        body.push_str("</tr>");
    }
    body.push_str("</tbody></table></div><p class=\"pager\">");
    if page_number > 0 {
        body.push_str(&format!(
            "<a href=\"/admin/tables/{}?page={}\">← anterior</a> ",
            url(&table.name),
            page_number - 1
        ));
    }
    if has_next {
        body.push_str(&format!(
            "<a href=\"/admin/tables/{}?page={}\">próxima →</a>",
            url(&table.name),
            page_number + 1
        ));
    }
    body.push_str("</p>");
    Ok(page(&table.name, "/admin/", &body))
}

fn row_form(table: &Table, action: &str, values: Option<&Row>, hidden: &str) -> String {
    let mut form = format!("<form method=\"post\" action=\"{action}\" class=\"edit\">{hidden}");
    for col in table.columns.iter().filter(|c| !c.generated) {
        let mut hints = vec![esc(&col.full_type)];
        if col.nullable {
            hints.push("vazio = NULL".into());
        }
        if col.has_default && values.is_none() {
            hints.push("vazio = DEFAULT".into());
        }
        form.push_str(&format!(
            "<label>{name} <small>{hints}</small><input name=\"{name}\" value=\"{value}\"></label>",
            name = esc(&col.name),
            hints = hints.join(" · "),
            value = html::input_value(values.and_then(|v| v.get(&col.name)).map(AsRef::as_ref)),
        ));
    }
    form.push_str("<button>Salvar</button></form>");
    form
}

pub async fn new_row(State(state): State<AdminState>, Path(name): Path<String>) -> PageResult {
    let table = table_or_404(&state, &name)?;
    let body = format!(
        "<h1>Nova linha em {}</h1>{}",
        esc(&table.name),
        row_form(
            &table,
            &format!("/admin/tables/{}/insert", url(&table.name)),
            None,
            ""
        )
    );
    Ok(page("Nova linha", "/admin/", &body))
}

pub async fn edit_row(
    State(state): State<AdminState>,
    Path(name): Path<String>,
    Query(params): Query<HashMap<String, String>>,
) -> PageResult {
    let table = table_or_404(&state, &name)?;
    let schema = state.catalog.get().schema.clone();
    let request = query::parse_request(&pk_filters(&table, &params)?, &table)?;
    let sql = query::select(&schema, &table, &request, Some(1));
    let client = state.db.get().await?;
    let row = client
        .query_one(sql.text.as_str(), &sql.param_refs())
        .await?;
    let rows: Vec<Row> = serde_json::from_str(&row.get::<_, String>(0))?;
    let Some(values) = rows.into_iter().next() else {
        return Err(PanelError("linha não encontrada".into()));
    };
    let hidden: String = table
        .primary_key
        .iter()
        .map(|col| {
            format!(
                "<input type=\"hidden\" name=\"__pk__{}\" value=\"{}\">",
                esc(col),
                esc(params.get(col).map_or("", String::as_str))
            )
        })
        .collect();
    let body = format!(
        "<h1>Editar linha de {}</h1>{}",
        esc(&table.name),
        row_form(
            &table,
            &format!("/admin/tables/{}/update", url(&table.name)),
            Some(&values),
            &hidden
        )
    );
    Ok(page("Editar", "/admin/", &body))
}

/// Separa os campos de chave (`__pk__col`) dos dados e converte os dados para
/// JSON: vazio vira NULL (colunas que aceitam NULL) ou é omitido (inserção,
/// para valer o DEFAULT); json/jsonb são interpretados como JSON.
fn split_form(
    table: &Table,
    fields: Vec<(String, String)>,
    inserting: bool,
) -> Result<RowForm, String> {
    let mut pk = HashMap::new();
    let mut data = Map::new();
    for (key, value) in fields {
        if let Some(col) = key.strip_prefix("__pk__") {
            pk.insert(col.to_owned(), value);
            continue;
        }
        let Some(col) = table.column(&key).filter(|c| !c.generated) else {
            return Err(format!("coluna desconhecida: {key}"));
        };
        let json = if value.is_empty() {
            if inserting {
                continue;
            }
            if col.nullable {
                Value::Null
            } else {
                Value::String(value)
            }
        } else if matches!(col.type_name.as_str(), "json" | "jsonb") {
            serde_json::from_str(&value)
                .map_err(|e| format!("{}: JSON inválido ({e})", col.name))?
        } else {
            Value::String(value)
        };
        data.insert(col.name.clone(), json);
    }
    Ok(RowForm { pk, data })
}

fn back(table: &str, result: Result<String, String>) -> Redirect {
    match result {
        Ok(message) => Redirect::to(&format!(
            "/admin/tables/{}?ok={}",
            url(table),
            url(&message)
        )),
        Err(error) => Redirect::to(&format!(
            "/admin/tables/{}?erro={}",
            url(table),
            url(&error)
        )),
    }
}

async fn execute(state: &AdminState, sql: &query::Sql) -> Result<u64, String> {
    let client = state.db.get().await.map_err(|e| e.to_string())?;
    client
        .execute(sql.text.as_str(), &sql.param_refs())
        .await
        .map_err(|e| {
            e.as_db_error()
                .map_or_else(|| e.to_string(), |db| db.message().to_owned())
        })
}

pub async fn insert_row(
    State(state): State<AdminState>,
    Path(name): Path<String>,
    Form(fields): Form<Vec<(String, String)>>,
) -> Result<Redirect, PanelError> {
    let table = table_or_404(&state, &name)?;
    let schema = state.catalog.get().schema.clone();
    let result = async {
        let RowForm { data, .. } = split_form(&table, fields, true)?;
        let sql =
            query::insert(&schema, &table, Value::Object(data), None).map_err(|e| e.to_string())?;
        execute(&state, &sql)
            .await
            .map(|_| "linha inserida".to_owned())
    }
    .await;
    Ok(back(&name, result))
}

pub async fn update_row(
    State(state): State<AdminState>,
    Path(name): Path<String>,
    Form(fields): Form<Vec<(String, String)>>,
) -> Result<Redirect, PanelError> {
    let table = table_or_404(&state, &name)?;
    let schema = state.catalog.get().schema.clone();
    let result = async {
        let RowForm { pk, data } = split_form(&table, fields, false)?;
        let filters = pk_filters(&table, &pk).map_err(|e| e.0)?;
        let request = query::parse_request(&filters, &table).map_err(|e| e.to_string())?;
        let sql = query::update(&schema, &table, Value::Object(data), &request.filters, None)
            .map_err(|e| e.to_string())?;
        execute(&state, &sql)
            .await
            .map(|n| format!("{n} linha(s) atualizada(s)"))
    }
    .await;
    Ok(back(&name, result))
}

pub async fn delete_row(
    State(state): State<AdminState>,
    Path(name): Path<String>,
    Form(fields): Form<Vec<(String, String)>>,
) -> Result<Redirect, PanelError> {
    let table = table_or_404(&state, &name)?;
    let schema = state.catalog.get().schema.clone();
    let result = async {
        let RowForm { pk, .. } = split_form(&table, fields, false)?;
        let filters = pk_filters(&table, &pk).map_err(|e| e.0)?;
        let request = query::parse_request(&filters, &table).map_err(|e| e.to_string())?;
        let sql = query::delete(&schema, &table, &request.filters, None);
        execute(&state, &sql)
            .await
            .map(|n| format!("{n} linha(s) apagada(s)"))
    }
    .await;
    Ok(back(&name, result))
}

#[derive(serde::Deserialize)]
pub struct UsersQuery {
    #[serde(default)]
    page: i64,
    q: Option<String>,
    ok: Option<String>,
}

pub async fn users(
    State(state): State<AdminState>,
    Query(params): Query<UsersQuery>,
) -> PageResult {
    let page_number = params.page.max(0);
    let search = params.q.clone().unwrap_or_default();
    let pattern = format!(
        "%{}%",
        search
            .replace('\\', "\\\\")
            .replace('%', "\\%")
            .replace('_', "\\_")
    );
    let client = state.db.get().await?;
    let rows = client
        .query(
            "SELECT u.id::text, u.email, to_char(u.created_at, 'YYYY-MM-DD HH24:MI'),
                    coalesce(to_char(u.last_sign_in_at, 'YYYY-MM-DD HH24:MI'), '—'),
                    (SELECT count(*) FROM auth.sessions s WHERE s.user_id = u.id AND s.revoked_at IS NULL)
             FROM auth.users u WHERE u.email LIKE $1
             ORDER BY u.created_at DESC LIMIT $2 OFFSET $3",
            &[&pattern, &(PAGE_SIZE + 1), &(page_number * PAGE_SIZE)],
        )
        .await?;
    let total: i64 = client
        .query_one("SELECT count(*) FROM auth.users", &[])
        .await?
        .get(0);

    let mut body = format!(
        "<h1>Usuários <small>{total}</small></h1>{}\
         <form method=\"get\" class=\"inline\"><input name=\"q\" placeholder=\"buscar por email\" value=\"{}\"><button>Buscar</button></form>\
         <table><thead><tr><th>Email</th><th>Criado</th><th>Último login</th><th>Sessões ativas</th><th></th></tr></thead><tbody>",
        params
            .ok
            .as_deref()
            .map(|m| notice("ok", m))
            .unwrap_or_default(),
        esc(&search),
    );
    for row in rows.iter().take(PAGE_SIZE as usize) {
        let id: String = row.get(0);
        body.push_str(&format!(
            "<tr><td>{email}<br><small>{id}</small></td><td>{}</td><td>{}</td><td>{}</td><td class=\"actions\">\
             <form method=\"post\" action=\"/admin/users/{id}/revoke\" data-confirm=\"Encerrar todas as sessões deste usuário?\"><button class=\"link\">encerrar sessões</button></form>\
             <form method=\"post\" action=\"/admin/users/{id}/delete\" data-confirm=\"Apagar este usuário? (sessões e dados em cascata)\"><button class=\"link danger\">apagar</button></form></td></tr>",
            esc(&row.get::<_, String>(2)),
            esc(&row.get::<_, String>(3)),
            row.get::<_, i64>(4),
            email = esc(&row.get::<_, String>(1)),
            id = esc(&id),
        ));
    }
    body.push_str("</tbody></table><p class=\"pager\">");
    if page_number > 0 {
        body.push_str(&format!(
            "<a href=\"?page={}&q={}\">← anterior</a> ",
            page_number - 1,
            url(&search)
        ));
    }
    if rows.len() as i64 > PAGE_SIZE {
        body.push_str(&format!(
            "<a href=\"?page={}&q={}\">próxima →</a>",
            page_number + 1,
            url(&search)
        ));
    }
    body.push_str("</p><p class=\"muted\">Senhas ficam em <code>auth.users.encrypted_password</code> (PHC argon2id) e nunca são exibidas.</p>");
    Ok(page("Usuários", "/admin/users", &body))
}

pub async fn revoke_sessions(
    State(state): State<AdminState>,
    Path(id): Path<String>,
) -> Result<Redirect, PanelError> {
    if !is_uuid(&id) {
        return Err(PanelError("id inválido".into()));
    }
    let client = state.db.get().await?;
    let n = client
        .execute(
            "UPDATE auth.sessions SET revoked_at = now() WHERE user_id = $1::text::uuid AND revoked_at IS NULL",
            &[&id],
        )
        .await?;
    Ok(Redirect::to(&format!(
        "/admin/users?ok={}",
        url(&format!("{n} sessão(ões) encerrada(s)"))
    )))
}

pub async fn delete_user(
    State(state): State<AdminState>,
    Path(id): Path<String>,
) -> Result<Redirect, PanelError> {
    if !is_uuid(&id) {
        return Err(PanelError("id inválido".into()));
    }
    let client = state.db.get().await?;
    client
        .execute("DELETE FROM auth.users WHERE id = $1::text::uuid", &[&id])
        .await?;
    Ok(Redirect::to("/admin/users?ok=usu%C3%A1rio+apagado"))
}

pub async fn policies(State(state): State<AdminState>) -> PageResult {
    let catalog = state.catalog.get();
    let client = state.db.get().await?;
    let rows = client
        .query(
            "SELECT tablename::text, policyname::text, permissive, roles::text[], cmd,
                    coalesce(qual, ''), coalesce(with_check, '')
             FROM pg_policies WHERE schemaname = $1 ORDER BY tablename, policyname",
            &[&catalog.schema],
        )
        .await?;
    let mut by_table: HashMap<String, Vec<&tokio_postgres::Row>> = HashMap::new();
    for row in &rows {
        by_table.entry(row.get(0)).or_default().push(row);
    }

    let mut body = String::from("<h1>Policies RLS</h1>");
    let exposed: Vec<&Table> = catalog
        .tables
        .values()
        .filter(|t| t.exposed_without_rls())
        .collect();
    if exposed.is_empty() {
        body.push_str(&notice("ok", "Nenhuma tabela exposta sem RLS."));
    } else {
        for table in &exposed {
            body.push_str(&format!(
                "<div class=\"alert\">⚠ <code>{}</code> está exposta sem RLS: quem tem GRANT vê e altera todas as linhas.</div>",
                esc(&table.name)
            ));
        }
    }
    for table in catalog
        .tables
        .values()
        .filter(|t| t.kind == TableKind::Table)
    {
        let list = by_table.get(&table.name);
        body.push_str(&format!(
            "<h2>{} {}</h2>",
            esc(&table.name),
            rls_badge(table, list.map_or(0, Vec::len))
        ));
        let Some(list) = list else {
            if table.rls_enabled {
                body.push_str("<p class=\"muted\">RLS ligado e nenhuma policy: só <code>service_role</code> (e o dono) acessam.</p>");
            }
            continue;
        };
        body.push_str("<table><thead><tr><th>Policy</th><th>Comando</th><th>Roles</th><th>USING</th><th>WITH CHECK</th></tr></thead><tbody>");
        for row in list {
            let permissive: String = row.get(2);
            let roles: Vec<String> = row.get(3);
            body.push_str(&format!(
                "<tr><td>{}{}</td><td>{}</td><td>{}</td><td><code>{}</code></td><td><code>{}</code></td></tr>",
                esc(&row.get::<_, String>(1)),
                if permissive == "RESTRICTIVE" { " <span class=\"badge\">restritiva</span>" } else { "" },
                esc(&row.get::<_, String>(4)),
                esc(&roles.join(", ")),
                esc(&row.get::<_, String>(5)),
                esc(&row.get::<_, String>(6)),
            ));
        }
        body.push_str("</tbody></table>");
    }

    let anon_functions: Vec<String> = catalog
        .functions
        .values()
        .flatten()
        .filter(|f| f.executable_by(nelcota_core::Role::Anon))
        .map(|f| format!("<code>{}</code>", esc(&f.name)))
        .collect();
    if !anon_functions.is_empty() {
        body.push_str(&format!(
            "<h2>Funções executáveis por anon</h2><p>{}</p><p class=\"muted\">O Postgres concede EXECUTE a PUBLIC por padrão. \
             Para restringir: <code>REVOKE EXECUTE ON FUNCTION f() FROM PUBLIC</code>.</p>",
            anon_functions.join(", ")
        ));
    }
    Ok(page("Policies", "/admin/policies", &body))
}

mod uuid_like {
    /// Validação de formato de uuid (8-4-4-4-12 hex), sem depender do crate.
    pub fn is_uuid(value: &str) -> bool {
        let parts: Vec<&str> = value.split('-').collect();
        parts.len() == 5
            && parts
                .iter()
                .zip([8, 4, 4, 4, 12])
                .all(|(p, len)| p.len() == len && p.chars().all(|c| c.is_ascii_hexdigit()))
    }
}
