//! Translation of the URL (PostgREST style) into parameterized SQL.
//!
//! Security rules, checked by the injection tests:
//! - identifiers (table, column, function) only reach SQL if they exist in the
//!   catalog, and always double-quoted ([`ident`]);
//! - types used in casts come from the catalog (`format_type`), never the URL;
//! - every user-supplied value is a parameter (`$n`), never SQL text.
//!
//! Values arrive as text and Postgres converts them to the column type
//! (`$1::text::integer`): type validation is its job, as with a literal.

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

/// Quoted identifier, with inner quotes doubled (libpq's rules).
pub fn ident(name: &str) -> String {
    postgres_protocol::escape::escape_identifier(name)
}

// ------------------------------------------------------------------ parameters

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
        // The real check happens in the inner value's `to_sql_checked`.
        true
    }

    to_sql_checked!();
}

/// SQL under construction + its parameters.
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

// ------------------------------------------------------------------ request

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

/// What the query string asks for: columns, filters, ordering and paging.
#[derive(Clone, Debug, PartialEq)]
pub struct Request {
    pub select: Select,
    pub filters: Vec<Filter>,
    pub order: Vec<OrderTerm>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

fn column<'a>(table: &'a Table, name: &str) -> Result<&'a Column, QueryError> {
    table.column(name).ok_or_else(|| {
        invalid(format!(
            "column '{name}' does not exist in '{}'",
            table.name
        ))
    })
}

fn parse_select(value: &str, table: &Table) -> Result<Select, QueryError> {
    let value = value.trim();
    if value == "*" {
        return Ok(Select::All);
    }
    let mut columns = Vec::new();
    for name in value.split(',').map(str::trim) {
        if name.is_empty() {
            return Err(invalid("empty or malformed select"));
        }
        if name.contains('(') {
            return Err(invalid(
                "embedding relations in select is not supported yet",
            ));
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
                other => return Err(invalid(format!("invalid order: '{other}'"))),
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
        .ok_or_else(|| invalid(format!("{name} must be an integer >= 0")))
}

/// `(a,b,"c,d")` → `["a", "b", "c,d"]`. Quotes allow commas and parentheses;
/// `\"` and `\\` escape inside quotes.
fn parse_in_list(value: &str) -> Result<Vec<String>, QueryError> {
    let inner = value
        .strip_prefix('(')
        .and_then(|v| v.strip_suffix(')'))
        .ok_or_else(|| invalid("in expects a parenthesized list: in.(a,b)"))?;
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
        return Err(invalid("unclosed quotes in the in list"));
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
        .ok_or_else(|| invalid(format!("invalid filter on '{key}': use operator.value")))?;
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
            _ => return Err(invalid("is accepts null, true, false or unknown")),
        }),
        other => return Err(invalid(format!("unknown operator: '{other}'"))),
    };
    Ok(Filter {
        column: col.name.clone(),
        negate,
        op,
    })
}

/// Parses the query string. Every column mentioned is validated against the table.
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

/// Escapes an element of a Postgres array literal (`{"a","b"}`).
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
            .expect("column validated while parsing");
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

/// `SELECT` returning `(json_text, rows)`; Postgres builds the JSON.
pub fn select(schema: &str, table: &Table, request: &Request, max_rows: Option<i64>) -> Sql {
    let mut sql = Sql::default();
    let rows = rows_subquery(&mut sql, schema, table, request, max_rows);
    sql.text = format!("SELECT coalesce(json_agg(_r), '[]')::text, count(*) FROM ({rows}) _r");
    sql
}

/// One record per result row, as JSON text: for streaming reads (export)
/// without building the whole response in memory.
pub fn select_rows(schema: &str, table: &Table, request: &Request) -> Sql {
    let mut sql = Sql::default();
    let rows = rows_subquery(&mut sql, schema, table, request, None);
    sql.text = format!("SELECT row_to_json(_r)::text FROM ({rows}) _r");
    sql
}

/// Columns, filters, ordering and paging of the table (alias `_t`).
fn rows_subquery(
    sql: &mut Sql,
    schema: &str,
    table: &Table,
    request: &Request,
    max_rows: Option<i64>,
) -> String {
    let filters = where_clause(sql, table, &request.filters);
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
    format!(
        "SELECT {} FROM {} AS _t{filters}{tail}",
        select_list(&request.select, "_t"),
        qualified(schema, table),
    )
}

/// Total rows the filters reach (for `Prefer: count=exact`).
pub fn count(schema: &str, table: &Table, request: &Request) -> Sql {
    let mut sql = Sql::default();
    let filters = where_clause(&mut sql, table, &request.filters);
    sql.text = format!(
        "SELECT count(*) FROM {} AS _t{filters}",
        qualified(schema, table)
    );
    sql
}

/// Wraps a write to return the representation (`Prefer: return=representation`).
fn with_representation(write: String, select: &Select) -> String {
    format!(
        "WITH _w AS ({write} RETURNING _t.*) \
         SELECT coalesce(json_agg(_r), '[]')::text, count(*) FROM (SELECT {} FROM _w AS _t) _r",
        select_list(select, "_t"),
    )
}

/// Columns of a JSON body (object or array of objects), validated.
fn body_columns(table: &Table, rows: &[Value]) -> Result<Vec<String>, QueryError> {
    let mut columns: Vec<String> = Vec::new();
    for row in rows {
        let Value::Object(map) = row else {
            return Err(invalid(
                "the body must be a JSON object or an array of objects",
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

/// `INSERT` from JSON: Postgres converts each field to the column type
/// (`json_populate_recordset`).
///
/// Columns missing from an object get the DEFAULT, even in batches with
/// different keys: rows are grouped by column set, and each group becomes an
/// INSERT (CTEs in the same statement, in a single transaction).
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
                "the body must be a JSON object or an array of objects",
            ));
        }
    };
    if rows.is_empty() {
        return Err(invalid("nothing to insert"));
    }
    // Group by column set, in the table's column order.
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
            // Empty objects: one all-DEFAULT row for each.
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

/// `UPDATE` with the JSON object's fields, only on the rows the filters (and RLS) allow.
pub fn update(
    schema: &str,
    table: &Table,
    body: Value,
    filters: &[Filter],
    representation: Option<&Select>,
) -> Result<Sql, QueryError> {
    let Value::Object(map) = body else {
        return Err(invalid("the PATCH body must be a JSON object"));
    };
    let columns = body_columns(table, &[Value::Object(map.clone())])?;
    if columns.is_empty() {
        return Err(invalid("the PATCH body has no columns"));
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

/// Picks the overload whose arguments match the body's keys.
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
        .ok_or_else(|| invalid("the arguments match no signature of the function"))
}

/// Function call with named arguments, converted by Postgres from the JSON
/// (`json_to_record`).
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
    fn select_with_filters_order_and_paging() {
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
    fn select_rows_returns_one_json_record_per_row() {
        let t = table();
        let req = parse_request(&pairs(&[("done", "eq.true"), ("order", "id")]), &t).unwrap();
        let sql = select_rows("public", &t, &req);
        assert_eq!(
            sql.text,
            "SELECT row_to_json(_r)::text FROM (SELECT _t.* FROM \"public\".\"todos\" AS _t \
             WHERE _t.\"done\" = $1::text::boolean ORDER BY _t.\"id\" ASC) _r"
        );
        assert_eq!(sql.params.len(), 1);
    }

    #[test]
    fn max_rows_caps_the_limit() {
        let t = table();
        let req = parse_request(&pairs(&[("limit", "5000")]), &t).unwrap();
        let sql = select("public", &t, &req, Some(100));
        assert!(matches!(sql.params[0], Param::Int(100)));
    }

    #[test]
    fn unknown_identifiers_are_rejected() {
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
            ("title", "is.nothing"),
            ("title", "in.(a,\"b)"),
        ] {
            assert!(parse_request(&pairs(&[q]), &t).is_err(), "{q:?}");
        }
    }

    #[test]
    fn values_become_parameters() {
        let t = table();
        let malicious = "'; DROP TABLE todos; --";
        let req = parse_request(&pairs(&[("title", &format!("eq.{malicious}"))]), &t).unwrap();
        let sql = select("public", &t, &req, None);
        assert!(!sql.text.contains("DROP"));
        assert!(matches!(&sql.params[0], Param::Text(p) if p == malicious));
    }

    #[test]
    fn in_list_with_quotes_and_escapes() {
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
    fn ident_doubles_quotes() {
        assert_eq!(ident("a\"b"), "\"a\"\"b\"");
        // Backslashes and non-ASCII pass through untouched (no E'' form for identifiers).
        assert_eq!(ident("a\\b"), "\"a\\b\"");
        assert_eq!(ident("café"), "\"café\"");
        assert_eq!(ident(""), "\"\"");
        assert_eq!(
            ident("x\"; drop table t; --"),
            "\"x\"\"; drop table t; --\""
        );
    }

    #[test]
    fn insert_rejects_unknown_columns() {
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
        // Different keys: one INSERT per group, so the DEFAULT applies.
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
