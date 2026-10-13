//! HTTP access to the Deezer API through the `host-http` host import.
//!
//! This is the only module that touches the network. It builds one request,
//! sends it, and turns the response into either a decoded payload or a provider
//! error; endpoint composition lives in [`crate::catalog`] and the translation
//! into catalog records in [`crate::map`].
//!
//! Deezer reports most failures with HTTP status 200 and an `error` object
//! rather than an HTTP error status, so a response body is never believed
//! before it has been inspected.

use hearthsound_plugin::host::http;
use hearthsound_plugin::types::ProviderError;
use serde::de::DeserializeOwned;

use crate::deezer;
use crate::url;

/// Wall-clock budget for one Deezer request, in milliseconds.
///
/// The host clamps this to its own ceiling and to the remaining call deadline.
const REQUEST_TIMEOUT_MS: u32 = 10_000;

/// Quota error code of the Deezer API.
const QUOTA_ERROR_CODE: i64 = 4;

/// "No data" error code of the Deezer API.
const NO_DATA_ERROR_CODE: i64 = 800;

/// Delay reported with a rate-limited error, in seconds.
///
/// Deezer does not send a `Retry-After` header, so the plugin reports a fixed
/// delay the server can spread its retry over.
const RATE_LIMIT_RETRY_SECS: u32 = 60;

/// Sends a GET request to a Deezer endpoint and decodes the JSON body.
///
/// # Parameters
///
/// - `base_url`: base URL of the API, without a trailing slash.
/// - `path`: API path, without a leading slash.
///
/// # Returns
///
/// The decoded response body.
///
/// # Errors
///
/// Returns [`ProviderError::Unavailable`] when the host refuses or cannot
/// complete the request and when the body is not JSON, the mapped Deezer error
/// (not-found, rate-limited or unavailable) when the body carries an `error`
/// object, and [`ProviderError::Internal`] when the body is JSON of an
/// unexpected shape.
pub(crate) fn get<T: DeserializeOwned>(base_url: &str, path: &str) -> Result<T, ProviderError> {
    let target = url::endpoint(base_url, path);
    let request = http::Request {
        method: "GET".to_string(),
        url: target.clone(),
        headers: vec![("accept".to_string(), "application/json".to_string())],
        body: None,
        timeout_ms: Some(REQUEST_TIMEOUT_MS),
    };

    let response = http::send(&request).map_err(|error| http_error(&target, error))?;
    decode(response.status, &response.body, &target)
}

/// Decodes one Deezer response.
///
/// # Parameters
///
/// - `status`: HTTP status of the response.
/// - `body`: raw response body.
/// - `url`: request URL, used in error messages.
///
/// # Returns
///
/// The decoded body.
///
/// # Errors
///
/// Returns [`ProviderError::NotFound`] for HTTP 404 and for a Deezer "no data"
/// error, [`ProviderError::RateLimited`] for HTTP 429 and for a Deezer quota
/// error, [`ProviderError::Unavailable`] for any other non-success status, a
/// body that is not JSON, or a Deezer error the plugin cannot classify, and
/// [`ProviderError::Internal`] for JSON of an unexpected shape.
pub(crate) fn decode<T: DeserializeOwned>(
    status: u16,
    body: &[u8],
    url: &str,
) -> Result<T, ProviderError> {
    if !(200..300).contains(&status) {
        return Err(match status {
            404 => ProviderError::NotFound,
            429 => ProviderError::RateLimited(RATE_LIMIT_RETRY_SECS),
            _ => ProviderError::Unavailable(format!("{url}: HTTP {status}")),
        });
    }

    let value: serde_json::Value = serde_json::from_slice(body)
        .map_err(|error| ProviderError::Unavailable(format!("{url}: invalid JSON: {error}")))?;

    if let Some(error) = value.get("error") {
        return Err(api_error(error, url));
    }

    serde_json::from_value(value)
        .map_err(|error| ProviderError::Internal(format!("{url}: unexpected response: {error}")))
}

/// Maps the `error` object of a Deezer response to a provider error.
///
/// # Parameters
///
/// - `value`: the `error` member of the response body.
/// - `url`: request URL, used in the message of the errors it reports.
///
/// # Returns
///
/// Not-found for a `DataException` or the `800` code, rate-limited for the `4`
/// quota code, and unavailable for anything else.
fn api_error(value: &serde_json::Value, url: &str) -> ProviderError {
    let error: deezer::Error = serde_json::from_value(value.clone()).unwrap_or_default();

    match error.code {
        Some(NO_DATA_ERROR_CODE) => ProviderError::NotFound,
        Some(QUOTA_ERROR_CODE) => ProviderError::RateLimited(RATE_LIMIT_RETRY_SECS),
        _ if error.kind.as_deref() == Some("DataException") => ProviderError::NotFound,
        _ => {
            let code = error
                .code
                .map_or_else(|| "unknown".to_string(), |code| code.to_string());
            let message = error.message.unwrap_or_default();
            ProviderError::Unavailable(format!("{url}: Deezer error {code}: {message}"))
        }
    }
}

/// Maps a `host-http` failure to a provider error.
///
/// # Parameters
///
/// - `url`: request URL, used in the message.
/// - `error`: failure reported by the host.
///
/// # Returns
///
/// [`ProviderError::Unavailable`], because every `host-http` failure is the
/// request not happening rather than an answer from Deezer.
fn http_error(url: &str, error: http::HttpError) -> ProviderError {
    ProviderError::Unavailable(match error {
        http::HttpError::Denied(message) => format!("{url}: request denied: {message}"),
        http::HttpError::Timeout => format!("{url}: request timed out"),
        http::HttpError::BodyTooLarge => format!("{url}: response body too large"),
        http::HttpError::Network(message) => format!("{url}: network error: {message}"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_a_page() {
        let body = include_str!("fixtures/search_tracks.json").as_bytes();
        let page: deezer::Page<deezer::Track> =
            decode(200, body, "https://api.deezer.com/search").unwrap();

        assert_eq!(page.data.len(), 1);
        assert_eq!(page.data[0].id, 67_238_732);
    }

    #[test]
    fn decodes_a_single_entity() {
        let body = include_str!("fixtures/track.json").as_bytes();
        let track: deezer::Track =
            decode(200, body, "https://api.deezer.com/track/isrc:USQX91300105").unwrap();

        assert_eq!(track.isrc.as_deref(), Some("USQX91300105"));
    }

    #[test]
    fn decodes_the_health_probe_response() {
        let body = include_str!("fixtures/genre.json").as_bytes();
        let page: deezer::Page<deezer::Genre> =
            decode(200, body, "https://api.deezer.com/genre").unwrap();

        assert_eq!(page.data.len(), 2);
        assert_eq!(page.data[0].name, "All");
        assert_eq!(page.data[1].name, "Pop");
        assert_eq!(page.total, None);
    }

    #[test]
    fn maps_no_data_to_not_found() {
        let body = include_str!("fixtures/error_no_data.json").as_bytes();
        let error: ProviderError =
            decode::<deezer::Track>(200, body, "https://api.deezer.com/track/1").unwrap_err();

        assert!(matches!(error, ProviderError::NotFound));
    }

    #[test]
    fn maps_a_quota_error_to_rate_limited() {
        let body = br#"{"error":{"type":"Exception","message":"Quota limit exceeded","code":4}}"#;
        let error: ProviderError =
            decode::<deezer::Track>(200, body, "https://api.deezer.com/track/1").unwrap_err();

        assert!(matches!(error, ProviderError::RateLimited(60)));
    }

    #[test]
    fn maps_other_deezer_errors_to_unavailable() {
        let body =
            br#"{"error":{"type":"ParameterException","message":"empty parameter","code":500}}"#;
        let error: ProviderError =
            decode::<deezer::Page<deezer::Track>>(200, body, "u").unwrap_err();

        match error {
            ProviderError::Unavailable(message) => {
                assert!(message.contains("500"), "{message}");
                assert!(message.contains("empty parameter"), "{message}");
            }
            other => panic!("expected unavailable, got {other:?}"),
        }
    }

    #[test]
    fn maps_http_statuses() {
        assert!(matches!(
            decode::<deezer::Track>(404, b"", "u").unwrap_err(),
            ProviderError::NotFound
        ));
        assert!(matches!(
            decode::<deezer::Track>(429, b"", "u").unwrap_err(),
            ProviderError::RateLimited(60)
        ));
        match decode::<deezer::Track>(503, br#"{"error":"unavailable"}"#, "u").unwrap_err() {
            ProviderError::Unavailable(message) => {
                assert!(message.contains("HTTP 503"), "{message}")
            }
            other => panic!("expected unavailable, got {other:?}"),
        }
    }

    #[test]
    fn rejects_bodies_that_are_not_json() {
        match decode::<deezer::Track>(200, b"<html>", "u").unwrap_err() {
            ProviderError::Unavailable(message) => {
                assert!(message.contains("invalid JSON"), "{message}")
            }
            other => panic!("expected unavailable, got {other:?}"),
        }
    }

    #[test]
    fn rejects_json_of_an_unexpected_shape() {
        match decode::<deezer::Page<deezer::Track>>(200, br#"{"data":"nope"}"#, "u").unwrap_err() {
            ProviderError::Internal(message) => {
                assert!(message.contains("unexpected response"), "{message}")
            }
            other => panic!("expected internal, got {other:?}"),
        }
    }

    #[test]
    fn maps_host_http_failures_to_unavailable() {
        for error in [
            http::HttpError::Denied("host not allowlisted".to_string()),
            http::HttpError::Timeout,
            http::HttpError::BodyTooLarge,
            http::HttpError::Network("connection reset".to_string()),
        ] {
            assert!(matches!(
                http_error("u", error),
                ProviderError::Unavailable(_)
            ));
        }
    }
}
