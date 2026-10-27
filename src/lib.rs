//! Deezer catalog plugin: a Hearthsound catalog provider over the Deezer
//! public API.
//!
//! The plugin needs no account and no key, because every endpoint it uses is
//! part of Deezer's public API. It implements `lifecycle` (configuration
//! validation and a health probe), `catalog` (search, entity lookups, artist
//! listings and ISRC resolution) and `artwork` (album covers and artist
//! portraits for the server's artwork layer).
//!
//! The only instance setting is `baseUrl`, which defaults to the public API and
//! can be pointed at a mirror or a test double.
//!
//! The pure logic sits in public modules ([`url`], [`deezer`], [`map`]); the
//! binding-coupled glue that drives it - the HTTP transport and the catalog and
//! artwork operations - stays private.
//!
//! # Tests
//!
//! The pure logic is unit-tested on the host target:
//!
//! ```text
//! cargo test
//! ```
//!
//! Rustdoc examples in those modules are illustrative `text` blocks rather than
//! doctests: the crate is a `cdylib` for `wasm32-wasip2`, so there is no host
//! library target for a doctest to link against. Every example appears as an
//! assertion in the unit tests of the module that carries it.

#![forbid(unsafe_code)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

use hearthsound_plugin::catalog_artwork_provider::exports::hearthsound::plugin::artwork::{
    ArtworkSubject, Guest as ArtworkGuest,
};
use hearthsound_plugin::catalog_artwork_provider::exports::hearthsound::plugin::catalog::{
    Guest as CatalogGuest, SearchQuery, SearchResults,
};
use hearthsound_plugin::catalog_artwork_provider::exports::hearthsound::plugin::lifecycle::Guest as LifecycleGuest;
use hearthsound_plugin::types::{
    Album, AlbumRef, Artist, EntityKind, IdPair, Image, ProviderError, Track,
};

mod api;
mod artwork;
mod catalog;
pub mod deezer;
pub mod map;
pub mod url;

/// The plugin implementation.
struct DeezerCatalog;

/// Server-scoped configuration of the plugin.
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct Config {
    /// Base URL of the Deezer API.
    #[serde(default = "default_base_url")]
    base_url: String,
}

/// Returns the base URL used when the instance configuration omits one.
fn default_base_url() -> String {
    url::DEFAULT_BASE_URL.to_string()
}

/// Reads this instance's configuration and returns its base URL.
///
/// # Returns
///
/// The configured base URL, trimmed and without a trailing slash.
///
/// # Errors
///
/// Returns [`ProviderError::InvalidConfig`] when the configuration cannot be
/// read or deserialized, and when the base URL is not an absolute `http` or
/// `https` URL.
fn base_url() -> Result<String, ProviderError> {
    let config: Config = hearthsound_plugin::config::server_config()?;
    let base_url = url::normalize_base_url(&config.base_url);

    if url::is_http_url(&base_url) {
        Ok(base_url)
    } else {
        Err(ProviderError::InvalidConfig(format!(
            "baseUrl must be an http or https URL, got `{}`",
            config.base_url
        )))
    }
}

/// Validates the plugin's server-scoped configuration.
///
/// # Parameters
///
/// - `server_config`: the configuration JSON as the administrator stored it.
///
/// # Returns
///
/// `Ok(())` when the configuration is usable.
///
/// # Errors
///
/// Returns one message per problem: JSON that does not deserialize into the
/// configuration, and a `baseUrl` that is not an absolute `http` or `https`
/// URL.
fn validate_config(server_config: &str) -> Result<(), Vec<String>> {
    let config: Config = match serde_json::from_str(server_config) {
        Ok(config) => config,
        Err(error) => return Err(vec![format!("server config: {error}")]),
    };
    let base_url = url::normalize_base_url(&config.base_url);

    if url::is_http_url(&base_url) {
        Ok(())
    } else {
        Err(vec![format!(
            "baseUrl must be an http or https URL, got `{}`",
            config.base_url
        )])
    }
}

/// Returns whether a query asked for one entity kind.
///
/// # Parameters
///
/// - `kinds`: the kinds the query asked for; empty means every kind.
/// - `wanted`: the kind to test.
///
/// # Returns
///
/// `true` when `wanted` is in `kinds`, or when `kinds` is empty.
fn wants(kinds: &[EntityKind], wanted: EntityKind) -> bool {
    kinds.is_empty() || kinds.contains(&wanted)
}

/// Implements the lifecycle interface.
impl LifecycleGuest for DeezerCatalog {
    fn validate_config(server_config: String) -> Result<(), Vec<String>> {
        validate_config(&server_config)
    }

    fn health() -> Result<String, ProviderError> {
        let base_url = base_url()?;
        catalog::health(&base_url)?;

        Ok(format!("Deezer API reachable at {base_url}"))
    }
}

/// Implements the catalog interface.
impl CatalogGuest for DeezerCatalog {
    fn search(query: SearchQuery) -> Result<SearchResults, ProviderError> {
        hearthsound_plugin::debug!(
            "search {:?} kinds {:?} limit {} offset {}",
            query.text,
            query.kinds,
            query.limit,
            query.offset
        );

        let base_url = base_url()?;
        let mut tracks = Vec::new();
        let mut albums = Vec::new();
        let mut artists = Vec::new();

        // Deezer answers an empty query with a parameter error, so an empty
        // search text means "nothing matches" rather than "ask anyway".
        if !query.text.trim().is_empty() {
            // Each kind has its own endpoint and its own result page, so a
            // search costs one request per kind the caller asked for.
            if wants(&query.kinds, EntityKind::Track) {
                tracks = catalog::search_tracks(&base_url, &query.text, query.limit, query.offset)?;
            }
            if wants(&query.kinds, EntityKind::Album) {
                albums = catalog::search_albums(&base_url, &query.text, query.limit, query.offset)?;
            }
            if wants(&query.kinds, EntityKind::Artist) {
                artists =
                    catalog::search_artists(&base_url, &query.text, query.limit, query.offset)?;
            }
        }

        Ok(SearchResults {
            tracks,
            albums,
            artists,
        })
    }

    fn get_track(id: String) -> Result<Track, ProviderError> {
        catalog::track(&base_url()?, &id)
    }

    fn get_album(id: String) -> Result<Album, ProviderError> {
        catalog::album(&base_url()?, &id)
    }

    fn get_artist(id: String) -> Result<Artist, ProviderError> {
        catalog::artist(&base_url()?, &id)
    }

    fn artist_albums(id: String, limit: u32, offset: u32) -> Result<Vec<AlbumRef>, ProviderError> {
        catalog::artist_albums(&base_url()?, &id, limit, offset)
    }

    fn artist_top_tracks(id: String, limit: u32) -> Result<Vec<Track>, ProviderError> {
        catalog::artist_top_tracks(&base_url()?, &id, limit)
    }

    fn lookup_isrc(isrc: String) -> Result<Vec<Track>, ProviderError> {
        catalog::lookup_isrc(&base_url()?, &isrc)
    }

    fn lookup_album(ids: Vec<IdPair>) -> Result<Vec<AlbumRef>, ProviderError> {
        catalog::lookup_album(&base_url()?, &ids)
    }
}

/// Implements the artwork interface.
impl ArtworkGuest for DeezerCatalog {
    fn find_artwork(subject: ArtworkSubject) -> Result<Vec<Image>, ProviderError> {
        hearthsound_plugin::debug!(
            "artwork for {:?} `{}` by {:?}",
            subject.kind,
            subject.title,
            subject.artists
        );

        let images = artwork::find_artwork(&base_url()?, &subject)?;

        if images.is_empty() {
            hearthsound_plugin::debug!("no artwork for {:?} `{}`", subject.kind, subject.title);
        }

        Ok(images)
    }
}

hearthsound_plugin::export_catalog_artwork_provider!(DeezerCatalog);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_a_valid_configuration() {
        assert_eq!(validate_config("{}"), Ok(()));
        assert_eq!(
            validate_config(r#"{"baseUrl":"https://api.deezer.com"}"#),
            Ok(())
        );
        assert_eq!(
            validate_config(r#"{"baseUrl":"http://deezer.internal:8080/"}"#),
            Ok(())
        );
    }

    #[test]
    fn rejects_a_base_url_that_is_not_http() {
        let errors = validate_config(r#"{"baseUrl":"api.deezer.com"}"#).unwrap_err();

        assert_eq!(errors.len(), 1);
        assert!(errors[0].contains("baseUrl"), "{}", errors[0]);
    }

    #[test]
    fn rejects_configuration_of_an_unexpected_shape() {
        let errors = validate_config(r#"{"baseUrl":7}"#).unwrap_err();

        assert_eq!(errors.len(), 1);
        assert!(errors[0].contains("server config"), "{}", errors[0]);
    }

    #[test]
    fn empty_kinds_mean_every_kind() {
        assert!(wants(&[], EntityKind::Track));
        assert!(wants(&[], EntityKind::Album));
        assert!(wants(&[], EntityKind::Artist));
        assert!(wants(&[], EntityKind::Playlist));
    }

    #[test]
    fn asks_only_for_the_requested_kinds() {
        let kinds = [EntityKind::Track, EntityKind::Artist];

        assert!(wants(&kinds, EntityKind::Track));
        assert!(wants(&kinds, EntityKind::Artist));
        assert!(!wants(&kinds, EntityKind::Album));
        assert!(!wants(&kinds, EntityKind::Playlist));
    }

    #[test]
    fn keeps_the_request_rate_under_deezers_ceiling() {
        let manifest: serde_json::Value =
            serde_json::from_str(include_str!("../manifest.json")).unwrap();
        let limits = &manifest["permissions"]["http"]["rateLimit"];
        let per_second = limits["perSecond"].as_u64().unwrap();
        let burst = limits["burst"].as_u64().unwrap();

        // Deezer publishes 50 requests per 5 s, so at most 10 in any second. The
        // declared rate plus one burst must stay below that.
        assert!(
            per_second + burst <= 10,
            "{per_second}/s with a burst of {burst} exceeds 10 requests in one second"
        );
        assert_eq!(per_second, 4);
        assert_eq!(burst, 4);
    }

    #[test]
    fn declares_the_api_version_it_builds_against() {
        let manifest: serde_json::Value =
            serde_json::from_str(include_str!("../manifest.json")).unwrap();

        assert_eq!(manifest["api"], "hearthsound:plugin@0.5.0");
    }
}
