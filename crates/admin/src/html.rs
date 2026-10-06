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

/// Ícones SVG (traço, 24×24), desenhados para o painel.
pub fn icon(name: &str) -> String {
    let path = match name {
        "home" => r#"<path d="M3 11 12 4l9 7"/><path d="M5 10v10h14V10"/>"#,
        "table" => {
            r#"<rect x="3" y="4" width="18" height="16" rx="2"/><path d="M3 10h18M9 4v16"/>"#
        }
        "sql" => {
            r#"<rect x="3" y="4" width="18" height="16" rx="2"/><path d="m7 9 3 3-3 3M13 15h4"/>"#
        }
        "users" => {
            r#"<circle cx="9" cy="8" r="3.5"/><path d="M2.5 20a6.5 6.5 0 0 1 13 0M16 4.5a3.5 3.5 0 0 1 0 7M18 14a5.5 5.5 0 0 1 3.5 6"/>"#
        }
        "shield" => {
            r#"<path d="M12 3 4.5 6v5.5c0 4.5 3.2 8.2 7.5 9.5 4.3-1.3 7.5-5 7.5-9.5V6z"/><path d="m9 12 2 2 4-4"/>"#
        }
        "book" => r#"<path d="M4 5a2 2 0 0 1 2-2h13v16H6a2 2 0 0 0-2 2z"/><path d="M4 19V5"/>"#,
        "logout" => {
            r#"<path d="M15 4h3a2 2 0 0 1 2 2v12a2 2 0 0 1-2 2h-3M10 16l-4-4 4-4M6 12h10"/>"#
        }
        "alert" => r#"<path d="M12 4 2.5 20h19z"/><path d="M12 10v4M12 17h.01"/>"#,
        "plus" => r#"<path d="M12 5v14M5 12h14"/>"#,
        "search" => r#"<circle cx="11" cy="11" r="6.5"/><path d="m20 20-4.2-4.2"/>"#,
        "key" => r#"<circle cx="8" cy="15" r="4"/><path d="m11 12 9-9M17 6l3 3"/>"#,
        "play" => r#"<path d="M7 5v14l11-7z"/>"#,
        "chevron" => r#"<path d="m9 6 6 6-6 6"/>"#,
        "lock" => {
            r#"<rect x="4" y="11" width="16" height="10" rx="2"/><path d="M8 11V7a4 4 0 0 1 8 0v4"/>"#
        }
        "fn" => r#"<path d="M14 4h-2a3 3 0 0 0-3 3v13M6 11h7M15 14l5 5M20 14l-5 5"/>"#,
        _ => "",
    };
    format!(r#"<svg class="i" viewBox="0 0 24 24" aria-hidden="true">{path}</svg>"#)
}

/// Itens da sidebar: (seção, href, ícone, rótulo).
const NAV: [(&str, &str, &str, &str); 6] = [
    ("", "/admin/", "home", "Visão geral"),
    ("", "/admin/tables", "table", "Editor de tabelas"),
    ("", "/admin/sql", "sql", "Editor SQL"),
    ("Autenticação", "/admin/users", "users", "Usuários"),
    ("", "/admin/policies", "shield", "Policies"),
    ("API", "/rest/v1/", "book", "OpenAPI"),
];

fn document(title: &str, body_class: &str, body: &str) -> Html<String> {
    Html(format!(
        "<!doctype html><html lang=\"pt-BR\"><head><meta charset=\"utf-8\">\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\
         <title>{title} · nelcota</title>\
         <link rel=\"preload\" href=\"/admin/assets/inter.woff2\" as=\"font\" type=\"font/woff2\" crossorigin>\
         <link rel=\"stylesheet\" href=\"/admin/assets/admin.css\">\
         <script src=\"/admin/assets/admin.js\" defer></script></head>\
         <body class=\"{body_class}\">{body}</body></html>",
        title = esc(title),
    ))
}

fn brand() -> &'static str {
    "<div class=\"brand\"><span class=\"mark\">n</span>nelcota<small>admin</small></div>"
}

/// Layout com sidebar e topbar. `crumbs` vira a trilha no topo; o conteúdo
/// ocupa a área inteira (o chamador decide o padding).
pub fn layout(title: &str, active: &str, crumbs: &[&str], body: &str) -> Html<String> {
    let mut nav = String::new();
    for (section, href, icon_name, label) in NAV {
        if !section.is_empty() {
            nav.push_str(&format!("<div class=\"label\">{section}</div>"));
        }
        let class = if href == active {
            " class=\"active\""
        } else {
            ""
        };
        let target = if href.starts_with("/rest") {
            " target=\"_blank\" rel=\"noopener\""
        } else {
            ""
        };
        nav.push_str(&format!(
            "<a href=\"{href}\"{class}{target}>{}{label}</a>",
            icon(icon_name)
        ));
    }
    let mut trail = String::from("<span>nelcota</span>");
    for (i, crumb) in crumbs.iter().enumerate() {
        trail.push_str(&icon("chevron"));
        if i + 1 == crumbs.len() {
            trail.push_str(&format!("<b>{}</b>", esc(crumb)));
        } else {
            trail.push_str(&format!("<span>{}</span>", esc(crumb)));
        }
    }
    document(
        title,
        "",
        &format!(
            "<div class=\"app\"><aside class=\"sidebar\">{brand}<nav class=\"nav\">{nav}</nav>\
             <div class=\"sidebar-foot\">\
             <div class=\"who\" title=\"A chave service_role ignora o RLS: use só no seu backend, nunca no frontend.\">{key}<span>service_role ignora o RLS</span></div>\
             <form method=\"post\" action=\"/admin/logout\" class=\"nav\"><button>{logout}Sair</button></form></div></aside>\
             <div class=\"main\"><div class=\"topbar\"><div class=\"crumbs\">{trail}</div><div class=\"spacer\"></div>\
             <span class=\"badge ok\">Postgres 17</span></div>\
             <div class=\"content\">{body}</div></div></div>",
            brand = brand(),
            key = icon("key"),
            logout = icon("logout"),
        ),
    )
}

/// Página comum (conteúdo com padding e largura máxima).
pub fn page(title: &str, active: &str, body: &str) -> Html<String> {
    layout(
        title,
        active,
        &[title],
        &format!("<div class=\"page\">{body}</div>"),
    )
}

pub fn login(error: Option<&str>) -> Html<String> {
    let error = error
        .map(|e| format!("<div class=\"notice error\">{}</div>", esc(e)))
        .unwrap_or_default();
    document(
        "Entrar",
        "login",
        &format!(
            "<form method=\"post\" action=\"/admin/login\" class=\"login-card\">{brand}\
             <h1>Bem-vindo de volta</h1><p class=\"sub\">Entre no painel administrativo</p>{error}\
             <div class=\"field\"><label for=\"email\">Email</label>\
             <input id=\"email\" name=\"email\" type=\"email\" autocomplete=\"username\" placeholder=\"admin@seudominio.com\" required autofocus></div>\
             <div class=\"field\"><label for=\"password\">Senha</label>\
             <input id=\"password\" name=\"password\" type=\"password\" autocomplete=\"current-password\" placeholder=\"••••••••\" required></div>\
             <button class=\"btn primary\">Entrar</button></form>",
            brand = brand(),
        ),
    )
}

pub fn notice(kind: &str, text: &str) -> String {
    format!("<div class=\"notice {kind}\">{}</div>", esc(text))
}

/// Alerta com ícone. `html` já deve estar escapado.
pub fn alert(kind: &str, html: &str) -> String {
    format!(
        "<div class=\"alert {kind}\">{}<div>{html}</div></div>",
        icon("alert")
    )
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
