//! Páginas do painel: visão geral, editor de tabelas, usuários e policies.
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
    Catalog,
    catalog::{Table, TableKind},
    query,
};
use serde_json::{Map, Value, value::RawValue};
use uuid_like::is_uuid;

use crate::{
    AdminState,
    html::{self, alert, esc, icon, layout, notice, page, url},
};

type Row = HashMap<String, Box<RawValue>>;

/// Formulário separado em valores da chave primária e dados da linha.
struct RowForm {
    pk: HashMap<String, String>,
    data: Map<String, Value>,
}

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

fn table_or_404(state: &AdminState, name: &str) -> Result<Table, PanelError> {
    state
        .catalog
        .get()
        .table(name)
        .cloned()
        .ok_or_else(|| PanelError(format!("tabela '{name}' não existe no schema exposto")))
}

/// Estado do RLS de uma tabela: (classe de cor, rótulo).
fn rls_state(table: &Table, policies: usize) -> (&'static str, String) {
    if table.kind != TableKind::Table {
        return ("", "view".into());
    }
    match (table.rls_enabled, policies) {
        (false, _) if table.exposed_without_rls() => ("danger", "⚠ sem RLS".into()),
        (false, _) => ("", "sem RLS (sem GRANT)".into()),
        (true, 0) => ("warn", "RLS sem policies".into()),
        (true, n) => ("ok", format!("RLS · {n} policies")),
    }
}

fn rls_badge(table: &Table, policies: usize) -> String {
    let (class, label) = rls_state(table, policies);
    format!("<span class=\"badge {class}\">{label}</span>")
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

fn grants(table: &Table, role: usize) -> String {
    let p = table.privileges[role];
    let pills: Vec<String> = [
        (p.select, "SELECT"),
        (p.insert, "INSERT"),
        (p.update, "UPDATE"),
        (p.delete, "DELETE"),
    ]
    .iter()
    .filter(|(on, _)| *on)
    .map(|(_, name)| format!("<span class=\"pill\">{name}</span>"))
    .collect();
    if pills.is_empty() {
        "<span class=\"muted\">—</span>".into()
    } else {
        pills.join(" ")
    }
}

fn exposed_alert(catalog: &Catalog) -> String {
    let names: Vec<String> = catalog
        .tables
        .values()
        .filter(|t| t.exposed_without_rls())
        .map(|t| format!("<code>{}</code>", esc(&t.name)))
        .collect();
    if names.is_empty() {
        return String::new();
    }
    alert(
        "",
        &format!(
            "<strong>Tabelas expostas sem RLS:</strong> {}. Qualquer role com GRANT \
             (inclusive <code>anon</code>, se concedido) lê e escreve <em>todas</em> as linhas. \
             Ative com <code>ALTER TABLE … ENABLE ROW LEVEL SECURITY</code> e crie policies, ou revogue os GRANTs.",
            names.join(", ")
        ),
    )
}

pub async fn dashboard(State(state): State<AdminState>) -> PageResult {
    let catalog = state.catalog.get();
    let policies = policy_counts(&state, &catalog.schema).await?;
    let client = state.db.get().await?;
    let estimates: HashMap<String, i64> = client
        .query(
            "SELECT c.relname::text, c.reltuples::int8 FROM pg_class c
             JOIN pg_namespace n ON n.oid = c.relnamespace WHERE n.nspname = $1",
            &[&catalog.schema],
        )
        .await?
        .iter()
        .map(|r| (r.get(0), r.get(1)))
        .collect();
    // Contagem exata para tabelas pequenas (ou nunca analisadas: reltuples = -1);
    // nas grandes, a estimativa do planejador basta e não custa um seq scan.
    let mut row_counts: HashMap<String, String> = HashMap::new();
    for table in catalog.tables.values() {
        let estimate = estimates.get(&table.name).copied().unwrap_or(-1);
        let text = if table.kind == TableKind::Table && estimate < 10_000 {
            let sql = format!(
                "SELECT count(*) FROM {}.{}",
                query::ident(&catalog.schema),
                query::ident(&table.name)
            );
            match client.query_one(sql.as_str(), &[]).await {
                Ok(row) => row.get::<_, i64>(0).to_string(),
                Err(_) => "—".into(),
            }
        } else if estimate >= 0 {
            format!("≈ {estimate}")
        } else {
            "—".into()
        };
        row_counts.insert(table.name.clone(), text);
    }
    let users: i64 = client
        .query_one("SELECT count(*) FROM auth.users", &[])
        .await?
        .get(0);
    let function_count: usize = catalog.functions.values().map(Vec::len).sum();

    let card = |icon_name: &str, label: &str, value: String| {
        format!(
            "<div class=\"card\"><div class=\"k\">{}{label}</div><div class=\"v\">{value}</div></div>",
            icon(icon_name)
        )
    };
    let mut body = format!(
        "<h1>Visão geral</h1><p class=\"sub\">Schema exposto <code>{}</code> · a API REST, o auth e o painel leem daqui.</p>\
         <div class=\"cards\">{}{}{}{}</div>{}",
        esc(&catalog.schema),
        card("table", "Tabelas e views", catalog.tables.len().to_string()),
        card("users", "Usuários", users.to_string()),
        card(
            "shield",
            "Policies RLS",
            policies.values().sum::<usize>().to_string()
        ),
        card("fn", "Funções (RPC)", function_count.to_string()),
        exposed_alert(&catalog),
    );

    body.push_str(
        "<div class=\"panel\"><div class=\"panel-head\"><h2>Tabelas</h2><div class=\"spacer\"></div>\
         <a class=\"btn\" href=\"/admin/sql\">Nova tabela via SQL</a></div>",
    );
    if catalog.tables.is_empty() {
        body.push_str("<div class=\"empty\">Nenhuma tabela ainda. Crie uma com <code>nelcota migrate</code> ou pelo editor SQL.</div>");
    } else {
        body.push_str("<table><thead><tr><th>Nome</th><th>Linhas</th><th>RLS</th><th>anon</th><th>authenticated</th></tr></thead><tbody>");
        for table in catalog.tables.values() {
            body.push_str(&format!(
                "<tr><td><a class=\"user\" href=\"/admin/tables/{}\">{}{}</a></td><td class=\"mono\">{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
                url(&table.name),
                icon("table"),
                esc(&table.name),
                row_counts.get(&table.name).map_or("—", String::as_str),
                rls_badge(table, policies.get(&table.name).copied().unwrap_or(0)),
                grants(table, 0),
                grants(table, 1),
            ));
        }
        body.push_str("</tbody></table>");
    }
    body.push_str("</div>");
    Ok(page("Visão geral", "/admin/", &body))
}

/// Lista de tabelas na lateral do editor, com a cor do estado do RLS.
fn tables_aside(catalog: &Catalog, policies: &HashMap<String, usize>, active: &str) -> String {
    let mut out = String::from(
        "<aside class=\"editor-aside\"><div class=\"search\">\
         <input id=\"table-search\" type=\"search\" placeholder=\"Buscar tabelas…\" autocomplete=\"off\"></div>\
         <ul id=\"table-list\">",
    );
    for table in catalog.tables.values() {
        let (class, label) = rls_state(table, policies.get(&table.name).copied().unwrap_or(0));
        let current = if table.name == active {
            " class=\"active\""
        } else {
            ""
        };
        out.push_str(&format!(
            "<li data-name=\"{name}\"><a href=\"/admin/tables/{href}\"{current} title=\"{label}\">\
             <i class=\"dot {class}\"></i><span>{name}</span></a></li>",
            name = esc(&table.name),
            href = url(&table.name),
        ));
    }
    out.push_str("</ul></aside>");
    out
}

pub async fn tables_index(State(state): State<AdminState>) -> PageResult {
    let catalog = state.catalog.get();
    let policies = policy_counts(&state, &catalog.schema).await?;
    let body = format!(
        "<div class=\"editor\">{}<div class=\"editor-main\"><div class=\"empty\">\
         <p>Selecione uma tabela à esquerda.</p>\
         <p class=\"muted\">Verde: RLS com policies · amarelo: RLS sem policies · vermelho: exposta sem RLS.</p></div></div></div>",
        tables_aside(&catalog, &policies, "")
    );
    Ok(layout(
        "Editor de tabelas",
        "/admin/tables",
        &["Editor de tabelas"],
        &body,
    ))
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
    let catalog = state.catalog.get();
    let page_number = params.page.max(0);
    let mut pairs = vec![
        ("limit".to_owned(), (PAGE_SIZE + 1).to_string()),
        ("offset".to_owned(), (page_number * PAGE_SIZE).to_string()),
    ];
    if !table.primary_key.is_empty() {
        pairs.push(("order".into(), table.primary_key.join(",")));
    }
    let request = query::parse_request(&pairs, &table)?;
    let sql = query::select(&catalog.schema, &table, &request, None);
    let client = state.db.get().await?;
    let row = client
        .query_one(sql.text.as_str(), &sql.param_refs())
        .await?;
    let rows: Vec<Row> = serde_json::from_str(&row.get::<_, String>(0))?;
    let has_next = rows.len() as i64 > PAGE_SIZE;
    let shown = rows.len().min(PAGE_SIZE as usize);
    let policies = policy_counts(&state, &catalog.schema).await?;

    let editable = !table.primary_key.is_empty() && table.kind != TableKind::MaterializedView;
    let mut main = format!(
        "<div class=\"toolbar\">{}<h1>{}</h1>{}<div class=\"spacer\"></div>",
        icon("table"),
        esc(&table.name),
        rls_badge(&table, policies.get(&table.name).copied().unwrap_or(0)),
    );
    if table.kind != TableKind::MaterializedView {
        main.push_str(&format!(
            "<a class=\"btn primary\" href=\"/admin/tables/{}/new\">{}Inserir linha</a>",
            url(&table.name),
            icon("plus")
        ));
    }
    main.push_str("</div>");

    let mut flash = String::new();
    if let Some(ok) = &params.ok {
        flash.push_str(&notice("ok", ok));
    }
    if let Some(err) = &params.erro {
        flash.push_str(&notice("error", err));
    }
    if table.exposed_without_rls() {
        flash.push_str(&alert(
            "",
            "Esta tabela está exposta sem RLS: quem tem GRANT vê e altera todas as linhas.",
        ));
    }
    if table.primary_key.is_empty() && table.kind == TableKind::Table {
        flash.push_str(&alert(
            "warn",
            "Tabela sem chave primária: as linhas podem ser vistas, mas não editadas pelo painel.",
        ));
    }
    if !flash.is_empty() {
        main.push_str(&format!("<div class=\"flash\">{flash}</div>"));
    }

    main.push_str("<div class=\"grid\"><table><thead><tr>");
    for col in &table.columns {
        let key = if table.primary_key.contains(&col.name) {
            icon("key")
        } else {
            String::new()
        };
        main.push_str(&format!(
            "<th>{key}{}<small>{}</small></th>",
            esc(&col.name),
            esc(&col.full_type)
        ));
    }
    if editable {
        main.push_str("<th></th>");
    }
    main.push_str("</tr></thead><tbody>");
    for map in rows.iter().take(PAGE_SIZE as usize) {
        main.push_str("<tr>");
        for col in &table.columns {
            main.push_str(&format!(
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
            main.push_str(&format!(
                "<td class=\"actions\"><a class=\"link\" href=\"/admin/tables/{name}/edit?{pk}\">Editar</a>\
                 <form method=\"post\" action=\"/admin/tables/{name}/delete\" data-confirm=\"Apagar esta linha?\">{hidden}\
                 <button class=\"link danger\">Apagar</button></form></td>",
                name = url(&table.name),
                pk = esc(&pk_query(&table, map)),
            ));
        }
        main.push_str("</tr>");
    }
    main.push_str("</tbody></table>");
    if shown == 0 {
        main.push_str("<div class=\"empty\">Nenhuma linha.</div>");
    }
    main.push_str(&format!(
        "</div><div class=\"footbar\"><span>{shown} linha(s) nesta página</span><span>página {}</span><div class=\"spacer\"></div><div class=\"pager\">",
        page_number + 1
    ));
    if page_number > 0 {
        main.push_str(&format!(
            "<a class=\"btn ghost\" href=\"/admin/tables/{}?page={}\">← Anterior</a>",
            url(&table.name),
            page_number - 1
        ));
    }
    if has_next {
        main.push_str(&format!(
            "<a class=\"btn ghost\" href=\"/admin/tables/{}?page={}\">Próxima →</a>",
            url(&table.name),
            page_number + 1
        ));
    }
    main.push_str("</div></div>");

    let body = format!(
        "<div class=\"editor\">{}<div class=\"editor-main\">{main}</div></div>",
        tables_aside(&catalog, &policies, &table.name)
    );
    Ok(layout(
        &table.name,
        "/admin/tables",
        &["Editor de tabelas", &table.name],
        &body,
    ))
}

fn row_form(table: &Table, action: &str, values: Option<&Row>, hidden: &str) -> String {
    let mut form = format!("<form method=\"post\" action=\"{action}\" class=\"form\">{hidden}");
    for col in table.columns.iter().filter(|c| !c.generated) {
        let mut hints = vec![esc(&col.full_type)];
        if table.primary_key.contains(&col.name) {
            hints.push("chave".into());
        }
        if col.nullable {
            hints.push("vazio = NULL".into());
        }
        if col.has_default && values.is_none() {
            hints.push("vazio = DEFAULT".into());
        }
        form.push_str(&format!(
            "<div class=\"field\"><label for=\"f-{name}\">{name} <small>{hints}</small></label>\
             <input id=\"f-{name}\" name=\"{name}\" value=\"{value}\"></div>",
            name = esc(&col.name),
            hints = hints.join(" · "),
            value = html::input_value(values.and_then(|v| v.get(&col.name)).map(AsRef::as_ref)),
        ));
    }
    form.push_str(&format!(
        "<div class=\"actions\"><button class=\"btn primary\">Salvar</button>\
         <a class=\"btn ghost\" href=\"/admin/tables/{}\">Cancelar</a></div></form>",
        url(&table.name)
    ));
    form
}

pub async fn new_row(State(state): State<AdminState>, Path(name): Path<String>) -> PageResult {
    let table = table_or_404(&state, &name)?;
    let body = format!(
        "<div class=\"page\"><h1>Inserir linha</h1><p class=\"sub\">em <code>{}</code></p>{}</div>",
        esc(&table.name),
        row_form(
            &table,
            &format!("/admin/tables/{}/insert", url(&table.name)),
            None,
            ""
        )
    );
    Ok(layout(
        "Inserir linha",
        "/admin/tables",
        &["Editor de tabelas", &table.name, "Inserir linha"],
        &body,
    ))
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
        "<div class=\"page\"><h1>Editar linha</h1><p class=\"sub\">em <code>{}</code></p>{}</div>",
        esc(&table.name),
        row_form(
            &table,
            &format!("/admin/tables/{}/update", url(&table.name)),
            Some(&values),
            &hidden
        )
    );
    Ok(layout(
        "Editar linha",
        "/admin/tables",
        &["Editor de tabelas", &table.name, "Editar linha"],
        &body,
    ))
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
            "SELECT u.id::text, u.email, to_char(u.created_at, 'DD/MM/YYYY HH24:MI'),
                    coalesce(to_char(u.last_sign_in_at, 'DD/MM/YYYY HH24:MI'), '—'),
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
        "<h1>Usuários</h1><p class=\"sub\">Usuários finais do <code>/auth/v1</code>. As senhas ficam em \
         <code>auth.users.encrypted_password</code> (PHC argon2id) e nunca são exibidas.</p>{}\
         <div class=\"panel\"><div class=\"panel-head\"><h2>{total} usuário(s)</h2><div class=\"spacer\"></div>\
         <form method=\"get\" class=\"searchbox\"><input name=\"q\" type=\"search\" placeholder=\"Buscar por email\" value=\"{}\">\
         <button class=\"btn\">{}Buscar</button></form></div>",
        params
            .ok
            .as_deref()
            .map(|m| notice("ok", m))
            .unwrap_or_default(),
        esc(&search),
        icon("search"),
    );
    if rows.is_empty() {
        body.push_str("<div class=\"empty\">Nenhum usuário encontrado.</div>");
    } else {
        body.push_str("<table><thead><tr><th>Usuário</th><th>Criado em</th><th>Último login</th><th>Sessões ativas</th><th></th></tr></thead><tbody>");
    }
    for row in rows.iter().take(PAGE_SIZE as usize) {
        let id: String = row.get(0);
        let email: String = row.get(1);
        let initial = email
            .chars()
            .next()
            .map(|c| c.to_uppercase().to_string())
            .unwrap_or_default();
        body.push_str(&format!(
            "<tr><td><div class=\"user\"><span class=\"avatar\">{initial}</span><div>{email}<small>{id}</small></div></div></td>\
             <td>{}</td><td>{}</td><td class=\"mono\">{}</td><td class=\"actions\">\
             <form method=\"post\" action=\"/admin/users/{id}/revoke\" data-confirm=\"Encerrar todas as sessões deste usuário?\"><button class=\"link\">Encerrar sessões</button></form>\
             <form method=\"post\" action=\"/admin/users/{id}/delete\" data-confirm=\"Apagar este usuário? (sessões e dados em cascata)\"><button class=\"link danger\">Apagar</button></form></td></tr>",
            esc(&row.get::<_, String>(2)),
            esc(&row.get::<_, String>(3)),
            row.get::<_, i64>(4),
            initial = esc(&initial),
            email = esc(&email),
            id = esc(&id),
        ));
    }
    if !rows.is_empty() {
        body.push_str("</tbody></table>");
    }
    body.push_str("</div><div class=\"pager\">");
    if page_number > 0 {
        body.push_str(&format!(
            "<a class=\"btn ghost\" href=\"?page={}&q={}\">← Anterior</a>",
            page_number - 1,
            url(&search)
        ));
    }
    if rows.len() as i64 > PAGE_SIZE {
        body.push_str(&format!(
            "<a class=\"btn ghost\" href=\"?page={}&q={}\">Próxima →</a>",
            page_number + 1,
            url(&search)
        ));
    }
    body.push_str("</div>");
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

    let mut body = String::from(
        "<h1>Policies</h1><p class=\"sub\">Row Level Security: quem enxerga e altera cada linha. \
         A API nunca decide permissão; o Postgres decide.</p>",
    );
    let exposed: Vec<&Table> = catalog
        .tables
        .values()
        .filter(|t| t.exposed_without_rls())
        .collect();
    if exposed.is_empty() {
        body.push_str(&notice("ok", "Nenhuma tabela exposta sem RLS."));
    }
    for table in &exposed {
        body.push_str(&alert(
            "",
            &format!(
                "<code>{}</code> está exposta sem RLS: quem tem GRANT vê e altera todas as linhas.",
                esc(&table.name)
            ),
        ));
    }

    for table in catalog
        .tables
        .values()
        .filter(|t| t.kind == TableKind::Table)
    {
        let list = by_table.get(&table.name);
        body.push_str(&format!(
            "<div class=\"panel\"><div class=\"panel-head\">{}<h2>{}</h2>{}<div class=\"spacer\"></div>\
             <a class=\"btn ghost\" href=\"/admin/tables/{}\">Ver dados</a></div>",
            icon("lock"),
            esc(&table.name),
            rls_badge(table, list.map_or(0, Vec::len)),
            url(&table.name),
        ));
        match list {
            None if table.rls_enabled => body.push_str(
                "<div class=\"empty\">RLS ligado e nenhuma policy: só <code>service_role</code> (e o dono) acessam.</div>",
            ),
            None => body.push_str("<div class=\"empty\">Sem policies.</div>"),
            Some(list) => {
                for row in list {
                    let permissive: String = row.get(2);
                    let roles: Vec<String> = row.get(3);
                    let using: String = row.get(5);
                    let check: String = row.get(6);
                    let mut expr = String::new();
                    if !using.is_empty() {
                        expr.push_str(&format!("<div>USING <code>{}</code></div>", esc(&using)));
                    }
                    if !check.is_empty() {
                        expr.push_str(&format!(
                            "<div>WITH CHECK <code>{}</code></div>",
                            esc(&check)
                        ));
                    }
                    body.push_str(&format!(
                        "<div class=\"policy\"><div class=\"name\">{}{}</div>\
                         <div><span class=\"pill\">{}</span> <span class=\"muted\">{}</span></div>\
                         <div class=\"expr\">{expr}</div></div>",
                        esc(&row.get::<_, String>(1)),
                        if permissive == "RESTRICTIVE" {
                            " <span class=\"badge\">restritiva</span>"
                        } else {
                            ""
                        },
                        esc(&row.get::<_, String>(4)),
                        esc(&roles.join(", ")),
                    ));
                }
            }
        }
        body.push_str("</div>");
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
            "<div class=\"panel\"><div class=\"panel-head\">{}<h2>Funções executáveis por anon</h2></div>\
             <div class=\"panel-body\"><p>{}</p><p class=\"muted\">O Postgres concede EXECUTE a PUBLIC por padrão. \
             Para restringir: <code>REVOKE EXECUTE ON FUNCTION f() FROM PUBLIC</code>.</p></div></div>",
            icon("fn"),
            anon_functions.join(" ")
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
