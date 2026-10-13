//! URL validation and query encoding for the Deezer API.
//!
//! Everything here is pure: it never touches the network and never depends on
//! the plugin bindings, so it is unit-tested on the host target.

/// Base URL of the Deezer public API.
pub const DEFAULT_BASE_URL: &str = "https://api.deezer.com";

/// Returns whether `value` is an absolute `http` or `https` URL.
///
/// The check is deliberately structural rather than a full URL parse: it
/// accepts exactly the shapes a Deezer base URL can have (the public API, a
/// mirror, or a test double on a private host), and rejects everything a
/// request could not be built from.
///
/// # Parameters
///
/// - `value`: the candidate URL.
///
/// # Returns
///
/// `true` when `value` starts with `http://` or `https://`, carries a non-empty
/// host, and contains no whitespace.
///
/// # Example
///
/// ```text
/// is_http_url("https://api.deezer.com")   -> true
/// is_http_url("http://localhost:8080")    -> true
/// is_http_url("api.deezer.com")           -> false
/// is_http_url("ftp://api.deezer.com")     -> false
/// ```
pub fn is_http_url(value: &str) -> bool {
    let after_scheme = match value.strip_prefix("https://") {
        Some(rest) => rest,
        None => match value.strip_prefix("http://") {
            Some(rest) => rest,
            None => return false,
        },
    };
    let host = after_scheme
        .split(['/', '?', '#'])
        .next()
        .unwrap_or_default();
    !host.is_empty() && !host.contains(char::is_whitespace)
}

/// Normalizes a configured base URL.
///
/// Surrounding whitespace and trailing slashes are removed so that the
/// configured value and the URLs built from it do not double up on `/`.
///
/// # Parameters
///
/// - `value`: the configured base URL.
///
/// # Returns
///
/// The trimmed URL, without a trailing slash.
///
/// # Example
///
/// ```text
/// normalize_base_url(" https://api.deezer.com/ ") -> "https://api.deezer.com"
/// normalize_base_url("https://api.deezer.com")   -> "https://api.deezer.com"
/// ```
pub fn normalize_base_url(value: &str) -> String {
    value.trim().trim_end_matches('/').to_string()
}

/// Joins a base URL with an API path.
///
/// # Parameters
///
/// - `base_url`: the configured base URL, with or without a trailing slash.
/// - `path`: the API path, with or without a leading slash.
///
/// # Returns
///
/// The absolute URL of the endpoint.
///
/// # Example
///
/// ```text
/// endpoint("https://api.deezer.com/", "/track/27") -> "https://api.deezer.com/track/27"
/// endpoint("https://api.deezer.com", "search?q=a") -> "https://api.deezer.com/search?q=a"
/// ```
pub fn endpoint(base_url: &str, path: &str) -> String {
    format!(
        "{}/{}",
        normalize_base_url(base_url),
        path.trim_start_matches('/')
    )
}

/// Percent-encodes one query or path component.
///
/// Everything outside the RFC 3986 unreserved set is escaped, including `/`,
/// `&`, `=`, `?` and `#`, so a search text or an identifier cannot add
/// parameters or change the endpoint it is pasted into. A space becomes `%20`
/// rather than `+`, which every Deezer endpoint accepts.
///
/// # Parameters
///
/// - `value`: the component to encode.
///
/// # Returns
///
/// The encoded component.
///
/// # Example
///
/// ```text
/// encode_component("Daft Punk")        -> "Daft%20Punk"
/// encode_component("AC/DC & friends")  -> "AC%2FDC%20%26%20friends"
/// ```
pub fn encode_component(value: &str) -> String {
    let mut encoded = String::with_capacity(value.len());
    for byte in value.as_bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(char::from(*byte));
            }
            _ => {
                encoded.push('%');
                encoded.push(char::from(HEX_DIGITS[usize::from(byte >> 4)]));
                encoded.push(char::from(HEX_DIGITS[usize::from(byte & 0x0f)]));
            }
        }
    }
    encoded
}

/// Uppercase hexadecimal digits used by [`encode_component`].
const HEX_DIGITS: [u8; 16] = *b"0123456789ABCDEF";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_http_and_https_urls() {
        assert!(is_http_url("https://api.deezer.com"));
        assert!(is_http_url("http://localhost:8080"));
        assert!(is_http_url("https://mirror.example.org/deezer/"));
    }

    #[test]
    fn rejects_urls_without_a_host_or_scheme() {
        assert!(!is_http_url("api.deezer.com"));
        assert!(!is_http_url("ftp://api.deezer.com"));
        assert!(!is_http_url("https://"));
        assert!(!is_http_url("https:///search"));
        assert!(!is_http_url("http://api deezer.com"));
    }

    #[test]
    fn normalization_removes_surrounding_space_and_trailing_slashes() {
        assert_eq!(
            normalize_base_url("  https://x.example/  "),
            "https://x.example"
        );
        assert_eq!(normalize_base_url("https://x.example"), "https://x.example");
    }

    #[test]
    fn endpoint_joins_exactly_one_slash() {
        assert_eq!(
            endpoint("https://api.deezer.com", "search?q=a"),
            "https://api.deezer.com/search?q=a"
        );
        assert_eq!(
            endpoint("https://api.deezer.com/", "/search?q=a"),
            "https://api.deezer.com/search?q=a"
        );
    }

    #[test]
    fn encoding_escapes_reserved_characters() {
        assert_eq!(encode_component("Daft Punk"), "Daft%20Punk");
        assert_eq!(
            encode_component("AC/DC & friends"),
            "AC%2FDC%20%26%20friends"
        );
        assert_eq!(encode_component("a=1&b=2"), "a%3D1%26b%3D2");
        assert_eq!(encode_component("Bjork"), "Bjork");
    }

    #[test]
    fn encoding_escapes_every_utf8_byte() {
        assert_eq!(encode_component("Bjork - Joga"), "Bjork%20-%20Joga");
        assert_eq!(
            encode_component("Sigur Ros: ( )"),
            "Sigur%20Ros%3A%20%28%20%29"
        );
        assert_eq!(encode_component("Sao Paulo"), "Sao%20Paulo");
    }
}
