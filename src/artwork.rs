//! The artwork interface: covers for albums and portraits for artists.
//!
//! One subject is answered at a time. A subject that carries the issuing
//! instance's own id is read from the entity endpoint, so the server gets the
//! cover the catalog layer reported; a subject that carries only a UPC, or
//! nothing but a title, falls back to the UPC endpoint and then to Deezer's
//! advanced search, and a search result is used only when its title and its
//! credited artist are the ones the subject names.
//!
//! Having no picture is not a failure: a lookup Deezer answers with "no data",
//! and a subject whose kind has no picture here, both answer an empty list.
//! Only a request that could not be made at all becomes a provider error, so
//! the server can tell "this provider has no picture" apart from "this provider
//! is unreachable".
//!
//! Deezer answers an artist without a portrait with a picture URL whose hash
//! segment is empty, which serves a flat black square rather than a portrait;
//! those entries are left out so a caller never shows one.

use hearthsound_plugin::catalog_artwork_provider::exports::hearthsound::plugin::artwork::ArtworkSubject;
use hearthsound_plugin::types::{EntityKind, IdPair, Image, ProviderError};

use crate::deezer;
use crate::{api, catalog, map};

/// Number of search results inspected when a subject carries no provider id.
///
/// Deezer orders a search by relevance, so the entity a subject names is at or
/// near the top of the first page; asking for more than the default page would
/// cost the same and find nothing extra in practice.
const SEARCH_LIMIT: u32 = 25;

/// Returns the renditions of one subject's picture.
///
/// # Parameters
///
/// - `base_url`: base URL of the API, without a trailing slash.
/// - `subject`: the entity whose picture is wanted.
///
/// # Returns
///
/// The images Deezer reports for the subject, smallest first, or an empty list
/// when the provider has no picture for it.
///
/// # Errors
///
/// Returns the provider error of a request that could not be made: an
/// unreachable API, a rate limit, or a response the plugin cannot read.
pub(crate) fn find_artwork(
    base_url: &str,
    subject: &ArtworkSubject,
) -> Result<Vec<Image>, ProviderError> {
    match subject.kind {
        EntityKind::Album => album_artwork(base_url, subject),
        EntityKind::Artist => artist_artwork(base_url, subject),
        // The host asks for covers and portraits; any other kind has no picture
        // this plugin could find.
        _ => Ok(Vec::new()),
    }
}

/// Returns the cover images of an album subject.
///
/// # Parameters
///
/// - `base_url`: base URL of the API.
/// - `subject`: the album whose cover is wanted.
///
/// # Returns
///
/// The album's covers, smallest first, or an empty list when Deezer does not
/// know the album, does not know its UPC, matches no search result, or has no
/// cover for it.
///
/// # Errors
///
/// Returns the provider error of a request that could not be made.
fn album_artwork(base_url: &str, subject: &ArtworkSubject) -> Result<Vec<Image>, ProviderError> {
    if let Some(id) = non_blank(subject.provider_id.as_deref()) {
        return covers(base_url, &catalog::element_path("album", id));
    }

    if let Some(upc) = id_value(&subject.ids, "upc") {
        return covers(base_url, &catalog::upc_path(upc));
    }

    if subject.title.trim().is_empty() {
        return Ok(Vec::new());
    }

    let path = catalog::search_path("search/album", &album_query(subject), SEARCH_LIMIT, 0);
    let Some(page) = known(api::get::<deezer::Page<deezer::Album>>(base_url, &path))? else {
        return Ok(Vec::new());
    };

    Ok(page
        .data
        .iter()
        .find(|candidate| matches_album(subject, candidate))
        .map_or_else(Vec::new, map::cover_images))
}

/// Returns the portrait images of an artist subject.
///
/// # Parameters
///
/// - `base_url`: base URL of the API.
/// - `subject`: the artist whose portrait is wanted.
///
/// # Returns
///
/// The artist's real portraits, smallest first, or an empty list when Deezer
/// does not know the artist, matches no search result, or has no portrait for
/// it.
///
/// # Errors
///
/// Returns the provider error of a request that could not be made.
fn artist_artwork(base_url: &str, subject: &ArtworkSubject) -> Result<Vec<Image>, ProviderError> {
    if let Some(id) = non_blank(subject.provider_id.as_deref()) {
        let path = catalog::element_path("artist", id);
        let Some(artist) = known(api::get::<deezer::Artist>(base_url, &path))? else {
            return Ok(Vec::new());
        };

        return Ok(portraits(&artist));
    }

    if subject.title.trim().is_empty() {
        return Ok(Vec::new());
    }

    let path = catalog::search_path("search/artist", subject.title.trim(), SEARCH_LIMIT, 0);
    let Some(page) = known(api::get::<deezer::Page<deezer::Artist>>(base_url, &path))? else {
        return Ok(Vec::new());
    };

    Ok(page
        .data
        .iter()
        .find(|candidate| same_text(&candidate.name, &subject.title))
        .map_or_else(Vec::new, portraits))
}

/// Reads one album and returns its covers.
///
/// # Parameters
///
/// - `base_url`: base URL of the API.
/// - `path`: endpoint path of the album.
///
/// # Returns
///
/// The album's covers, smallest first, or an empty list when Deezer does not
/// know the album.
///
/// # Errors
///
/// Returns the provider error of a request that could not be made.
fn covers(base_url: &str, path: &str) -> Result<Vec<Image>, ProviderError> {
    let Some(album) = known(api::get::<deezer::Album>(base_url, path))? else {
        return Ok(Vec::new());
    };

    Ok(map::cover_images(&album))
}

/// Returns the portraits of a Deezer artist, placeholders left out.
///
/// # Parameters
///
/// - `source`: the artist as Deezer returned it.
///
/// # Returns
///
/// One image per real picture URL, smallest first; empty when the artist has no
/// portrait.
fn portraits(source: &deezer::Artist) -> Vec<Image> {
    map::artist_images(source)
}

/// Returns the result of a lookup that may legitimately find nothing.
///
/// Deezer reports an entity it does not know as an error with the "no data"
/// code, which [`crate::api`] translates into a not-found. Neither is a failure
/// of the lookup itself: the caller answers "no picture" instead.
///
/// # Parameters
///
/// - `result`: the outcome of the lookup.
///
/// # Returns
///
/// `Some` with the value, or `None` when the provider does not know the entity.
///
/// # Errors
///
/// Returns every other provider error unchanged.
fn known<T>(result: Result<T, ProviderError>) -> Result<Option<T>, ProviderError> {
    match result {
        Ok(value) => Ok(Some(value)),
        Err(ProviderError::NotFound) => Ok(None),
        Err(error) => Err(error),
    }
}

/// Builds the advanced search text of an album subject.
///
/// Deezer's search understands one `field:"value"` clause per field. The artist
/// clause is left out when the subject credits nobody, because an empty clause
/// matches nothing; the title clause is always sent, since the caller only
/// searches when there is a title.
///
/// # Parameters
///
/// - `subject`: the album whose cover is wanted.
///
/// # Returns
///
/// `artist:"<first artist>" album:"<title>"`, or `album:"<title>"` when the
/// subject credits no artist.
///
/// # Example
///
/// ```text
/// album_query(Random Access Memories by Daft Punk) -> artist:"Daft Punk" album:"Random Access Memories"
/// album_query(Random Access Memories by nobody)    -> album:"Random Access Memories"
/// ```
fn album_query(subject: &ArtworkSubject) -> String {
    let title = clause(&subject.title);

    match subject.artists.first().map(|artist| clause(artist)) {
        Some(artist) if !artist.is_empty() => format!("artist:\"{artist}\" album:\"{title}\""),
        _ => format!("album:\"{title}\""),
    }
}

/// Returns a search clause value without the quotes that would close it.
///
/// # Parameters
///
/// - `value`: the title or artist name as the subject carries it.
///
/// # Returns
///
/// The trimmed value with every quote removed.
fn clause(value: &str) -> String {
    value.replace('"', "").trim().to_string()
}

/// Returns whether a search result is the album a subject describes.
///
/// Deezer's search is fuzzy, so a result is accepted only when its title, and
/// the artist the response credits, are the ones the subject names. A subject
/// that credits nobody accepts any artist, and a subject whose credited artist
/// the response omits is rejected, because nothing proves the result is the
/// right one.
///
/// # Parameters
///
/// - `subject`: the album whose cover is wanted.
/// - `candidate`: one search result.
///
/// # Returns
///
/// `true` when the result is the album the subject describes.
fn matches_album(subject: &ArtworkSubject, candidate: &deezer::Album) -> bool {
    if !same_text(&candidate.title, &subject.title) {
        return false;
    }

    match (subject.artists.first(), candidate.artist.as_ref()) {
        (Some(wanted), Some(found)) => same_text(&found.name, wanted),
        (Some(_), None) => false,
        (None, _) => true,
    }
}

/// Compares two names the way a search result should be compared.
///
/// # Parameters
///
/// - `left`: the name from the response.
/// - `right`: the name from the subject.
///
/// # Returns
///
/// `true` when the two are equal ignoring case and any difference in the
/// amount of whitespace.
fn same_text(left: &str, right: &str) -> bool {
    normalize(left) == normalize(right)
}

/// Folds a name for comparison: lower case, whitespace runs collapsed.
///
/// # Parameters
///
/// - `value`: the name to fold.
///
/// # Returns
///
/// The folded name.
fn normalize(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<&str>>()
        .join(" ")
        .to_lowercase()
}

/// Returns the value of the identifier a subject carries in one namespace.
///
/// # Parameters
///
/// - `ids`: the identifiers the subject carries.
/// - `namespace`: the namespace to look for, for example `upc`.
///
/// # Returns
///
/// The trimmed value of the first identifier in that namespace, or `None` when
/// the subject carries none, or carries a blank one.
fn id_value<'a>(ids: &'a [IdPair], namespace: &str) -> Option<&'a str> {
    ids.iter()
        .filter(|id| id.namespace == namespace)
        .find_map(|id| non_blank(Some(id.value.as_str())))
}

/// Returns a trimmed value, or `None` when it is absent or blank.
///
/// # Parameters
///
/// - `value`: the value as the subject or the API carries it.
///
/// # Returns
///
/// The trimmed value, or `None`.
fn non_blank(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Builds an album subject with no ids and the given artist credits.
    ///
    /// # Parameters
    ///
    /// - `title`: the album title.
    /// - `artists`: the credited artist names, in credit order.
    ///
    /// # Returns
    ///
    /// The subject.
    fn album(title: &str, artists: &[&str]) -> ArtworkSubject {
        ArtworkSubject {
            kind: EntityKind::Album,
            provider_id: None,
            ids: Vec::new(),
            title: title.to_string(),
            artists: artists.iter().map(|name| (*name).to_string()).collect(),
        }
    }

    /// Parses one fixture into the type it holds.
    ///
    /// # Parameters
    ///
    /// - `body`: the fixture as the API would return it.
    ///
    /// # Returns
    ///
    /// The decoded payload.
    fn decode_fixture<T: serde::de::DeserializeOwned>(body: &str) -> T {
        api::decode(200, body.as_bytes(), "https://api.deezer.com/test").unwrap()
    }

    #[test]
    fn builds_the_album_search_text() {
        assert_eq!(
            album_query(&album("Random Access Memories", &["Daft Punk"])),
            "artist:\"Daft Punk\" album:\"Random Access Memories\""
        );
        assert_eq!(
            album_query(&album("Random Access Memories", &[])),
            "album:\"Random Access Memories\""
        );
        assert_eq!(
            album_query(&album("  Homework  ", &["  Daft Punk  ", "Nobody"])),
            "artist:\"Daft Punk\" album:\"Homework\""
        );
    }

    #[test]
    fn a_quote_in_a_name_cannot_close_a_clause() {
        assert_eq!(
            album_query(&album("Say \"Hello\"", &["A \"B\""])),
            "artist:\"A B\" album:\"Say Hello\""
        );
    }

    #[test]
    fn accepts_the_album_a_subject_describes() {
        let results: deezer::Page<deezer::Album> =
            decode_fixture(include_str!("fixtures/search_albums.json"));
        let candidate = &results.data[0];

        assert!(matches_album(
            &album("Random Access Memories", &["Daft Punk"]),
            candidate
        ));
        // Case and whitespace do not decide a match.
        assert!(matches_album(
            &album("  random   access memories ", &["daft  punk"]),
            candidate
        ));
        // A subject that credits nobody accepts the response's artist.
        assert!(matches_album(
            &album("Random Access Memories", &[]),
            candidate
        ));
    }

    #[test]
    fn rejects_the_albums_a_subject_does_not_describe() {
        let results: deezer::Page<deezer::Album> =
            decode_fixture(include_str!("fixtures/search_albums.json"));
        let candidate = &results.data[0];

        assert!(!matches_album(
            &album("Random Access Memories", &["Daft Punkk"]),
            candidate
        ));
        assert!(!matches_album(
            &album("Random Access Memories (Drumless Edition)", &["Daft Punk"]),
            candidate
        ));
    }

    #[test]
    fn accepts_the_artist_a_subject_names() {
        let results: deezer::Page<deezer::Artist> =
            decode_fixture(include_str!("fixtures/search_artists.json"));
        let candidate = &results.data[0];

        assert!(same_text(&candidate.name, "Daft Punk"));
        assert!(same_text(&candidate.name, "  daft   punk "));
        assert!(!same_text(&candidate.name, "Daft Punkk"));
    }

    #[test]
    fn returns_the_real_portraits_of_an_artist() {
        let artist: deezer::Artist = decode_fixture(include_str!("fixtures/artist.json"));
        let images = portraits(&artist);

        assert_eq!(images.len(), 4);
        assert_eq!(images[1].width, Some(250));
    }

    #[test]
    fn leaves_out_the_placeholder_portraits() {
        let artist: deezer::Artist =
            decode_fixture(include_str!("fixtures/artist_placeholder.json"));

        assert!(portraits(&artist).is_empty());
    }

    #[test]
    fn a_lookup_that_finds_nothing_is_not_an_error() {
        assert!(matches!(
            known::<deezer::Album>(Err(ProviderError::NotFound)),
            Ok(None)
        ));
        assert_eq!(known(Ok(7)).unwrap(), Some(7));
        assert!(matches!(
            known::<deezer::Album>(Err(ProviderError::RateLimited(30))),
            Err(ProviderError::RateLimited(30))
        ));
    }

    #[test]
    fn reads_the_upc_of_a_subject() {
        let ids = vec![
            IdPair {
                namespace: "mb-release".to_string(),
                value: "9c9f0e4b".to_string(),
            },
            IdPair {
                namespace: "upc".to_string(),
                value: " 886443927087 ".to_string(),
            },
        ];

        assert_eq!(id_value(&ids, "upc"), Some("886443927087"));
        assert_eq!(id_value(&ids, "isrc"), None);
        assert_eq!(id_value(&[], "upc"), None);
    }
}
