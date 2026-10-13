//! The catalog operations, one function per Deezer endpoint.
//!
//! Each function composes an endpoint path, calls [`crate::api`] and translates
//! the payload with [`crate::map`]. The functions are thin on purpose: the
//! interesting logic is the URL building, which is tested here directly, and
//! the translation, which is tested in [`crate::map`].

use hearthsound_plugin::types::{Album, AlbumRef, Artist, ProviderError, Track};

use crate::deezer::{self, Page};
use crate::{api, map, url};

/// Number of album tracks requested per page.
const TRACK_PAGE_SIZE: u32 = 100;

/// Upper bound on the tracks read for one album.
///
/// Deezer pages album tracks 100 at a time; the cap keeps a broken or hostile
/// response from paging until the call deadline.
const MAX_ALBUM_TRACKS: usize = 1_000;

/// Searches Deezer's mixed index for tracks.
///
/// # Parameters
///
/// - `base_url`: base URL of the API.
/// - `text`: search text.
/// - `limit`: maximum number of results.
/// - `offset`: number of results to skip.
///
/// # Returns
///
/// The matching tracks, in Deezer's relevance order.
///
/// # Errors
///
/// Returns the provider error of the underlying request.
pub(crate) fn search_tracks(
    base_url: &str,
    text: &str,
    limit: u32,
    offset: u32,
) -> Result<Vec<Track>, ProviderError> {
    let page: Page<deezer::Track> =
        api::get(base_url, &search_path("search", text, limit, offset))?;

    Ok(page
        .data
        .iter()
        .map(|track| map::track(track, None))
        .collect())
}

/// Searches Deezer for albums.
///
/// # Parameters
///
/// - `base_url`: base URL of the API.
/// - `text`: search text.
/// - `limit`: maximum number of results.
/// - `offset`: number of results to skip.
///
/// # Returns
///
/// The matching album references, in Deezer's relevance order.
///
/// # Errors
///
/// Returns the provider error of the underlying request.
pub(crate) fn search_albums(
    base_url: &str,
    text: &str,
    limit: u32,
    offset: u32,
) -> Result<Vec<AlbumRef>, ProviderError> {
    let page: Page<deezer::Album> =
        api::get(base_url, &search_path("search/album", text, limit, offset))?;

    Ok(page.data.iter().map(map::album_ref).collect())
}

/// Searches Deezer for artists.
///
/// # Parameters
///
/// - `base_url`: base URL of the API.
/// - `text`: search text.
/// - `limit`: maximum number of results.
/// - `offset`: number of results to skip.
///
/// # Returns
///
/// The matching artists, in Deezer's relevance order.
///
/// # Errors
///
/// Returns the provider error of the underlying request.
pub(crate) fn search_artists(
    base_url: &str,
    text: &str,
    limit: u32,
    offset: u32,
) -> Result<Vec<Artist>, ProviderError> {
    let page: Page<deezer::Artist> =
        api::get(base_url, &search_path("search/artist", text, limit, offset))?;

    Ok(page.data.iter().map(map::artist).collect())
}

/// Reads one track.
///
/// # Parameters
///
/// - `base_url`: base URL of the API.
/// - `id`: Deezer track id.
///
/// # Returns
///
/// The track with its album reference.
///
/// # Errors
///
/// Returns [`ProviderError::NotFound`] when Deezer does not know the id.
pub(crate) fn track(base_url: &str, id: &str) -> Result<Track, ProviderError> {
    let source: deezer::Track = api::get(base_url, &element_path("track", id))?;

    Ok(map::track(&source, None))
}

/// Reads one album together with its tracks.
///
/// The tracks come from `/album/{id}/tracks` rather than from the album
/// response: the embedded listing carries no ISRCs, track numbers or disc
/// numbers and stops at 25 entries, while the dedicated listing carries all
/// three and pages.
///
/// # Parameters
///
/// - `base_url`: base URL of the API.
/// - `id`: Deezer album id.
///
/// # Returns
///
/// The album with its tracks.
///
/// # Errors
///
/// Returns [`ProviderError::NotFound`] when Deezer does not know the id.
pub(crate) fn album(base_url: &str, id: &str) -> Result<Album, ProviderError> {
    let source: deezer::Album = api::get(base_url, &element_path("album", id))?;
    let tracks = album_tracks(base_url, id)?;

    Ok(map::album(&source, &tracks))
}

/// Reads one artist.
///
/// # Parameters
///
/// - `base_url`: base URL of the API.
/// - `id`: Deezer artist id.
///
/// # Returns
///
/// The artist with its pictures.
///
/// # Errors
///
/// Returns [`ProviderError::NotFound`] when Deezer does not know the id.
pub(crate) fn artist(base_url: &str, id: &str) -> Result<Artist, ProviderError> {
    let source: deezer::Artist = api::get(base_url, &element_path("artist", id))?;

    Ok(map::artist(&source))
}

/// Reads the albums of an artist.
///
/// Deezer's artist album listing carries no album artist, so the returned
/// references have no artist credits; the client already knows which artist it
/// asked for.
///
/// # Parameters
///
/// - `base_url`: base URL of the API.
/// - `id`: Deezer artist id.
/// - `limit`: maximum number of albums.
/// - `offset`: number of albums to skip.
///
/// # Returns
///
/// The artist's albums.
///
/// # Errors
///
/// Returns [`ProviderError::NotFound`] when Deezer does not know the id.
pub(crate) fn artist_albums(
    base_url: &str,
    id: &str,
    limit: u32,
    offset: u32,
) -> Result<Vec<AlbumRef>, ProviderError> {
    let page: Page<deezer::Album> = api::get(
        base_url,
        &listing_path("artist", id, "albums", limit, offset),
    )?;

    Ok(page.data.iter().map(map::album_ref).collect())
}

/// Reads the most popular tracks of an artist.
///
/// # Parameters
///
/// - `base_url`: base URL of the API.
/// - `id`: Deezer artist id.
/// - `limit`: maximum number of tracks.
///
/// # Returns
///
/// The artist's top tracks. Deezer reports no ISRC for these, so the tracks
/// carry the album reference and the other metadata only.
///
/// # Errors
///
/// Returns [`ProviderError::NotFound`] when Deezer does not know the id.
pub(crate) fn artist_top_tracks(
    base_url: &str,
    id: &str,
    limit: u32,
) -> Result<Vec<Track>, ProviderError> {
    let page: Page<deezer::Track> = api::get(
        base_url,
        &format!("{}?limit={limit}", listing("artist", id, "top")),
    )?;

    Ok(page
        .data
        .iter()
        .map(|track| map::track(track, None))
        .collect())
}

/// Resolves an ISRC to the track that carries it.
///
/// # Parameters
///
/// - `base_url`: base URL of the API.
/// - `isrc`: International Standard Recording Code, with or without hyphens,
///   in any case.
///
/// # Returns
///
/// A one-element list with the track, or an empty list when `isrc` is blank.
///
/// # Errors
///
/// Returns [`ProviderError::NotFound`] when Deezer has no track for the code.
pub(crate) fn lookup_isrc(base_url: &str, isrc: &str) -> Result<Vec<Track>, ProviderError> {
    let isrc = normalize_isrc(isrc);
    if isrc.is_empty() {
        return Ok(Vec::new());
    }

    let source: deezer::Track = api::get(
        base_url,
        &format!("track/isrc:{}", url::encode_component(&isrc)),
    )?;

    Ok(vec![map::track(&source, None)])
}

/// Checks that the API answers.
///
/// The genre list is the cheapest endpoint that proves the base URL is a
/// Deezer API: it returns a short list and takes no parameters.
///
/// # Parameters
///
/// - `base_url`: base URL of the API.
///
/// # Errors
///
/// Returns the provider error of the underlying request.
pub(crate) fn health(base_url: &str) -> Result<(), ProviderError> {
    let _genres: Page<deezer::Genre> = api::get(base_url, "genre")?;

    Ok(())
}

/// Reads every track of an album, one Deezer page at a time.
///
/// # Parameters
///
/// - `base_url`: base URL of the API.
/// - `id`: Deezer album id.
///
/// # Returns
///
/// The album's tracks in Deezer's order, at most [`MAX_ALBUM_TRACKS`] of them.
///
/// # Errors
///
/// Returns the provider error of the request that fails first.
fn album_tracks(base_url: &str, id: &str) -> Result<Vec<deezer::Track>, ProviderError> {
    let mut tracks: Vec<deezer::Track> = Vec::new();
    let mut offset = 0;

    while tracks.len() < MAX_ALBUM_TRACKS {
        let page: Page<deezer::Track> = api::get(
            base_url,
            &listing_path("album", id, "tracks", TRACK_PAGE_SIZE, offset),
        )?;
        let received = page.data.len();
        let last_page = match page.total {
            Some(total) => u64::from(offset) + received as u64 >= total,
            None => received < TRACK_PAGE_SIZE as usize,
        };

        tracks.extend(page.data);
        if received == 0 || last_page {
            break;
        }
        // Advance by what Deezer returned rather than by the requested page
        // size, so a short page never skips tracks.
        offset += received as u32;
    }

    Ok(tracks)
}

/// Builds the path of an endpoint addressing a single entity.
///
/// # Parameters
///
/// - `kind`: collection name, for example `track`.
/// - `id`: entity id.
///
/// # Returns
///
/// `<kind>/<encoded id>`.
fn element_path(kind: &str, id: &str) -> String {
    format!("{kind}/{}", url::encode_component(id))
}

/// Builds the path of a collection below an entity.
///
/// # Parameters
///
/// - `kind`: collection name, for example `artist`.
/// - `id`: entity id.
/// - `listing`: sub-collection name, for example `albums`.
/// - `limit`: maximum number of results.
/// - `offset`: number of results to skip.
///
/// # Returns
///
/// `<kind>/<encoded id>/<listing>?limit=<limit>&index=<offset>`.
fn listing_path(kind: &str, id: &str, listing_name: &str, limit: u32, offset: u32) -> String {
    format!(
        "{}?limit={limit}&index={offset}",
        listing(kind, id, listing_name)
    )
}

/// Builds the path of a collection below an entity, without a query string.
///
/// # Parameters
///
/// - `kind`: collection name, for example `artist`.
/// - `id`: entity id.
/// - `listing_name`: sub-collection name, for example `albums`.
///
/// # Returns
///
/// `<kind>/<encoded id>/<listing>`.
fn listing(kind: &str, id: &str, listing_name: &str) -> String {
    format!("{}/{listing_name}", element_path(kind, id))
}

/// Builds the path of a search endpoint.
///
/// # Parameters
///
/// - `kind`: search endpoint, for example `search` or `search/album`.
/// - `text`: search text, percent-encoded by this function.
/// - `limit`: maximum number of results.
/// - `offset`: number of results to skip, sent as Deezer's `index`.
///
/// # Returns
///
/// `<kind>?q=<encoded text>&limit=<limit>&index=<offset>`.
fn search_path(kind: &str, text: &str, limit: u32, offset: u32) -> String {
    format!(
        "{kind}?q={}&limit={limit}&index={offset}",
        url::encode_component(text)
    )
}

/// Normalizes an ISRC to the form Deezer accepts.
///
/// Deezer expects the twelve characters of the code without separators, and
/// answers "no data" for a hyphenated code, so hyphens and spaces are removed
/// and the remainder upper-cased.
///
/// # Parameters
///
/// - `value`: the code as the caller wrote it.
///
/// # Returns
///
/// The code without separators, in upper case.
fn normalize_isrc(value: &str) -> String {
    value
        .chars()
        .filter(|character| !matches!(character, '-' | ' '))
        .collect::<String>()
        .to_ascii_uppercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_search_paths() {
        assert_eq!(
            search_path("search", "Daft Punk", 25, 0),
            "search?q=Daft%20Punk&limit=25&index=0"
        );
        assert_eq!(
            search_path("search/album", "AC/DC & friends", 10, 100),
            "search/album?q=AC%2FDC%20%26%20friends&limit=10&index=100"
        );
        assert_eq!(
            search_path("search/artist", "a=1&b=2", 1, 2),
            "search/artist?q=a%3D1%26b%3D2&limit=1&index=2"
        );
    }

    #[test]
    fn builds_element_paths() {
        assert_eq!(element_path("track", "67238732"), "track/67238732");
        assert_eq!(element_path("album", "6575789"), "album/6575789");
        assert_eq!(element_path("artist", "27"), "artist/27");
        // An id can never add a path segment or a query parameter.
        assert_eq!(element_path("track", "../album/1"), "track/..%2Falbum%2F1");
    }

    #[test]
    fn builds_listing_paths() {
        assert_eq!(
            listing_path("album", "6575789", "tracks", 100, 0),
            "album/6575789/tracks?limit=100&index=0"
        );
        assert_eq!(
            listing_path("artist", "27", "albums", 25, 50),
            "artist/27/albums?limit=25&index=50"
        );
        assert_eq!(listing("artist", "27", "top"), "artist/27/top");
    }

    #[test]
    fn normalizes_isrcs() {
        assert_eq!(normalize_isrc("USQX91300105"), "USQX91300105");
        assert_eq!(normalize_isrc("usqx91300105"), "USQX91300105");
        assert_eq!(normalize_isrc("US-QX9-13-00105"), "USQX91300105");
        assert_eq!(normalize_isrc(" us qx9 1300105 "), "USQX91300105");
        assert_eq!(normalize_isrc(""), "");
        assert_eq!(normalize_isrc("---"), "");
    }
}
