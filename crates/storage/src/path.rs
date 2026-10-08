//! Bucket and object names (D79). Names reach the store only as part of a
//! row's key (`<bucket>/<version>`), never as a file path, but they still end
//! up in URLs, listings and `Content-Disposition`: the rules keep them
//! unambiguous and printable.

use axum::http::StatusCode;
use nelcota_core::ApiError;
use unicode_normalization::UnicodeNormalization;

/// Longest object name, in bytes (same CHECK as `storage.objects.name`).
pub const MAX_NAME_BYTES: usize = 1024;
const MAX_SEGMENT_BYTES: usize = 255;

/// Words the routes use after `/storage/v1/object/` (also refused by the
/// CHECK on `storage.buckets.id`).
const RESERVED: [&str; 3] = ["public", "sign", "list"];

fn invalid(message: impl Into<String>) -> ApiError {
    ApiError::new(StatusCode::BAD_REQUEST, "invalid_path", message)
}

/// A bucket id: `[a-z0-9][a-z0-9_-]{0,62}`, not a route word.
pub fn bucket(id: &str) -> Result<&str, ApiError> {
    let mut chars = id.chars();
    let valid = chars
        .next()
        .is_some_and(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
        && id.len() <= 63
        && !RESERVED.contains(&id);
    if valid {
        Ok(id)
    } else {
        Err(invalid(
            "invalid bucket name: lowercase letters, digits, '-' and '_', up to 63",
        ))
    }
}

/// An object name, normalized to NFC. Segments are separated by `/`; none
/// may be empty, `.` or `..`, and control characters and `\` are refused.
pub fn object(name: &str) -> Result<String, ApiError> {
    let name: String = name.nfc().collect();
    if name.is_empty() || name.len() > MAX_NAME_BYTES {
        return Err(invalid(format!(
            "the file name needs 1 to {MAX_NAME_BYTES} bytes"
        )));
    }
    if name.chars().any(|c| c.is_control() || c == '\\') {
        return Err(invalid(
            "the file name has control characters or a backslash",
        ));
    }
    for segment in name.split('/') {
        if segment.is_empty() || segment == "." || segment == ".." {
            return Err(invalid("the file name has an empty, '.' or '..' segment"));
        }
        if segment.len() > MAX_SEGMENT_BYTES {
            return Err(invalid(format!(
                "each part of the file name holds up to {MAX_SEGMENT_BYTES} bytes"
            )));
        }
    }
    Ok(name)
}

/// A listing prefix: empty, or object-name segments ending in `/`.
pub fn prefix(prefix: &str) -> Result<String, ApiError> {
    if prefix.is_empty() {
        return Ok(String::new());
    }
    let Some(folder) = prefix.strip_suffix('/') else {
        return Err(invalid("a prefix is a folder and ends in '/'"));
    };
    Ok(format!("{}/", object(folder)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bucket_names() {
        for good in ["a", "avatars", "user-files_2", &"x".repeat(63)] {
            assert!(bucket(good).is_ok(), "{good}");
        }
        for bad in [
            "",
            "Avatars",
            "-a",
            "_a",
            "a b",
            "a/b",
            "public",
            "sign",
            "list",
            "çã",
            &"x".repeat(64),
        ] {
            assert!(bucket(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn object_names() {
        assert_eq!(object("a/b/c.png").unwrap(), "a/b/c.png");
        assert_eq!(
            object("relatório final.pdf").unwrap(),
            "relatório final.pdf"
        );
        // "e" + combining acute becomes the single code point (NFC).
        assert_eq!(object("cafe\u{301}.txt").unwrap(), "caf\u{e9}.txt");
        for bad in [
            "",
            "/a",
            "a/",
            "a//b",
            ".",
            "..",
            "a/../b",
            "a/./b",
            "a\\b",
            "a\nb",
            "a\0b",
            "a\u{7f}b",
            &"x".repeat(256),
            &"x/".repeat(600),
        ] {
            assert!(object(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn prefixes() {
        assert_eq!(prefix("").unwrap(), "");
        assert_eq!(prefix("a/b/").unwrap(), "a/b/");
        for bad in ["a", "/", "a//", "../"] {
            assert!(prefix(bad).is_err(), "{bad:?}");
        }
    }
}
