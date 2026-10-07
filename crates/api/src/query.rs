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

use crate::catalog::{Catalog, Column, ForeignKey, Function, Table};

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

/// What `select=` asks for: `*` and/or columns, plus embedded relations.
#[derive(Clone, Debug, PartialEq)]
pub struct Select {
    pub star: bool,
    pub columns: Vec<String>,
    pub embeds: Vec<Embed>,
}

impl Select {
    /// No `select=`: every column, no relations.
    pub fn all() -> Self {
        Select {
            star: true,
            columns: Vec::new(),
            embeds: Vec::new(),
        }
    }
}

/// `(related column, this table's column)` pairs of a foreign key, ANDed.
pub type JoinPairs = Vec<(String, String)>;

/// A related table embedded through a foreign key (`customers(name)`), with
/// its own columns, nested embeds and, through `items.qty=gt.1`-style
/// parameters, its own filters, ordering and paging.
#[derive(Clone, Debug, PartialEq)]
pub struct Embed {
    /// Key of the embedded value in the JSON (the table name or `alias:`).
    pub alias: String,
    /// The related table.
    pub table: String,
    /// `true` when the related table points here (one-to-many, an array);
    /// `false` when this table points there (many-to-one, an object or null).
    pub many: bool,
    pub join: JoinPairs,
    pub select: Select,
    /// Narrow the embedded rows only; the parent rows stay.
    pub filters: Vec<Condition>,
    pub order: Vec<OrderTerm>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

/// Longest identifier Postgres keeps (longer aliases would be cut silently).
const MAX_IDENTIFIER_BYTES: usize = 63;
/// Deepest embedding accepted (`a(b(c(d(*))))`): bounds the recursion and
/// the correlated subqueries a single request can stack.
const MAX_EMBED_DEPTH: usize = 4;

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
    /// The column's type, for the parameter cast.
    type_name: String,
    negate: bool,
    op: Op,
}

/// One filter, or a parenthesized group joined by OR/AND (`or=(...)`,
/// `and=(...)`, optionally negated), nested as a tree.
#[derive(Clone, Debug, PartialEq)]
pub enum Condition {
    Filter(Filter),
    Group {
        /// `true` joins the items with OR, `false` with AND.
        any: bool,
        negate: bool,
        items: Vec<Condition>,
    },
}

/// Deepest nesting accepted in `or=`/`and=`: bounds the recursion on hostile input.
const MAX_DEPTH: usize = 8;
/// Most conditions accepted in one logic tree.
const MAX_CONDITIONS: usize = 100;

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
    /// Joined with AND, like separate query parameters.
    pub filters: Vec<Condition>,
    pub order: Vec<OrderTerm>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
    /// `on_conflict=col1,col2`: the unique key an upsert resolves on.
    pub on_conflict: Option<Vec<String>>,
}

/// What `Prefer: resolution=...` asks a POST to do with a row whose key
/// already exists.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Resolution {
    /// `merge-duplicates`: update the existing row with the sent columns.
    Merge,
    /// `ignore-duplicates`: keep the existing row.
    Ignore,
}

/// `INSERT ... ON CONFLICT (columns)` and what to do on a conflict.
#[derive(Clone, Debug, PartialEq)]
pub struct Upsert {
    pub columns: Vec<String>,
    pub resolution: Resolution,
}

impl Upsert {
    /// Conflict target: `on_conflict` when given, else the primary key.
    pub fn new(
        table: &Table,
        on_conflict: Option<&[String]>,
        resolution: Resolution,
    ) -> Result<Self, QueryError> {
        let columns = match on_conflict {
            Some(columns) => columns.to_vec(),
            None if !table.primary_key.is_empty() => table.primary_key.clone(),
            None => {
                return Err(invalid(
                    "upsert needs a primary key or on_conflict=col1,col2",
                ));
            }
        };
        Ok(Upsert {
            columns,
            resolution,
        })
    }

    /// Merge updates only the columns the row sent, never the key itself;
    /// with nothing left to update, a merge behaves like ignore.
    fn clause(&self, columns: &[String]) -> String {
        let target = self
            .columns
            .iter()
            .map(|c| ident(c))
            .collect::<Vec<_>>()
            .join(", ");
        let updates: Vec<String> = columns
            .iter()
            .filter(|c| !self.columns.contains(c))
            .map(|c| format!("{0} = EXCLUDED.{0}", ident(c)))
            .collect();
        match self.resolution {
            Resolution::Merge if !updates.is_empty() => {
                format!(
                    " ON CONFLICT ({target}) DO UPDATE SET {}",
                    updates.join(", ")
                )
            }
            _ => format!(" ON CONFLICT ({target}) DO NOTHING"),
        }
    }
}

fn column<'a>(table: &'a Table, name: &str) -> Result<&'a Column, QueryError> {
    table.column(name).ok_or_else(|| {
        invalid(format!(
            "column '{name}' does not exist in '{}'",
            table.name
        ))
    })
}

/// `*`, columns and embedded relations: `id,total,customers(name),items(*)`.
/// Relations need the catalog; without it (the panel), they are refused.
/// `depth` is how many embeds this select sits inside.
fn parse_select(
    value: &str,
    table: &Table,
    catalog: Option<&Catalog>,
    depth: usize,
) -> Result<Select, QueryError> {
    let mut select = Select {
        star: false,
        columns: Vec::new(),
        embeds: Vec::new(),
    };
    for item in split_top_level(value)?.into_iter().map(str::trim) {
        if item.is_empty() {
            return Err(invalid("empty or malformed select"));
        }
        if item == "*" {
            select.star = true;
        } else if item.contains('(') {
            let Some(catalog) = catalog else {
                return Err(invalid("embedding relations is not available here"));
            };
            select
                .embeds
                .push(parse_embed(item, table, catalog, depth + 1)?);
        } else if item.contains(':') {
            return Err(invalid(format!(
                "column aliases are not supported: '{item}'"
            )));
        } else {
            select.columns.push(column(table, item)?.name.clone());
        }
    }
    // An embedded key must not collide with a column of the same row.
    let mut keys: Vec<&str> = select.columns.iter().map(String::as_str).collect();
    if select.star {
        keys.extend(table.columns.iter().map(|c| c.name.as_str()));
    }
    for embed in &select.embeds {
        if keys.contains(&embed.alias.as_str()) {
            return Err(invalid(format!(
                "'{}' is already a key of the row: rename the embed with alias:{}(...)",
                embed.alias, embed.table
            )));
        }
        keys.push(&embed.alias);
    }
    Ok(select)
}

/// `[alias:]table[!hint](select)`, where `select` may embed again, at most
/// [`MAX_EMBED_DEPTH`] levels. The hint picks among several foreign keys:
/// the key's column or the constraint name.
fn parse_embed(
    item: &str,
    parent: &Table,
    catalog: &Catalog,
    depth: usize,
) -> Result<Embed, QueryError> {
    if depth > MAX_EMBED_DEPTH {
        return Err(invalid(format!(
            "embeds nested too deep (at most {MAX_EMBED_DEPTH} levels)"
        )));
    }
    let open = item.find('(').unwrap_or_default();
    let inner = item[open + 1..]
        .strip_suffix(')')
        .ok_or_else(|| invalid(format!("malformed embed: '{item}'")))?;
    let head = item[..open].trim();
    let (alias, rest) = match head.split_once(':') {
        Some((alias, rest)) => (Some(alias.trim()), rest.trim()),
        None => (None, head),
    };
    let (name, hint) = match rest.split_once('!') {
        Some((name, hint)) => (name.trim(), Some(hint.trim())),
        None => (rest, None),
    };
    let target = catalog.table(name).ok_or_else(|| {
        invalid(format!(
            "table '{name}' does not exist in the exposed schema"
        ))
    })?;
    let alias = alias.unwrap_or(name);
    if alias.is_empty() || alias.len() > MAX_IDENTIFIER_BYTES {
        return Err(invalid(format!("invalid embed alias: '{alias}'")));
    }
    let (many, join) = resolve_relation(&catalog.schema, parent, target, hint)?;
    if inner.trim().is_empty() {
        return Err(invalid(format!("empty column list in '{item}'")));
    }
    Ok(Embed {
        alias: alias.to_owned(),
        table: target.name.clone(),
        many,
        join,
        select: parse_select(inner, target, Some(catalog), depth)?,
        filters: Vec::new(),
        order: Vec::new(),
        limit: None,
        offset: None,
    })
}

/// The embed a dotted parameter addresses (`items.qty`, `customers.addresses.order`)
/// and the parameter left for it (`qty`, `order`); `None` when the key does
/// not start with an embed's key.
fn embedded_param<'s, 'k>(
    select: &'s mut Select,
    key: &'k str,
) -> Option<(&'s mut Embed, &'k str)> {
    let (head, rest) = key.split_once('.')?;
    let embed = select.embeds.iter_mut().find(|e| e.alias == head)?;
    // A deeper embed takes the parameter when the rest names one; the
    // check runs first so the borrow of `embed` ends on the way back.
    let deeper = rest
        .split_once('.')
        .is_some_and(|(next, _)| embed.select.embeds.iter().any(|e| e.alias == next));
    if deeper {
        embedded_param(&mut embed.select, rest)
    } else {
        Some((embed, rest))
    }
}

/// A filter, logic tree, `order`, `limit` or `offset` aimed at an embed.
fn parse_embedded_param(
    embed: &mut Embed,
    key: &str,
    value: &str,
    catalog: &Catalog,
) -> Result<(), QueryError> {
    let table = catalog
        .table(&embed.table)
        .expect("embedded table resolved while parsing select");
    let paging = matches!(key, "order" | "limit" | "offset");
    if paging && !embed.many {
        return Err(invalid(format!(
            "'{}' is a single row: {key} only applies to embedded arrays",
            embed.alias
        )));
    }
    match key {
        "order" => embed.order = parse_order(value, table)?,
        "limit" => embed.limit = Some(parse_non_negative("limit", value)?),
        "offset" => embed.offset = Some(parse_non_negative("offset", value)?),
        "or" | "and" | "not.or" | "not.and" => embed.filters.push(parse_logic(table, key, value)?),
        "select" | "on_conflict" => {
            return Err(invalid(format!(
                "{key} cannot be set on an embed: write it inside {}(...)",
                embed.alias
            )));
        }
        _ => embed
            .filters
            .push(Condition::Filter(parse_filter(table, key, value)?)),
    }
    Ok(())
}

/// The foreign key linking `parent` and `target`, in either direction.
/// Returns `(many, join pairs)`; no link or several links are errors.
fn resolve_relation(
    schema: &str,
    parent: &Table,
    target: &Table,
    hint: Option<&str>,
) -> Result<(bool, JoinPairs), QueryError> {
    // The same key serves both directions here, so a hint cannot tell parent
    // from children.
    if parent.name == target.name {
        return Err(invalid(format!(
            "embedding '{}' in itself (a self-reference) is not supported yet",
            parent.name
        )));
    }
    let matches_hint = |fk: &ForeignKey| match hint {
        None => true,
        Some(h) => fk.name == h || fk.columns == [h],
    };
    let pairs = |left: &[String], right: &[String]| -> JoinPairs {
        left.iter().cloned().zip(right.iter().cloned()).collect()
    };
    let mut candidates: Vec<(&ForeignKey, bool, JoinPairs)> = Vec::new();
    // This table points there: many-to-one.
    for fk in &parent.foreign_keys {
        if fk.foreign_schema == schema && fk.foreign_table == target.name && matches_hint(fk) {
            candidates.push((fk, false, pairs(&fk.foreign_columns, &fk.columns)));
        }
    }
    // The related table points here: one-to-many.
    for fk in &target.foreign_keys {
        if fk.foreign_schema == schema && fk.foreign_table == parent.name && matches_hint(fk) {
            candidates.push((fk, true, pairs(&fk.columns, &fk.foreign_columns)));
        }
    }
    match candidates.len() {
        0 => Err(invalid(match hint {
            Some(h) => format!(
                "no relationship between '{}' and '{}' matches '{h}'",
                parent.name, target.name
            ),
            None => format!(
                "no foreign key links '{}' and '{}'",
                parent.name, target.name
            ),
        })),
        1 => {
            let (_, many, join) = candidates.remove(0);
            Ok((many, join))
        }
        _ => {
            let options: Vec<String> = candidates
                .iter()
                .map(|(fk, _, _)| match fk.columns.as_slice() {
                    [column] => format!("{}!{column}", target.name),
                    _ => format!("{}!{}", target.name, fk.name),
                })
                .collect();
            Err(invalid(format!(
                "more than one relationship between '{}' and '{}': pick one with {}",
                parent.name,
                target.name,
                options.join(" or ")
            )))
        }
    }
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
        type_name: col.type_name.clone(),
        negate,
        op,
    })
}

/// `or=(a.eq.1,and(b.gt.2,c.lt.3))`, `and=(...)`, `not.or=(...)`: the
/// PostgREST logic tree. `key` is the query parameter (`or`, `not.and`...).
fn parse_logic(table: &Table, key: &str, value: &str) -> Result<Condition, QueryError> {
    let (negate, op) = match key.strip_prefix("not.") {
        Some(op) => (true, op),
        None => (false, key),
    };
    let mut count = 0;
    Ok(Condition::Group {
        any: op == "or",
        negate,
        items: parse_group(table, value, 1, &mut count)?,
    })
}

/// `(item,item,...)` at nesting `depth`.
fn parse_group(
    table: &Table,
    value: &str,
    depth: usize,
    count: &mut usize,
) -> Result<Vec<Condition>, QueryError> {
    if depth > MAX_DEPTH {
        return Err(invalid(format!(
            "or/and nested too deep (at most {MAX_DEPTH} levels)"
        )));
    }
    let inner = value
        .strip_prefix('(')
        .and_then(|v| v.strip_suffix(')'))
        .ok_or_else(|| invalid("or/and expects a parenthesized list: or=(a.eq.1,b.eq.2)"))?;
    let items = split_top_level(inner)?;
    if items.iter().all(|i| i.trim().is_empty()) {
        return Err(invalid("empty or/and list"));
    }
    items
        .into_iter()
        .map(|item| parse_logic_item(table, item.trim(), depth, count))
        .collect()
}

/// A group item: a nested `or(...)`/`and(...)` (maybe `not.`), or a filter
/// written `column.operator.value`.
fn parse_logic_item(
    table: &Table,
    item: &str,
    depth: usize,
    count: &mut usize,
) -> Result<Condition, QueryError> {
    *count += 1;
    if *count > MAX_CONDITIONS {
        return Err(invalid(format!(
            "too many conditions in or/and (at most {MAX_CONDITIONS})"
        )));
    }
    let (negate, rest) = match item.strip_prefix("not.") {
        Some(rest) if rest.starts_with("or(") || rest.starts_with("and(") => (true, rest),
        _ => (false, item),
    };
    for (prefix, any) in [("or", true), ("and", false)] {
        if let Some(group) = rest.strip_prefix(prefix).filter(|g| g.starts_with('(')) {
            return Ok(Condition::Group {
                any,
                negate,
                items: parse_group(table, group, depth + 1, count)?,
            });
        }
    }
    let (column, filter) = item.split_once('.').ok_or_else(|| {
        invalid(format!(
            "invalid condition '{item}': use column.operator.value"
        ))
    })?;
    parse_filter(table, column, &unquote_operand(filter)?).map(Condition::Filter)
}

/// In a logic tree, a value with commas or parentheses is double-quoted:
/// `name.eq."a,b"`. `in` lists keep their own quoting rules.
fn unquote_operand(filter: &str) -> Result<String, QueryError> {
    let (negation, rest) = match filter.strip_prefix("not.") {
        Some(rest) => ("not.", rest),
        None => ("", filter),
    };
    let Some((op, operand)) = rest.split_once('.') else {
        return Ok(filter.to_owned());
    };
    if op == "in" || !operand.starts_with('"') {
        return Ok(filter.to_owned());
    }
    let inner = operand
        .strip_prefix('"')
        .and_then(|v| v.strip_suffix('"'))
        .ok_or_else(|| invalid("unclosed quotes in an or/and value"))?;
    let mut value = String::with_capacity(inner.len());
    let mut chars = inner.chars();
    while let Some(c) = chars.next() {
        match c {
            '\\' => value.extend(chars.next()),
            c => value.push(c),
        }
    }
    Ok(format!("{negation}{op}.{value}"))
}

/// Splits on commas outside parentheses and double quotes (`\` escapes inside
/// quotes). Unbalanced parentheses or quotes are an error.
fn split_top_level(input: &str) -> Result<Vec<&str>, QueryError> {
    let mut parts = Vec::new();
    let mut depth = 0usize;
    let mut quoted = false;
    let mut escaped = false;
    let mut start = 0;
    for (i, c) in input.char_indices() {
        if quoted {
            match c {
                _ if escaped => escaped = false,
                '\\' => escaped = true,
                '"' => quoted = false,
                _ => {}
            }
            continue;
        }
        match c {
            '"' => quoted = true,
            '(' => depth += 1,
            ')' => {
                depth = depth
                    .checked_sub(1)
                    .ok_or_else(|| invalid("unbalanced parentheses in or/and"))?;
            }
            ',' if depth == 0 => {
                parts.push(&input[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    if quoted || depth != 0 {
        return Err(invalid("unbalanced parentheses or quotes in or/and"));
    }
    parts.push(&input[start..]);
    Ok(parts)
}

/// `on_conflict=a,b`: catalog columns, no repeats. Whether they form a unique
/// key is Postgres' call (a mismatch is a 400).
fn parse_on_conflict(value: &str, table: &Table) -> Result<Vec<String>, QueryError> {
    let mut columns: Vec<String> = Vec::new();
    for name in value.split(',').map(str::trim) {
        if name.is_empty() {
            return Err(invalid("empty or malformed on_conflict"));
        }
        let name = column(table, name)?.name.clone();
        if columns.contains(&name) {
            return Err(invalid(format!("column '{name}' repeated in on_conflict")));
        }
        columns.push(name);
    }
    Ok(columns)
}

/// Parses the query string. Every column mentioned is validated against the
/// table; embedded relations are refused (see [`parse_request_with_relations`]).
pub fn parse_request(pairs: &[(String, String)], table: &Table) -> Result<Request, QueryError> {
    parse(pairs, table, None)
}

/// Like [`parse_request`], also resolving embedded relations in `select=`
/// through the catalog's foreign keys.
pub fn parse_request_with_relations(
    pairs: &[(String, String)],
    table: &Table,
    catalog: &Catalog,
) -> Result<Request, QueryError> {
    parse(pairs, table, Some(catalog))
}

fn parse(
    pairs: &[(String, String)],
    table: &Table,
    catalog: Option<&Catalog>,
) -> Result<Request, QueryError> {
    let mut request = Request {
        select: Select::all(),
        filters: Vec::new(),
        order: Vec::new(),
        limit: None,
        offset: None,
        on_conflict: None,
    };
    // `select` first: it defines the embeds that dotted parameters address,
    // wherever it sits in the query string.
    for (_, value) in pairs.iter().filter(|(key, _)| key == "select") {
        request.select = parse_select(value, table, catalog, 0)?;
    }
    for (key, value) in pairs {
        if let Some(catalog) = catalog
            && let Some((embed, rest)) = embedded_param(&mut request.select, key)
        {
            parse_embedded_param(embed, rest, value, catalog)?;
            continue;
        }
        match key.as_str() {
            "select" => {}
            "order" => request.order = parse_order(value, table)?,
            "limit" => request.limit = Some(parse_non_negative("limit", value)?),
            "offset" => request.offset = Some(parse_non_negative("offset", value)?),
            "on_conflict" => request.on_conflict = Some(parse_on_conflict(value, table)?),
            // Logic trees, as in PostgREST: a column named `or`/`and` cannot
            // be filtered directly (it still can inside a tree).
            "or" | "and" | "not.or" | "not.and" => {
                request.filters.push(parse_logic(table, key, value)?);
            }
            _ => request
                .filters
                .push(Condition::Filter(parse_filter(table, key, value)?)),
        }
    }
    Ok(request)
}

// ------------------------------------------------------------------ SQL

fn qualified(schema: &str, table: &Table) -> String {
    format!("{}.{}", ident(schema), ident(&table.name))
}

/// The select list over the row aliased `alias`. `path` numbers the embeds
/// so nested subqueries get distinct aliases (`_e0`, `_e0_1`...).
fn select_list(sql: &mut Sql, schema: &str, select: &Select, alias: &str, path: &str) -> String {
    let mut parts: Vec<String> = Vec::new();
    if select.star {
        parts.push(format!("{alias}.*"));
    }
    parts.extend(
        select
            .columns
            .iter()
            .map(|c| format!("{alias}.{}", ident(c))),
    );
    for (n, embed) in select.embeds.iter().enumerate() {
        let path = if path.is_empty() {
            n.to_string()
        } else {
            format!("{path}_{n}")
        };
        parts.push(embed_sql(sql, schema, embed, alias, &path));
    }
    parts.join(", ")
}

/// Correlated subquery for one embedded relation: an object (or null) for
/// many-to-one, an array for one-to-many. It runs with the request's role,
/// so the related table's RLS filters what comes back.
fn embed_sql(sql: &mut Sql, schema: &str, embed: &Embed, parent: &str, path: &str) -> String {
    let row = format!("_e{path}");
    let json = format!("_j{path}");
    let columns = select_list(sql, schema, &embed.select, &row, path);
    let mut conditions: Vec<String> = embed
        .join
        .iter()
        .map(|(related, own)| format!("{row}.{} = {parent}.{}", ident(related), ident(own)))
        .collect();
    conditions.extend(
        embed
            .filters
            .iter()
            .map(|condition| condition_sql(sql, &row, condition)),
    );
    let mut rows = format!(
        "SELECT {columns} FROM {}.{} AS {row} WHERE {}{}",
        ident(schema),
        ident(&embed.table),
        conditions.join(" AND "),
        order_clause(&row, &embed.order),
    );
    rows.push_str(&paging(sql, embed.limit, embed.offset));
    let value = if embed.many {
        format!("(SELECT coalesce(json_agg({json}), '[]') FROM ({rows}) AS {json})")
    } else {
        // The foreign key references a unique key: at most one row.
        format!("(SELECT to_json({json}) FROM ({rows}) AS {json})")
    };
    format!("{value} AS {}", ident(&embed.alias))
}

/// Escapes an element of a Postgres array literal (`{"a","b"}`).
fn array_literal(items: &[String]) -> String {
    let escaped: Vec<String> = items
        .iter()
        .map(|item| format!("\"{}\"", item.replace('\\', "\\\\").replace('"', "\\\"")))
        .collect();
    format!("{{{}}}", escaped.join(","))
}

/// Filters on the row aliased `_t` (the request's own table).
fn where_clause(sql: &mut Sql, filters: &[Condition]) -> String {
    if filters.is_empty() {
        return String::new();
    }
    let conditions: Vec<String> = filters
        .iter()
        .map(|condition| condition_sql(sql, "_t", condition))
        .collect();
    format!(" WHERE {}", conditions.join(" AND "))
}

/// A tree node on the row aliased `alias`: groups become `(a OR b)` /
/// `(a AND b)`, each wrapped so the precedence never depends on what
/// surrounds it.
fn condition_sql(sql: &mut Sql, alias: &str, condition: &Condition) -> String {
    match condition {
        Condition::Filter(filter) => filter_sql(sql, alias, filter),
        Condition::Group { any, negate, items } => {
            let parts: Vec<String> = items
                .iter()
                .map(|item| condition_sql(sql, alias, item))
                .collect();
            let joined = format!("({})", parts.join(if *any { " OR " } else { " AND " }));
            if *negate {
                format!("NOT {joined}")
            } else {
                joined
            }
        }
    }
}

fn filter_sql(sql: &mut Sql, alias: &str, filter: &Filter) -> String {
    let target = format!("{alias}.{}", ident(&filter.column));
    let type_name = &filter.type_name;
    let condition = match &filter.op {
        Op::Cmp(op, value) => {
            let p = sql.param(Param::Text(value.clone()));
            format!("{target} {} {p}::text::{type_name}", op.sql())
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
            format!("{target} = ANY({p}::text::{type_name}[])")
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
        format!("NOT ({condition})")
    } else {
        condition
    }
}

fn order_clause(alias: &str, order: &[OrderTerm]) -> String {
    if order.is_empty() {
        return String::new();
    }
    let terms: Vec<String> = order
        .iter()
        .map(|o| {
            let mut term = format!(
                "{alias}.{} {}",
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
    let columns = select_list(sql, schema, &request.select, "_t", "");
    let filters = where_clause(sql, &request.filters);
    let order = order_clause("_t", &request.order);
    let limit = match (request.limit, max_rows) {
        (Some(l), Some(m)) => Some(l.min(m)),
        (l, m) => l.or(m),
    };
    let paging = paging(sql, limit, request.offset);
    format!(
        "SELECT {columns} FROM {} AS _t{filters}{order}{paging}",
        qualified(schema, table),
    )
}

/// ` LIMIT $n OFFSET $m`, as far as each is set.
fn paging(sql: &mut Sql, limit: Option<i64>, offset: Option<i64>) -> String {
    let mut tail = String::new();
    if let Some(limit) = limit {
        let p = sql.param(Param::Int(limit));
        tail.push_str(&format!(" LIMIT {p}"));
    }
    if let Some(offset) = offset {
        let p = sql.param(Param::Int(offset));
        tail.push_str(&format!(" OFFSET {p}"));
    }
    tail
}

/// Total rows the filters reach (for `Prefer: count=exact`).
pub fn count(schema: &str, table: &Table, request: &Request) -> Sql {
    let mut sql = Sql::default();
    let filters = where_clause(&mut sql, &request.filters);
    sql.text = format!(
        "SELECT count(*) FROM {} AS _t{filters}",
        qualified(schema, table)
    );
    sql
}

/// Wraps a write to return the representation (`Prefer: return=representation`).
fn with_representation(sql: &mut Sql, schema: &str, write: String, select: &Select) -> String {
    format!(
        "WITH _w AS ({write} RETURNING _t.*) \
         SELECT coalesce(json_agg(_r), '[]')::text, count(*) FROM (SELECT {} FROM _w AS _t) _r",
        select_list(sql, schema, select, "_t", ""),
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
    upsert: Option<&Upsert>,
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
            let conflict = upsert.map(|u| u.clause(&[])).unwrap_or_default();
            for _ in rows {
                statements.push(format!(
                    "INSERT INTO {target} AS _t DEFAULT VALUES{conflict}"
                ));
            }
            continue;
        }
        let list = columns
            .iter()
            .map(|c| ident(c))
            .collect::<Vec<_>>()
            .join(", ");
        let p = sql.param(Param::Json(Value::Array(rows)));
        let conflict = upsert.map(|u| u.clause(&columns)).unwrap_or_default();
        statements.push(format!(
            "INSERT INTO {target} AS _t ({list}) SELECT {list} FROM json_populate_recordset(NULL::{target}, {p}::json){conflict}"
        ));
    }

    sql.text = match (statements.len(), representation) {
        (1, Some(select)) => with_representation(&mut sql, schema, statements.remove(0), select),
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
                select_list(&mut sql, schema, select, "_t", ""),
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
    filters: &[Condition],
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
    let filters = where_clause(&mut sql, filters);
    let write = format!(
        "UPDATE {target} AS _t SET {assignments} FROM json_populate_record(NULL::{target}, {p}::json) AS _b{filters}"
    );
    sql.text = match representation {
        Some(select) => with_representation(&mut sql, schema, write, select),
        None => write,
    };
    Ok(sql)
}

pub fn delete(
    schema: &str,
    table: &Table,
    filters: &[Condition],
    representation: Option<&Select>,
) -> Sql {
    let mut sql = Sql::default();
    let filters = where_clause(&mut sql, filters);
    let write = format!("DELETE FROM {} AS _t{filters}", qualified(schema, table));
    sql.text = match representation {
        Some(select) => with_representation(&mut sql, schema, write, select),
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
        assert!(insert("public", &t, body, None, None).is_err());
        let sql = insert(
            "public",
            &t,
            serde_json::json!([{"title": "a"}, {"title": "b"}]),
            None,
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
            None,
        )
        .unwrap();
        assert!(sql.text.starts_with("WITH _w0 AS (INSERT"), "{}", sql.text);
        assert!(sql.text.contains("DEFAULT VALUES"));
        assert_eq!(sql.params.len(), 2);
    }

    // ------------------------------------------------------------- or / and

    fn where_of(q: &[(&str, &str)]) -> (String, Vec<String>) {
        let t = table();
        let req = parse_request(&pairs(q), &t).unwrap();
        let sql = count("public", &t, &req);
        let params = sql
            .params
            .iter()
            .map(|p| match p {
                Param::Text(v) => v.clone(),
                other => format!("{other:?}"),
            })
            .collect();
        let text = sql
            .text
            .split(" WHERE ")
            .nth(1)
            .unwrap_or_default()
            .to_owned();
        (text, params)
    }

    #[test]
    fn or_joins_its_items_and_ands_with_other_filters() {
        let (text, params) = where_of(&[("or", "(title.eq.a,title.eq.b)"), ("done", "is.false")]);
        assert_eq!(
            text,
            "(_t.\"title\" = $1::text::text OR _t.\"title\" = $2::text::text) AND _t.\"done\" IS FALSE"
        );
        assert_eq!(params, ["a", "b"]);
    }

    #[test]
    fn groups_nest_and_negate() {
        let (text, params) = where_of(&[("or", "(id.eq.1,and(done.is.true,title.not.like.*x*))")]);
        assert_eq!(
            text,
            "(_t.\"id\" = $1::text::bigint OR (_t.\"done\" IS TRUE AND NOT (_t.\"title\"::text LIKE $2::text)))"
        );
        assert_eq!(params, ["1", "%x%"]);

        let (text, _) = where_of(&[("not.and", "(id.gt.1,not.or(done.is.true,id.lt.0))")]);
        assert_eq!(
            text,
            "NOT (_t.\"id\" > $1::text::bigint AND NOT (_t.\"done\" IS TRUE OR _t.\"id\" < $2::text::bigint))"
        );
    }

    #[test]
    fn quoted_values_may_hold_commas_and_parentheses() {
        let (_, params) = where_of(&[(
            "or",
            r#"(title.eq."a,b",title.eq."f(x)",title.eq."say \"hi\"",title.in.("x,y",z))"#,
        )]);
        assert_eq!(params, ["a,b", "f(x)", "say \"hi\"", r#"{"x,y","z"}"#]);
    }

    #[test]
    fn logic_values_stay_parameters() {
        let malicious = "x') OR true; DROP TABLE todos; --";
        let (text, params) = where_of(&[("or", &format!("(title.eq.\"{malicious}\",id.eq.1)"))]);
        assert!(!text.contains("DROP"), "{text}");
        assert_eq!(params[0], malicious);
    }

    #[test]
    fn malformed_or_hostile_trees_are_rejected() {
        let t = table();
        let deep = format!(
            "({}id.eq.1{})",
            "or(".repeat(MAX_DEPTH),
            ")".repeat(MAX_DEPTH)
        );
        let wide = format!("({})", vec!["id.eq.1"; MAX_CONDITIONS + 1].join(","));
        for (key, value) in [
            ("or", "title.eq.a"),                // no parentheses
            ("or", "()"),                        // empty
            ("or", "(title.eq.a,)"),             // empty item
            ("or", "(title.eq.a"),               // unbalanced
            ("or", "(title.eq.\"a)"),            // unclosed quote
            ("or", "(nope.eq.1)"),               // unknown column
            ("or", "(title)"),                   // no operator
            ("or", "(title.eq.a,xor(id.eq.1))"), // not a group keyword
            ("and", "(\"x\"\"; drop table todos; --\".eq.1)"),
            ("or", deep.as_str()),
            ("or", wide.as_str()),
        ] {
            assert!(
                parse_request(&pairs(&[(key, value)]), &t).is_err(),
                "{key}={value}"
            );
        }
        // The deepest accepted tree still parses.
        let ok = format!(
            "({}id.eq.1{})",
            "or(".repeat(MAX_DEPTH - 1),
            ")".repeat(MAX_DEPTH - 1)
        );
        assert!(parse_request(&pairs(&[("or", &ok)]), &t).is_ok());
    }

    // ------------------------------------------------------------- upsert

    fn upsert_sql(body: serde_json::Value, upsert: &Upsert) -> String {
        insert("public", &table(), body, None, Some(upsert))
            .unwrap()
            .text
    }

    #[test]
    fn merge_updates_only_the_sent_columns_never_the_key() {
        let t = table();
        let merge = Upsert::new(&t, None, Resolution::Merge).unwrap();
        assert_eq!(merge.columns, ["id"]);
        let text = upsert_sql(serde_json::json!({"id": 1, "title": "a"}), &merge);
        assert!(
            text.ends_with(" ON CONFLICT (\"id\") DO UPDATE SET \"title\" = EXCLUDED.\"title\""),
            "{text}"
        );
        // Only the key sent: nothing to update, so the row is kept.
        let text = upsert_sql(serde_json::json!({"id": 1}), &merge);
        assert!(text.ends_with(" ON CONFLICT (\"id\") DO NOTHING"), "{text}");
    }

    #[test]
    fn ignore_keeps_the_existing_row_and_groups_each_get_the_clause() {
        let t = table();
        let ignore = Upsert::new(&t, None, Resolution::Ignore).unwrap();
        let text = upsert_sql(
            serde_json::json!([{"id": 1, "title": "a"}, {"id": 2, "done": true}]),
            &ignore,
        );
        assert_eq!(
            text.matches(" ON CONFLICT (\"id\") DO NOTHING").count(),
            2,
            "{text}"
        );
    }

    #[test]
    fn on_conflict_picks_the_key() {
        let t = table();
        let req = parse_request(&pairs(&[("on_conflict", "title, done")]), &t).unwrap();
        let cols = req.on_conflict.unwrap();
        assert_eq!(cols, ["title", "done"]);
        let merge = Upsert::new(&t, Some(&cols), Resolution::Merge).unwrap();
        let text = upsert_sql(
            serde_json::json!({"title": "a", "done": true, "id": 3}),
            &merge,
        );
        assert!(
            text.ends_with(
                " ON CONFLICT (\"title\", \"done\") DO UPDATE SET \"id\" = EXCLUDED.\"id\""
            ),
            "{text}"
        );
    }

    #[test]
    fn bad_upsert_requests_are_rejected() {
        let t = table();
        for value in [
            "",
            "title,",
            "nope",
            "title,title",
            "\"id\"; drop table todos",
        ] {
            assert!(
                parse_request(&pairs(&[("on_conflict", value)]), &t).is_err(),
                "{value}"
            );
        }
        let mut no_pk = table();
        no_pk.primary_key.clear();
        assert!(Upsert::new(&no_pk, None, Resolution::Merge).is_err());
        assert!(Upsert::new(&no_pk, Some(&["title".to_owned()]), Resolution::Merge).is_ok());
    }

    // ------------------------------------------------------------- embedding

    fn fk(name: &str, columns: &[&str], table: &str, foreign: &[&str]) -> ForeignKey {
        ForeignKey {
            name: name.into(),
            columns: columns.iter().map(|c| (*c).into()).collect(),
            foreign_schema: "public".into(),
            foreign_table: table.into(),
            foreign_columns: foreign.iter().map(|c| (*c).into()).collect(),
        }
    }

    fn shop() -> Catalog {
        let mut customers = table();
        customers.name = "customers".into();
        customers.columns = vec![col("id", "bigint"), col("name", "text")];
        let mut orders = table();
        orders.name = "orders".into();
        orders.columns = vec![
            col("id", "bigint"),
            col("buyer_id", "bigint"),
            col("seller_id", "bigint"),
            col("customer_id", "bigint"),
            col("total", "numeric"),
        ];
        orders.foreign_keys = vec![
            fk(
                "orders_customer_id_fkey",
                &["customer_id"],
                "customers",
                &["id"],
            ),
            fk("orders_buyer_id_fkey", &["buyer_id"], "users", &["id"]),
            fk("orders_seller_id_fkey", &["seller_id"], "users", &["id"]),
        ];
        let mut items = table();
        items.name = "items".into();
        items.columns = vec![
            col("id", "bigint"),
            col("order_id", "bigint"),
            col("qty", "integer"),
        ];
        items.foreign_keys = vec![fk("items_order_id_fkey", &["order_id"], "orders", &["id"])];
        let mut users = table();
        users.name = "users".into();
        users.columns = vec![col("id", "bigint"), col("email", "text")];
        Catalog {
            schema: "public".into(),
            tables: [customers, orders, items, users]
                .into_iter()
                .map(|t| (t.name.clone(), t))
                .collect(),
            functions: Default::default(),
        }
    }

    fn select_of(table: &str, select: &str) -> Result<(Select, String), QueryError> {
        let catalog = shop();
        let t = catalog.table(table).unwrap();
        let req = parse_request_with_relations(&pairs(&[("select", select)]), t, &catalog)?;
        let sql = super::select("public", t, &req, None);
        Ok((req.select, sql.text))
    }

    #[test]
    fn embeds_follow_foreign_keys_in_both_directions() {
        let (select, text) = select_of("orders", "id,customers(name),items(*)").unwrap();
        assert_eq!(select.columns, ["id"]);
        let customer = &select.embeds[0];
        assert_eq!(
            (customer.many, &customer.join),
            (false, &vec![("id".to_owned(), "customer_id".to_owned())])
        );
        let items = &select.embeds[1];
        assert_eq!(
            (items.many, &items.join),
            (true, &vec![("order_id".to_owned(), "id".to_owned())])
        );
        assert!(text.contains(
            "(SELECT to_json(_j0) FROM (SELECT _e0.\"name\" FROM \"public\".\"customers\" AS _e0 WHERE _e0.\"id\" = _t.\"customer_id\") AS _j0) AS \"customers\""
        ), "{text}");
        assert!(text.contains(
            "(SELECT coalesce(json_agg(_j1), '[]') FROM (SELECT _e1.* FROM \"public\".\"items\" AS _e1 WHERE _e1.\"order_id\" = _t.\"id\") AS _j1) AS \"items\""
        ), "{text}");
    }

    #[test]
    fn several_keys_need_a_hint_and_aliases_name_the_result() {
        let err = select_of("orders", "users(email)").unwrap_err().to_string();
        assert!(err.contains("users!buyer_id or users!seller_id"), "{err}");
        let (select, _) = select_of(
            "orders",
            "buyer:users!buyer_id(email),seller:users!orders_seller_id_fkey(email)",
        )
        .unwrap();
        let picked: Vec<(&str, &str)> = select
            .embeds
            .iter()
            .map(|e| (e.alias.as_str(), e.join[0].1.as_str()))
            .collect();
        assert_eq!(picked, [("buyer", "buyer_id"), ("seller", "seller_id")]);
    }

    #[test]
    fn bad_embeds_are_rejected() {
        let long = format!("{}:customers(name)", "a".repeat(64));
        for (table, select) in [
            ("orders", "nope(id)"),        // unknown table
            ("customers", "users(email)"), // no foreign key
            ("orders", "customers(nope)"), // unknown column
            (
                "customers",
                "orders(customers(orders(customers(orders(id)))))",
            ), // too deep
            ("orders", "customers(name"),  // unbalanced
            ("orders", "customers()"),     // empty list
            ("orders", "users!nope(email)"), // hint matches nothing
            ("orders", "total,total:customers(name)"), // alias collides with a column
            ("orders", "*,id:customers(name)"),
            ("orders", "customers(name),customers(id)"), // same key twice
            ("orders", "x:customers(n:name)"),           // column alias
            ("orders", long.as_str()),
            ("orders", "orders(id)"), // self-reference
            ("orders", "\"customers\"; drop table x(id)"),
        ] {
            assert!(select_of(table, select).is_err(), "{table}?select={select}");
        }
    }

    fn request_of(table: &str, q: &[(&str, &str)]) -> Result<(Request, Sql), QueryError> {
        let catalog = shop();
        let t = catalog.table(table).unwrap();
        let req = parse_request_with_relations(&pairs(q), t, &catalog)?;
        let sql = super::select("public", t, &req, None);
        Ok((req, sql))
    }

    #[test]
    fn embeds_nest_with_their_own_aliases() {
        let (select, text) = select_of(
            "customers",
            "name,orders(id,items(qty),buyer:users!buyer_id(email))",
        )
        .unwrap();
        let orders = &select.embeds[0];
        assert_eq!(orders.select.columns, ["id"]);
        assert_eq!(orders.select.embeds[0].alias, "items");
        assert!(text.contains(
            "(SELECT coalesce(json_agg(_j0_0), '[]') FROM (SELECT _e0_0.\"qty\" FROM \"public\".\"items\" AS _e0_0 WHERE _e0_0.\"order_id\" = _e0.\"id\") AS _j0_0) AS \"items\""
        ), "{text}");
        assert!(
            text.contains("WHERE _e0_1.\"id\" = _e0.\"buyer_id\") AS _j0_1) AS \"buyer\""),
            "{text}"
        );
        assert!(
            text.contains("WHERE _e0.\"customer_id\" = _t.\"id\""),
            "{text}"
        );
    }

    #[test]
    fn dotted_parameters_filter_order_and_page_the_embed() {
        let (req, sql) = request_of(
            "customers",
            &[
                ("orders.total", "gt.10"),
                ("select", "id,orders(id,items(*))"),
                ("orders.items.or", "(qty.eq.1,qty.gt.5)"),
                ("orders.order", "total.desc"),
                ("orders.limit", "2"),
                ("id", "eq.7"),
            ],
        )
        .unwrap();
        let orders = &req.select.embeds[0];
        assert_eq!((orders.filters.len(), orders.limit), (1, Some(2)));
        assert_eq!(orders.select.embeds[0].filters.len(), 1);
        assert_eq!(req.filters.len(), 1, "the parent keeps only its own filter");
        let text = &sql.text;
        assert!(
            text.contains("WHERE _e0.\"customer_id\" = _t.\"id\" AND _e0.\"total\" > $"),
            "{text}"
        );
        assert!(
            text.contains("::text::numeric ORDER BY _e0.\"total\" DESC LIMIT $"),
            "{text}"
        );
        assert!(
            text.contains("WHERE _e0_0.\"order_id\" = _e0.\"id\" AND (_e0_0.\"qty\" = $"),
            "{text}"
        );
        assert!(text.contains(" WHERE _t.\"id\" = $"), "{text}");
        // Every value is a parameter: 10, 1, 5, 2 and 7.
        assert_eq!(sql.params.len(), 5);
    }

    #[test]
    fn bad_embedded_parameters_are_rejected() {
        for q in [
            [("select", "id,orders(id)"), ("orders.nope", "eq.1")], // unknown column
            [("select", "id,orders(id)"), ("orders.select", "id")], // select belongs inside
            [("select", "id,customers(id)"), ("customers.order", "id")], // single row
            [("select", "id,customers(id)"), ("customers.limit", "1")],
            [("select", "id,orders(id)"), ("orders.limit", "-1")],
            [("select", "id,orders(id)"), ("orders.or", "(total.zz.1)")],
        ] {
            let table = if q[0].1.contains("customers") {
                "orders"
            } else {
                "customers"
            };
            assert!(request_of(table, &q).is_err(), "{q:?}");
        }
        // Without the embed in select, the dotted key is not a column either.
        assert!(request_of("customers", &[("orders.total", "gt.1")]).is_err());
    }

    #[test]
    fn the_panel_parser_refuses_embeds() {
        let catalog = shop();
        let t = catalog.table("orders").unwrap();
        assert!(parse_request(&pairs(&[("select", "customers(name)")]), t).is_err());
    }
}
