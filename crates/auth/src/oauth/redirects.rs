//! Where a sign-in may send the person back: only to the app pages the project
//! lists, so `/authorize` cannot be used as an open redirect (or to deliver a
//! code to someone else's site).
use url::Url;

#[derive(Debug)]
pub(super) struct RedirectAllowlist(Vec<Url>);

impl RedirectAllowlist {
    /// Each entry is an http(s) URL; a target matches when it has the same
    /// origin and its path is the entry's path or below it.
    pub(super) fn new<'a>(entries: impl IntoIterator<Item = &'a str>) -> Result<Self, String> {
        let mut allowed = Vec::new();
        for entry in entries {
            let url = parse(entry).ok_or_else(|| {
                format!("invalid redirect URL {entry:?}: use an http(s) URL without credentials")
            })?;
            allowed.push(url);
        }
        if allowed.is_empty() {
            return Err("list at least one app URL the sign-in may return to".into());
        }
        Ok(Self(allowed))
    }

    /// The target, parsed, when one of the entries allows it.
    pub(super) fn allows(&self, target: &str) -> Option<Url> {
        let url = parse(target)?;
        self.0
            .iter()
            .any(|entry| {
                let base = entry.path().trim_end_matches('/');
                entry.origin() == url.origin()
                    && (url.path() == base || url.path().starts_with(&format!("{base}/")))
            })
            .then_some(url)
    }
}

fn parse(value: &str) -> Option<Url> {
    let url = Url::parse(value.trim()).ok()?;
    let plain = matches!(url.scheme(), "http" | "https")
        && url.host().is_some()
        && url.username().is_empty()
        && url.password().is_none();
    plain.then_some(url)
}

#[cfg(test)]
mod tests {
    use super::RedirectAllowlist;

    #[test]
    fn same_origin_and_path_prefix_only() {
        let list =
            RedirectAllowlist::new(["https://app.shop.com/auth", "http://localhost:5173"]).unwrap();
        for ok in [
            "https://app.shop.com/auth",
            "https://app.shop.com/auth/callback?next=/cart",
            "http://localhost:5173/anything",
        ] {
            assert!(list.allows(ok).is_some(), "{ok}");
        }
        for refused in [
            "https://app.shop.com/authx",
            "https://app.shop.com/",
            "https://evil.com/auth",
            "https://app.shop.com.evil.com/auth",
            "http://app.shop.com/auth",
            "https://user:pw@app.shop.com/auth",
            "javascript:alert(1)",
            "//evil.com",
            "http://localhost:5174/",
        ] {
            assert!(list.allows(refused).is_none(), "{refused}");
        }
    }

    #[test]
    fn needs_valid_entries() {
        assert!(RedirectAllowlist::new([]).is_err());
        assert!(RedirectAllowlist::new(["ftp://x.com"]).is_err());
        assert!(RedirectAllowlist::new(["not a url"]).is_err());
    }
}
