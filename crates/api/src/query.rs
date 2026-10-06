//! Tradução da URL (estilo PostgREST) para SQL parametrizado.
//!
//! Regras de segurança, verificadas nos testes de injeção:
//! - identificadores (tabela, coluna, função) só entram no SQL se existirem no
//!   catálogo, e sempre entre aspas duplas ([`ident`]);
//! - tipos usados em casts vêm do catálogo (`format_type`), nunca da URL;
//! - todo valor vindo do usuário é um parâmetro (`$n`), nunca texto do SQL.
//!
//! Os valores chegam como texto e o Postgres converte para o tipo da coluna
//! (`$1::text::integer`): a validação de tipo é dele, como num literal.

use std::error::Error;

use bytes::BytesMut;
use serde_json::{Map, Value};
use tokio_postgres::types::{IsNull, ToSql, Type, to_sql_checked};

use crate::catalog::{Column, Function, Table};

#[derive(Debug, thiserror::Error)]
pub enum QueryError {
    #[error("{0}")]
    Invalid(String),
}

fn invalid(message: impl Into<String>) -> QueryError {
    QueryError::Invalid(message.into())
}

/// Identificador entre aspas, com aspas internas duplicadas.
pub fn ident(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}

// ------------------------------------------------------------------ parâmetros

#[derive(Debug)]
pub enum Param {
    Text(String),
    Int(i64),
    Json(Value),
}

impl ToSql for Param {
    fn to_sql(
        &self,
        ty: &Type,
        out: &mut BytesMut,
    ) -> Result<IsNull, Box<dyn Error + Sync + Send>> {
        match self {
            Param::Text(v) => v.to_sql_checked(ty, out),
            Param::Int(v) => v.to_sql_checked(ty, out),
            Param::Json(v) => v.to_sql_checked(ty, out),
        }
    }

    fn accepts(_: &Type) -> bool {
        // A checagem real acontece no `to_sql_checked` do valor interno.
        true
    }

    to_sql_checked!();
}

/// SQL em construção + seus parâmetros.
#[derive(Debug, Default)]
pub struct Sql {
    pub text: String,
    pub params: Vec<Param>,
}

impl Sql {
    fn param(&mut self, param: Param) -> String {
        self.params.push(param);
        format!("${}", self.params.len())
    }

    pub fn param_refs(&self) -> Vec<&(dyn ToSql + Sync)> {
        self.params
            .iter()
            .map(|p| p as &(dyn ToSql + Sync))
            .collect()
    }
}

// ------------------------------------------------------------------ requisição

#[derive(Clone, Debug, PartialEq)]
pub enum Select {
    All,
    Columns(Vec<String>),
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum CmpOp {
    Eq,
    Neq,
    Gt,
    Gte,
    Lt,
    Lte,
}

impl CmpOp {
    fn sql(self) -> &'static str {
        match self {
            CmpOp::Eq => "=",
            CmpOp::Neq => "<>",
            CmpOp::Gt => ">",
            CmpOp::Gte => ">=",
            CmpOp::Lt => "<",
            CmpOp::Lte => "<=",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum IsValue {
    Null,
    True,
    False,
    Unknown,
}

#[derive(Clone, Debug, PartialEq)]
enum Op {
    Cmp(CmpOp, String),
    Like { insensitive: bool, pattern: String },
    In(Vec<String>),
    Is(IsValue),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Filter {
    column: String,
    negate: bool,
    op: Op,
}

#[derive(Clone, Debug, PartialEq)]
pub struct OrderTerm {
    column: String,
    desc: bool,
    nulls_first: Option<bool>,
}

/// O que a query string pede: colunas, filtros, ordem e paginação.
#[derive(Clone, Debug, PartialEq)]
pub struct Request {
    pub select: Select,
    pub filters: Vec<Filter>,
    pub order: Vec<OrderTerm>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

fn column<'a>(table: &'a Table, name: &str) -> Result<&'a Column, QueryError> {
    table
        .column(name)
        .ok_or_else(|| invalid(format!("coluna '{name}' não existe em '{}'", table.name)))
}

fn parse_select(value: &str, table: &Table) -> Result<Select, QueryError> {
    let value = value.trim();
    if value == "*" {
        return Ok(Select::All);
    }
    let mut columns = Vec::new();
    for name in value.split(',').map(str::trim) {
        if name.is_empty() {
            return Err(invalid("select vazio ou malformado"));
        }
        if name.contains('(') {
            return Err(invalid("embed de relações em select ainda não é suportado"));
        }
        columns.push(column(table, name)?.name.clone());
    }
    Ok(Select::Columns(columns))
}

fn parse_order(value: &str, table: &Table) -> Result<Vec<OrderTerm>, QueryError> {
    let mut terms = Vec::new();
    for term in value.split(',').map(str::trim) {
        let mut parts = term.split('.');
        let name = parts.next().unwrap_or_default();
        let mut order = OrderTerm {
            column: column(table, name)?.name.clone(),
            desc: false,
            nulls_first: None,
        };
        for modifier in parts {
            match modifier {
                "asc" => order.desc = false,
                "desc" => order.desc = true,
                "nullsfirst" => order.nulls_first = Some(true),
                "nullslast" => order.nulls_first = Some(false),
                other => return Err(invalid(format!("ordem inválida: '{other}'"))),
            }
        }
        terms.push(order);
    }
    Ok(terms)
}

fn parse_non_negative(name: &str, value: &str) -> Result<i64, QueryError> {
    value
        .parse::<i64>()
        .ok()
        .filter(|v| *v >= 0)
        .ok_or_else(|| invalid(format!("{name} precisa ser um inteiro >= 0")))
}

/// `(a,b,"c,d")` → `["a", "b", "c,d"]`. Aspas permitem vírgulas e parênteses;
/// `\"` e `\\` escapam dentro das aspas.
fn parse_in_list(value: &str) -> Result<Vec<String>, QueryError> {
    let inner = value
        .strip_prefix('(')
        .and_then(|v| v.strip_suffix(')'))
        .ok_or_else(|| invalid("in espera uma lista entre parênteses: in.(a,b)"))?;
    if inner.trim().is_empty() {
        return Ok(Vec::new());
    }
    let mut items = Vec::new();
    let mut current = String::new();
    let mut chars = inner.chars().peekable();
    let mut quoted = false;
    let mut was_quoted = false;
    while let Some(c) = chars.next() {
        match c {
            '"' if quoted => quoted = false,
            '"' if current.trim().is_empty() => {
                quoted = true;
                was_quoted = true;
                current.clear();
            }
            '\\' if quoted => {
                if let Some(next) = chars.next() {
                    current.push(next);
                }
            }
            ',' if !quoted => {
                items.push(finish_item(&current, was_quoted));
                current.clear();
                was_quoted = false;
            }
            c => current.push(c),
        }
    }
    if quoted {
        return Err(invalid("aspas não fechadas na lista do in"));
    }
    items.push(finish_item(&current, was_quoted));
    Ok(items)
}

fn finish_item(item: &str, quoted: bool) -> String {
    if quoted {
        item.to_owned()
    } else {
        item.trim().to_owned()
    }
}

fn parse_filter(table: &Table, key: &str, value: &str) -> Result<Filter, QueryError> {
    let col = column(table, key)?;
    let (negate, rest) = match value.strip_prefix("not.") {
        Some(rest) => (true, rest),
        None => (false, value),
    };
    let (op, operand) = rest
        .split_once('.')
        .ok_or_else(|| invalid(format!("filtro inválido em '{key}': use operador.valor")))?;
    let op = match op {
        "eq" => Op::Cmp(CmpOp::Eq, operand.to_owned()),
        "neq" => Op::Cmp(CmpOp::Neq, operand.to_owned()),
        "gt" => Op::Cmp(CmpOp::Gt, operand.to_owned()),
        "gte" => Op::Cmp(CmpOp::Gte, operand.to_owned()),
        "lt" => Op::Cmp(CmpOp::Lt, operand.to_owned()),
        "lte" => Op::Cmp(CmpOp::Lte, operand.to_owned()),
        "like" | "ilike" => Op::Like {
            insensitive: op == "ilike",
            pattern: operand.replace('*', "%"),
        },
        "in" => Op::In(parse_in_list(operand)?),
        "is" => Op::Is(match operand {
            "null" => IsValue::Null,
            "true" => IsValue::True,
            "false" => IsValue::False,
            "unknown" => IsValue::Unknown,
            _ => return Err(invalid("is aceita null, true, false ou unknown")),
        }),
        other => return Err(invalid(format!("operador desconhecido: '{other}'"))),
    };
    Ok(Filter {
        column: col.name.clone(),
        negate,
        op,
    })
}

/// Interpreta a query string. Toda coluna citada é validada contra a tabela.
pub fn parse_request(pairs: &[(String, String)], table: &Table) -> Result<Request, QueryError> {
    let mut request = Request {
        select: Select::All,
        filters: Vec::new(),
        order: Vec::new(),
        limit: None,
        offset: None,
    };
    for (key, value) in pairs {
        match key.as_str() {
            "select" => request.select = parse_select(value, table)?,
            "order" => request.order = parse_order(value, table)?,
            "limit" => request.limit = Some(parse_non_negative("limit", value)?),
            "offset" => request.offset = Some(parse_non_negative("offset", value)?),
            _ => request.filters.push(parse_filter(table, key, value)?),
        }
    }
    Ok(request)
}

// ------------------------------------------------------------------ SQL

fn qualified(schema: &str, table: &Table) -> String {
    format!("{}.{}", ident(schema), ident(&table.name))
}

fn select_list(select: &Select, alias: &str) -> String {
    match select {
        Select::All => format!("{alias}.*"),
        Select::Columns(columns) => columns
            .iter()
            .map(|c| format!("{alias}.{}", ident(c)))
            .collect::<Vec<_>>()
            .join(", "),
    }
}

/// Escapa um elemento de literal de array do Postgres (`{"a","b"}`).
fn array_literal(items: &[String]) -> String {
    let escaped: Vec<String> = items
        .iter()
        .map(|item| format!("\"{}\"", item.replace('\\', "\\\\").replace('"', "\\\"")))
        .collect();
    format!("{{{}}}", escaped.join(","))
}

fn where_clause(sql: &mut Sql, table: &Table, filters: &[Filter]) -> String {
    if filters.is_empty() {
        return String::new();
    }
    let mut conditions = Vec::with_capacity(filters.len());
    for filter in filters {
        let col = table
            .column(&filter.column)
            .expect("coluna validada no parse");
        let target = format!("_t.{}", ident(&col.name));
        let condition = match &filter.op {
            Op::Cmp(op, value) => {
                let p = sql.param(Param::Text(value.clone()));
                format!("{target} {} {p}::text::{}", op.sql(), col.type_name)
            }
            Op::Like {
                insensitive,
                pattern,
            } => {
                let p = sql.param(Param::Text(pattern.clone()));
                let op = if *insensitive { "ILIKE" } else { "LIKE" };
                format!("{target}::text {op} {p}::text")
            }
            Op::In(items) if items.is_empty() => "false".to_owned(),
            Op::In(items) => {
                let p = sql.param(Param::Text(array_literal(items)));
                format!("{target} = ANY({p}::text::{}[])", col.type_name)
            }
            Op::Is(value) => {
                let value = match value {
                    IsValue::Null => "NULL",
                    IsValue::True => "TRUE",
                    IsValue::False => "FALSE",
                    IsValue::Unknown => "UNKNOWN",
                };
                format!("{target} IS {value}")
            }
        };
        if filter.negate {
            conditions.push(format!("NOT ({condition})"));
        } else {
            conditions.push(condition);
        }
    }
    format!(" WHERE {}", conditions.join(" AND "))
}

fn order_clause(order: &[OrderTerm]) -> String {
    if order.is_empty() {
        return String::new();
    }
    let terms: Vec<String> = order
        .iter()
        .map(|o| {
            let mut term = format!(
                "_t.{} {}",
                ident(&o.column),
                if o.desc { "DESC" } else { "ASC" }
            );
            match o.nulls_first {
                Some(true) => term.push_str(" NULLS FIRST"),
                Some(false) => term.push_str(" NULLS LAST"),
                None => {}
            }
            term
        })
        .collect();
    format!(" ORDER BY {}", terms.join(", "))
}

/// `SELECT` que devolve `(json_text, linhas)`; o Postgres monta o JSON.
pub fn select(schema: &str, table: &Table, request: &Request, max_rows: Option<i64>) -> Sql {
    let mut sql = Sql::default();
    let filters = where_clause(&mut sql, table, &request.filters);
    let mut tail = order_clause(&request.order);
    let limit = match (request.limit, max_rows) {
        (Some(l), Some(m)) => Some(l.min(m)),
        (l, m) => l.or(m),
    };
    if let Some(limit) = limit {
        let p = sql.param(Param::Int(limit));
        tail.push_str(&format!(" LIMIT {p}"));
    }
    if let Some(offset) = request.offset {
        let p = sql.param(Param::Int(offset));
        tail.push_str(&format!(" OFFSET {p}"));
    }
    sql.text = format!(
        "SELECT coalesce(json_agg(_r), '[]')::text, count(*) FROM (SELECT {} FROM {} AS _t{filters}{tail}) _r",
        select_list(&request.select, "_t"),
        qualified(schema, table),
    );
    sql
}

/// Total de linhas que os filtros alcançam (para `Prefer: count=exact`).
pub fn count(schema: &str, table: &Table, request: &Request) -> Sql {
    let mut sql = Sql::default();
    let filters = where_clause(&mut sql, table, &request.filters);
    sql.text = format!(
        "SELECT count(*) FROM {} AS _t{filters}",
        qualified(schema, table)
    );
    sql
}

/// Envolve uma escrita para devolver a representação (`Prefer: return=representation`).
fn with_representation(write: String, select: &Select) -> String {
    format!(
        "WITH _w AS ({write} RETURNING _t.*) \
         SELECT coalesce(json_agg(_r), '[]')::text, count(*) FROM (SELECT {} FROM _w AS _t) _r",
        select_list(select, "_t"),
    )
}

/// Colunas de um corpo JSON (objeto ou array de objetos), validadas.
fn body_columns(table: &Table, rows: &[Value]) -> Result<Vec<String>, QueryError> {
    let mut columns: Vec<String> = Vec::new();
    for row in rows {
        let Value::Object(map) = row else {
            return Err(invalid(
                "o corpo deve ser um objeto JSON ou um array de objetos",
            ));
        };
        for key in map.keys() {
            if !columns.iter().any(|c| c == key) {
                columns.push(column(table, key)?.name.clone());
            }
        }
    }
    Ok(columns)
}

/// `INSERT` a partir de JSON: o Postgres converte cada campo para o tipo da
/// coluna (`json_populate_recordset`).
///
/// Colunas ausentes num objeto recebem o DEFAULT, mesmo em lotes com chaves
/// diferentes: as linhas são agrupadas por conjunto de colunas, e cada grupo
/// vira um INSERT (CTEs na mesma instrução, numa transação só).
pub fn insert(
    schema: &str,
    table: &Table,
    body: Value,
    representation: Option<&Select>,
) -> Result<Sql, QueryError> {
    let rows = match body {
        Value::Array(rows) => rows,
        object @ Value::Object(_) => vec![object],
        _ => {
            return Err(invalid(
                "o corpo deve ser um objeto JSON ou um array de objetos",
            ));
        }
    };
    if rows.is_empty() {
        return Err(invalid("nada para inserir"));
    }
    // Agrupa por conjunto de colunas, na ordem das colunas da tabela.
    let mut groups: Vec<(Vec<String>, Vec<Value>)> = Vec::new();
    for row in rows {
        let columns = body_columns(table, std::slice::from_ref(&row))?;
        let ordered: Vec<String> = table
            .columns
            .iter()
            .filter(|c| columns.contains(&c.name))
            .map(|c| c.name.clone())
            .collect();
        match groups.iter_mut().find(|(cols, _)| *cols == ordered) {
            Some((_, group)) => group.push(row),
            None => groups.push((ordered, vec![row])),
        }
    }

    let target = qualified(schema, table);
    let mut sql = Sql::default();
    let mut statements = Vec::new();
    for (columns, rows) in groups {
        if columns.is_empty() {
            // Objetos vazios: uma linha só com DEFAULTs para cada um.
            for _ in rows {
                statements.push(format!("INSERT INTO {target} AS _t DEFAULT VALUES"));
            }
            continue;
        }
        let list = columns
            .iter()
            .map(|c| ident(c))
            .collect::<Vec<_>>()
            .join(", ");
        let p = sql.param(Param::Json(Value::Array(rows)));
        statements.push(format!(
            "INSERT INTO {target} AS _t ({list}) SELECT {list} FROM json_populate_recordset(NULL::{target}, {p}::json)"
        ));
    }

    sql.text = match (statements.len(), representation) {
        (1, Some(select)) => with_representation(statements.remove(0), select),
        (1, None) => statements.remove(0),
        (_, Some(select)) => {
            let ctes: Vec<String> = statements
                .iter()
                .enumerate()
                .map(|(i, s)| format!("_w{i} AS ({s} RETURNING _t.*)"))
                .collect();
            let union: Vec<String> = (0..statements.len())
                .map(|i| format!("SELECT * FROM _w{i}"))
                .collect();
            format!(
                "WITH {} SELECT coalesce(json_agg(_r), '[]')::text, count(*) FROM (SELECT {} FROM ({}) AS _t) _r",
                ctes.join(", "),
                select_list(select, "_t"),
                union.join(" UNION ALL "),
            )
        }
        (n, None) => {
            let ctes: Vec<String> = statements
                .iter()
                .enumerate()
                .map(|(i, s)| format!("_w{i} AS ({s})"))
                .collect();
            format!("WITH {} SELECT {n}", ctes.join(", "))
        }
    };
    Ok(sql)
}

/// `UPDATE` com os campos do objeto JSON, só nas linhas dos filtros (e do RLS).
pub fn update(
    schema: &str,
    table: &Table,
    body: Value,
    filters: &[Filter],
    representation: Option<&Select>,
) -> Result<Sql, QueryError> {
    let Value::Object(map) = body else {
        return Err(invalid("o corpo do PATCH deve ser um objeto JSON"));
    };
    let columns = body_columns(table, &[Value::Object(map.clone())])?;
    if columns.is_empty() {
        return Err(invalid("o corpo do PATCH não tem colunas"));
    }
    let target = qualified(schema, table);
    let mut sql = Sql::default();
    let p = sql.param(Param::Json(Value::Object(map)));
    let assignments = columns
        .iter()
        .map(|c| format!("{0} = _b.{0}", ident(c)))
        .collect::<Vec<_>>()
        .join(", ");
    let filters = where_clause(&mut sql, table, filters);
    let write = format!(
        "UPDATE {target} AS _t SET {assignments} FROM json_populate_record(NULL::{target}, {p}::json) AS _b{filters}"
    );
    sql.text = match representation {
        Some(select) => with_representation(write, select),
        None => write,
    };
    Ok(sql)
}

pub fn delete(
    schema: &str,
    table: &Table,
    filters: &[Filter],
    representation: Option<&Select>,
) -> Sql {
    let mut sql = Sql::default();
    let filters = where_clause(&mut sql, table, filters);
    let write = format!("DELETE FROM {} AS _t{filters}", qualified(schema, table));
    sql.text = match representation {
        Some(select) => with_representation(write, select),
        None => write,
    };
    sql
}

/// Escolhe a sobrecarga cujos argumentos batem com as chaves do corpo.
pub fn resolve_function<'a>(
    candidates: &'a [Function],
    args: &Map<String, Value>,
) -> Result<&'a Function, QueryError> {
    candidates
        .iter()
        .filter(|f| {
            args.keys().all(|k| f.args.iter().any(|a| &a.name == k))
                && f.args
                    .iter()
                    .all(|a| a.has_default || args.contains_key(&a.name))
        })
        .min_by_key(|f| f.args.len())
        .ok_or_else(|| invalid("os argumentos não batem com nenhuma assinatura da função"))
}

/// Chamada de função com argumentos nomeados, convertidos pelo Postgres a
/// partir do JSON (`json_to_record`).
pub fn rpc(schema: &str, function: &Function, args: Map<String, Value>) -> Sql {
    let mut sql = Sql::default();
    let used: Vec<_> = function
        .args
        .iter()
        .filter(|a| args.contains_key(&a.name))
        .collect();
    let call_args = used
        .iter()
        .map(|a| format!("{0} => _a.{0}", ident(&a.name)))
        .collect::<Vec<_>>()
        .join(", ");
    let call = format!("{}.{}({call_args})", ident(schema), ident(&function.name));
    let source = if used.is_empty() {
        None
    } else {
        let definition = used
            .iter()
            .map(|a| format!("{} {}", ident(&a.name), a.type_name))
            .collect::<Vec<_>>()
            .join(", ");
        let p = sql.param(Param::Json(Value::Object(args)));
        Some(format!("json_to_record({p}::json) AS _a({definition})"))
    };
    sql.text = match (function.returns_set, function.returns_void, source) {
        (true, _, Some(source)) => {
            format!("SELECT coalesce(json_agg(_r), '[]')::text FROM {source}, LATERAL {call} AS _r")
        }
        (true, _, None) => format!("SELECT coalesce(json_agg(_r), '[]')::text FROM {call} AS _r"),
        (false, true, Some(source)) => format!("SELECT {call}::text FROM {source}"),
        (false, true, None) => format!("SELECT {call}::text"),
        (false, false, Some(source)) => format!("SELECT to_json({call})::text FROM {source}"),
        (false, false, None) => format!("SELECT to_json({call})::text"),
    };
    sql
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::{Privileges, TableKind};

    fn col(name: &str, type_name: &str) -> Column {
        Column {
            name: name.into(),
            type_name: type_name.into(),
            full_type: type_name.into(),
            category: 'S',
            element_type: None,
            enum_values: vec![],
            nullable: true,
            has_default: false,
            generated: false,
            comment: None,
        }
    }

    fn table() -> Table {
        Table {
            name: "todos".into(),
            kind: TableKind::Table,
            columns: vec![
                col("id", "bigint"),
                col("title", "text"),
                col("done", "boolean"),
            ],
            primary_key: vec!["id".into()],
            foreign_keys: vec![],
            rls_enabled: true,
            rls_forced: false,
            comment: None,
            privileges: [Privileges::default(); 3],
        }
    }

    fn pairs(q: &[(&str, &str)]) -> Vec<(String, String)> {
        q.iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn select_com_filtros_ordem_e_paginacao() {
        let t = table();
        let req = parse_request(
            &pairs(&[
                ("select", "id,title"),
                ("done", "is.false"),
                ("id", "gte.10"),
                ("title", "not.ilike.*compr*"),
                ("order", "id.desc.nullslast"),
                ("limit", "5"),
                ("offset", "10"),
            ]),
            &t,
        )
        .unwrap();
        let sql = select("public", &t, &req, Some(1000));
        assert_eq!(
            sql.text,
            "SELECT coalesce(json_agg(_r), '[]')::text, count(*) FROM (SELECT _t.\"id\", _t.\"title\" \
             FROM \"public\".\"todos\" AS _t WHERE _t.\"done\" IS FALSE AND _t.\"id\" >= $1::text::bigint \
             AND NOT (_t.\"title\"::text ILIKE $2::text) ORDER BY _t.\"id\" DESC NULLS LAST LIMIT $3 OFFSET $4) _r"
        );
        assert_eq!(sql.params.len(), 4);
        assert!(matches!(&sql.params[1], Param::Text(p) if p == "%compr%"));
    }

    #[test]
    fn max_rows_limita_o_limit() {
        let t = table();
        let req = parse_request(&pairs(&[("limit", "5000")]), &t).unwrap();
        let sql = select("public", &t, &req, Some(100));
        assert!(matches!(sql.params[0], Param::Int(100)));
    }

    #[test]
    fn identificadores_desconhecidos_sao_rejeitados() {
        let t = table();
        for q in [
            ("select", "id,\"title\""),
            ("select", "id;drop table todos"),
            ("select", "todos(*)"),
            ("order", "title;drop table todos"),
            ("order", "id.sideways"),
            ("x\"; drop table todos; --", "eq.1"),
            ("title", "eq"),
            ("title", "foo.1"),
            ("limit", "-1"),
            ("limit", "1;drop"),
            ("title", "is.nada"),
            ("title", "in.(a,\"b)"),
        ] {
            assert!(parse_request(&pairs(&[q]), &t).is_err(), "{q:?}");
        }
    }

    #[test]
    fn valores_viram_parametros() {
        let t = table();
        let malicious = "'; DROP TABLE todos; --";
        let req = parse_request(&pairs(&[("title", &format!("eq.{malicious}"))]), &t).unwrap();
        let sql = select("public", &t, &req, None);
        assert!(!sql.text.contains("DROP"));
        assert!(matches!(&sql.params[0], Param::Text(p) if p == malicious));
    }

    #[test]
    fn lista_do_in_com_aspas_e_escapes() {
        assert_eq!(
            parse_in_list(r#"(1, 2,"a,b","x\"y",)"#).unwrap(),
            vec!["1", "2", "a,b", "x\"y", ""]
        );
        assert_eq!(parse_in_list("()").unwrap(), Vec::<String>::new());
        assert_eq!(
            array_literal(&["a\"b".into(), "c\\d".into()]),
            r#"{"a\"b","c\\d"}"#
        );
    }

    #[test]
    fn ident_duplica_aspas() {
        assert_eq!(ident("a\"b"), "\"a\"\"b\"");
    }

    #[test]
    fn insert_rejeita_colunas_desconhecidas() {
        let t = table();
        let body = serde_json::json!({"title": "x", "\"; drop table todos; --": 1});
        assert!(insert("public", &t, body, None).is_err());
        let sql = insert(
            "public",
            &t,
            serde_json::json!([{"title": "a"}, {"title": "b"}]),
            None,
        )
        .unwrap();
        assert!(
            sql.text
                .starts_with("INSERT INTO \"public\".\"todos\" AS _t (\"title\")")
        );
        // Chaves diferentes: um INSERT por grupo, para valer o DEFAULT.
        let sql = insert(
            "public",
            &t,
            serde_json::json!([{"title": "a"}, {"done": true}, {}]),
            None,
        )
        .unwrap();
        assert!(sql.text.starts_with("WITH _w0 AS (INSERT"), "{}", sql.text);
        assert!(sql.text.contains("DEFAULT VALUES"));
        assert_eq!(sql.params.len(), 2);
    }
}
