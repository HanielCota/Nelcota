//! Exportação de tabela em CSV ou JSON, lida do Postgres em fluxo: um
//! registro por vez vai direto para a resposta, sem montar o arquivo inteiro
//! na memória. Respeita a mesma ordem e os mesmos filtros da grade.

use std::{error::Error, future::ready};

use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::header,
    response::Response,
};
use futures_util::{StreamExt, stream};
use nelcota_api::query;
use serde::Deserialize;

use crate::{
    AdminState, ApiError,
    api::{Row, RowsQuery, build_request, raw_text, table_or_404, user_query_error},
};

type BoxError = Box<dyn Error + Send + Sync>;

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Format {
    Csv,
    Json,
}

impl Format {
    fn content_type(self) -> &'static str {
        match self {
            Format::Csv => "text/csv; charset=utf-8",
            Format::Json => "application/json",
        }
    }

    fn extension(self) -> &'static str {
        match self {
            Format::Csv => "csv",
            Format::Json => "json",
        }
    }
}

#[derive(Deserialize)]
pub struct ExportQuery {
    format: Format,
    sort: Option<String>,
    #[serde(default)]
    desc: bool,
    filters: Option<String>,
}

/// Transforma cada registro (texto JSON vindo do `row_to_json`) num pedaço
/// do arquivo.
struct Encoder {
    format: Format,
    columns: Vec<String>,
}

impl Encoder {
    fn header(&self) -> String {
        match self.format {
            // BOM: o Excel só reconhece UTF-8 (acentos) com ele.
            Format::Csv => format!(
                "\u{feff}{}",
                csv_line(self.columns.iter().map(|c| Some(c.as_str())))
            ),
            Format::Json => "[".to_owned(),
        }
    }

    fn row(&self, index: usize, json: &str) -> Result<String, BoxError> {
        match self.format {
            Format::Csv => {
                let row: Row = serde_json::from_str(json)?;
                let values: Vec<Option<String>> = self
                    .columns
                    .iter()
                    .map(|c| raw_text(row.get(c).map(AsRef::as_ref)))
                    .collect();
                Ok(csv_line(values.iter().map(Option::as_deref)))
            }
            // O texto do Postgres vai intacto: numeric não perde casas.
            Format::Json => Ok(format!("{}\n  {json}", if index == 0 { "" } else { "," })),
        }
    }

    fn footer(&self) -> &'static str {
        match self.format {
            Format::Csv => "",
            Format::Json => "\n]\n",
        }
    }
}

/// Uma linha CSV (RFC 4180). `None` = NULL, exportado como campo vazio.
fn csv_line<'a>(values: impl Iterator<Item = Option<&'a str>>) -> String {
    let mut line = values
        .map(|v| v.map_or_else(String::new, csv_field))
        .collect::<Vec<_>>()
        .join(",");
    line.push_str("\r\n");
    line
}

fn csv_field(value: &str) -> String {
    if value.contains([',', '"', '\n', '\r']) || value.starts_with(' ') || value.ends_with(' ') {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_owned()
    }
}

/// Nome de arquivo seguro para o `Content-Disposition`.
fn file_name(table: &str, format: Format) -> String {
    let base: String = table
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let base = if base.trim_matches('_').is_empty() {
        "tabela".to_owned()
    } else {
        base
    };
    format!("{base}.{}", format.extension())
}

pub async fn export(
    State(state): State<AdminState>,
    Path(name): Path<String>,
    Query(params): Query<ExportQuery>,
) -> Result<Response, ApiError> {
    let table = table_or_404(&state, &name)?;
    let schema = state.catalog.get().schema.clone();
    let rows_query = RowsQuery {
        sort: params.sort,
        desc: params.desc,
        filters: params.filters,
    };
    let request = build_request(&table, &rows_query, None)?;
    let sql = query::select_rows(&schema, &table, &request);

    // Erros de filtro aparecem aqui, antes do primeiro byte: viram 400.
    let client = state.db.get().await?;
    let rows = client
        .query_raw(sql.text.as_str(), sql.param_refs())
        .await
        .map_err(user_query_error)?;

    let encoder = Encoder {
        format: params.format,
        columns: table.columns.iter().map(|c| c.name.clone()).collect(),
    };
    let header = encoder.header();
    let footer = encoder.footer();
    let table_name = table.name.clone();
    let body = rows.enumerate().map(move |(index, row)| {
        let json: String = row?.try_get(0)?;
        encoder.row(index, &json)
    });
    let stream = stream::once(ready(Ok::<_, BoxError>(header)))
        .chain(body)
        // A conexão fica com o fluxo até o fim e só então volta ao pool.
        .chain(stream::once(async move {
            drop(client);
            Ok(footer.to_owned())
        }))
        .inspect(move |chunk| {
            if let Err(err) = chunk {
                tracing::warn!(table = %table_name, error = %err, "exportação interrompida");
            }
        });

    Ok(Response::builder()
        .header(header::CONTENT_TYPE, params.format.content_type())
        .header(
            header::CONTENT_DISPOSITION,
            format!(
                "attachment; filename=\"{}\"",
                file_name(&table.name, params.format)
            ),
        )
        .header(header::CACHE_CONTROL, "no-store")
        .body(Body::from_stream(stream))?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn csv_escapa_so_o_necessario() {
        assert_eq!(csv_field("simples"), "simples");
        assert_eq!(csv_field("a,b"), "\"a,b\"");
        assert_eq!(csv_field("diz \"oi\""), "\"diz \"\"oi\"\"\"");
        assert_eq!(csv_field("linha\nnova"), "\"linha\nnova\"");
        assert_eq!(csv_field(" espaço"), "\" espaço\"");
    }

    #[test]
    fn csv_null_vira_campo_vazio() {
        assert_eq!(
            csv_line([Some("1"), None, Some("x")].into_iter()),
            "1,,x\r\n"
        );
    }

    #[test]
    fn linhas_csv_seguem_a_ordem_das_colunas() {
        let encoder = Encoder {
            format: Format::Csv,
            columns: vec!["id".into(), "nome".into(), "extra".into()],
        };
        let line = encoder
            .row(0, r#"{"nome":"Ana, a primeira","id":1,"extra":null}"#)
            .unwrap();
        assert_eq!(line, "1,\"Ana, a primeira\",\r\n");
    }

    #[test]
    fn json_separa_registros_por_virgula() {
        let encoder = Encoder {
            format: Format::Json,
            columns: vec![],
        };
        let body = [
            encoder.header(),
            encoder.row(0, r#"{"id":1}"#).unwrap(),
            encoder.row(1, r#"{"id":2.50}"#).unwrap(),
            encoder.footer().to_owned(),
        ]
        .concat();
        let parsed: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(parsed.as_array().unwrap().len(), 2);
        assert!(body.contains("2.50"), "numeric preservado: {body}");
    }

    #[test]
    fn nome_de_arquivo_sem_caracteres_perigosos() {
        assert_eq!(file_name("pedidos", Format::Csv), "pedidos.csv");
        assert_eq!(file_name("a\"b;c", Format::Json), "a_b_c.json");
        assert_eq!(file_name("ção", Format::Csv), "__o.csv");
        assert_eq!(file_name("çã", Format::Csv), "tabela.csv");
    }
}
