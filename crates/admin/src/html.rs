//! HTML do painel: layout e escape. Sem template engine: as páginas são
//! pequenas e todo texto dinâmico passa por [`esc`].

use axum::response::Html;
use serde_json::value::RawValue;

/// Escapa texto para HTML (conteúdo e atributos entre aspas).
pub fn esc(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            c => out.push(c),
        }
    }
    out
}

/// Escapa um componente de URL (query string).
pub fn url(text: &str) -> String {
    form_urlencoded::byte_serialize(text.as_bytes()).collect()
}

/// Texto de um valor JSON vindo do Postgres, sem passar por `f64` (numeric
/// mantém todas as casas). `None` = NULL.
pub fn raw_text(value: Option<&RawValue>) -> Option<String> {
    let raw = value?.get();
    if raw == "null" {
        None
    } else if raw.starts_with('"') {
        serde_json::from_str::<String>(raw).ok()
    } else {
        Some(raw.to_owned())
    }
}

/// Valor de uma célula, truncado para a listagem.
pub fn cell(value: Option<&RawValue>, max: usize) -> String {
    let Some(text) = raw_text(value) else {
        return "<em class=\"null\">NULL</em>".into();
    };
    let short: String = text.chars().take(max).collect();
    if short.len() < text.len() {
        format!("{}…", esc(&short))
    } else {
        esc(&short)
    }
}

/// Valor para um `<input>` (sem truncar).
pub fn input_value(value: Option<&RawValue>) -> String {
    raw_text(value).map(|t| esc(&t)).unwrap_or_default()
}

const NAV: [(&str, &str); 4] = [
    ("/admin/", "Tabelas"),
    ("/admin/sql", "SQL"),
    ("/admin/users", "Usuários"),
    ("/admin/policies", "Policies"),
];

pub fn page(title: &str, active: &str, body: &str) -> Html<String> {
    let nav: String = NAV
        .iter()
        .map(|(href, label)| {
            let class = if *href == active {
                " class=\"active\""
            } else {
                ""
            };
            format!("<a href=\"{href}\"{class}>{label}</a>")
        })
        .collect();
    Html(format!(
        "<!doctype html><html lang=\"pt-BR\"><head><meta charset=\"utf-8\">\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\
         <title>{title} · nelcota</title>\
         <link rel=\"stylesheet\" href=\"/admin/assets/admin.css\">\
         <script src=\"/admin/assets/admin.js\" defer></script></head><body>\
         <header><strong>nelcota</strong><nav>{nav}</nav>\
         <form method=\"post\" action=\"/admin/logout\"><button class=\"link\">Sair</button></form></header>\
         <p class=\"banner\">A chave <code>service_role</code> ignora o RLS: use só no seu backend, nunca no frontend.</p>\
         <main>{body}</main></body></html>",
        title = esc(title),
    ))
}

pub fn login(error: Option<&str>) -> Html<String> {
    let error = error
        .map(|e| format!("<p class=\"error\">{}</p>", esc(e)))
        .unwrap_or_default();
    Html(format!(
        "<!doctype html><html lang=\"pt-BR\"><head><meta charset=\"utf-8\">\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\
         <title>Entrar · nelcota</title><link rel=\"stylesheet\" href=\"/admin/assets/admin.css\"></head>\
         <body class=\"login\"><form method=\"post\" action=\"/admin/login\" class=\"card\">\
         <h1>nelcota</h1><p>Painel administrativo</p>{error}\
         <label>Email<input name=\"email\" type=\"email\" autocomplete=\"username\" required autofocus></label>\
         <label>Senha<input name=\"password\" type=\"password\" autocomplete=\"current-password\" required></label>\
         <button>Entrar</button></form></body></html>"
    ))
}

pub fn notice(kind: &str, text: &str) -> String {
    format!("<p class=\"{kind}\">{}</p>", esc(text))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapa_html() {
        assert_eq!(
            esc("<script>alert('x')</script> & \"a\""),
            "&lt;script&gt;alert(&#39;x&#39;)&lt;/script&gt; &amp; &quot;a&quot;"
        );
        let raw = |s: &str| serde_json::from_str::<Box<RawValue>>(s).unwrap();
        assert_eq!(cell(Some(&raw("\"<b>\"")), 10), "&lt;b&gt;");
        assert_eq!(cell(Some(&raw("\"abcdef\"")), 3), "abc…");
        assert_eq!(
            cell(Some(&raw("12345678901234567890.10")), 40),
            "12345678901234567890.10"
        );
        assert!(cell(Some(&raw("null")), 10).contains("NULL"));
        assert_eq!(url("a b&c"), "a+b%26c");
    }
}
