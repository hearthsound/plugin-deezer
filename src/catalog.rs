//! The catalog operations, one function per Deezer endpoint.
//!
//! Each function composes an endpoint path, calls [`crate::api`] and translates
//! the payload with [`crate::map`]. The functions are thin on purpose: the
//! interesting logic is the URL building, which is tested here directly, and
//! the translation, which is tested in [`crate::map`].

use hearthsound_plugin::types::{Album, AlbumRef, Artist, IdPair, ProviderError, Track};

use crate::deezer::{self, Page};
use crate::{api, map, url};

/// Number of album tracks requested per page.
const TRACK_PAGE_SIZE: u32 = 100;

/// The canonical id namespace Deezer can resolve an album from.
const UPC_NAMESPACE: &str = "upc";

/// Upper bound on the tracks read for one album.
///
/// Deezer pages album tracks 100 at a time; the cap keeps a broken or hostile
/// response from paging until the call deadline.
const MAX_ALBUM_TRACKS: usize = 1_000;

/// Returns whether `id` has the shape of a Deezer entity id.
///
/// Deezer mints numeric ids only, so any other identifier cannot name a track,
/// an album or an artist; asking about one earns an "invalid query" error
/// instead of the not-found it really is.
///
/// # Parameters
///
/// - `id`: the identifier a caller supplied.
///
/// # Returns
///
/// `true` when `id` is non-empty and made of ASCII digits only.
fn is_entity_id(id: &str) -> bool {
    !id.is_empty() && id.bytes().all(|byte| byte.is_ascii_digit())
}

/// Refuses an id Deezer cannot have issued, without a request.
///
/// # Parameters
///
/// - `id`: the identifier a caller supplied.
///
/// # Errors
///
/// Returns [`ProviderError::NotFound`] when `id` is not numeric.
fn require_entity_id(id: &str) -> Result<(), ProviderError> {
    if is_entity_id(id) {
        Ok(())
    } else {
        Err(ProviderError::NotFound)
    }
}

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
/// The matching tracks, in Deezer's relevance order, or an empty list when
/// `limit` is zero.
///
/// # Errors
///
/// Returns the provider error of the underlying request. A zero limit sends no
/// request at all, because Deezer reads `limit=0` as "no limit".
pub(crate) fn search_tracks(
    base_url: &str,
    text: &str,
    limit: u32,
    offset: u32,
) -> Result<Vec<Track>, ProviderError> {
    if limit == 0 {
        return Ok(Vec::new());
    }

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
/// The matching album references, in Deezer's relevance order, or an empty
/// list when `limit` is zero.
///
/// # Errors
///
/// Returns the provider error of the underlying request. A zero limit sends no
/// request at all, because Deezer reads `limit=0` as "no limit".
pub(crate) fn search_albums(
    base_url: &str,
    text: &str,
    limit: u32,
    offset: u32,
) -> Result<Vec<AlbumRef>, ProviderError> {
    if limit == 0 {
        return Ok(Vec::new());
    }

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
/// The matching artists, in Deezer's relevance order, or an empty list when
/// `limit` is zero.
///
/// # Errors
///
/// Returns the provider error of the underlying request. A zero limit sends no
/// request at all, because Deezer reads `limit=0` as "no limit".
pub(crate) fn search_artists(
    base_url: &str,
    text: &str,
    limit: u32,
    offset: u32,
) -> Result<Vec<Artist>, ProviderError> {
    if limit == 0 {
        return Ok(Vec::new());
    }

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
/// Returns [`ProviderError::NotFound`] when Deezer does not know the id, and
/// for any id that is not numeric.
pub(crate) fn track(base_url: &str, id: &str) -> Result<Track, ProviderError> {
    require_entity_id(id)?;
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
/// Returns [`ProviderError::NotFound`] when Deezer does not know the id, and
/// for any id that is not numeric.
pub(crate) fn album(base_url: &str, id: &str) -> Result<Album, ProviderError> {
    require_entity_id(id)?;
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
/// Returns [`ProviderError::NotFound`] when Deezer does not know the id, and
/// for any id that is not numeric.
pub(crate) fn artist(base_url: &str, id: &str) -> Result<Artist, ProviderError> {
    require_entity_id(id)?;
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
/// The artist's albums, or an empty list when `limit` is zero.
///
/// # Errors
///
/// Returns [`ProviderError::NotFound`] when Deezer does not know the id, and
/// for any id that is not numeric. A zero limit sends no request at all,
/// because Deezer reads `limit=0` as "no limit".
pub(crate) fn artist_albums(
    base_url: &str,
    id: &str,
    limit: u32,
    offset: u32,
) -> Result<Vec<AlbumRef>, ProviderError> {
    require_entity_id(id)?;
    if limit == 0 {
        return Ok(Vec::new());
    }

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
/// The artist's top tracks, or an empty list when `limit` is zero. Deezer
/// reports no ISRC for these, so the tracks carry the album reference and the
/// other metadata only.
///
/// # Errors
///
/// Returns [`ProviderError::NotFound`] when Deezer does not know the id, and
/// for any id that is not numeric. A zero limit sends no request at all,
/// because Deezer reads `limit=0` as "no limit".
pub(crate) fn artist_top_tracks(
    base_url: &str,
    id: &str,
    limit: u32,
) -> Result<Vec<Track>, ProviderError> {
    require_entity_id(id)?;
    if limit == 0 {
        return Ok(Vec::new());
    }

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

/// Resolves the canonical album ids this provider knows to its album refs.
///
/// Deezer issues no MusicBrainz identifiers, so only the `upc` namespace can
/// match; every other namespace is skipped without a request.
///
/// # Parameters
///
/// - `base_url`: base URL of the API.
/// - `ids`: the canonical ids to look up.
///
/// # Returns
///
/// One reference per `upc` id Deezer knows, in the order the ids were given.
/// Empty when none of them names an album Deezer holds, including when a UPC
/// matches no album.
///
/// # Errors
///
/// Returns the provider error of a request that could not be made. A UPC that
/// matches no album is not an error: the lookup answers whatever it did find.
pub(crate) fn lookup_album(base_url: &str, ids: &[IdPair]) -> Result<Vec<AlbumRef>, ProviderError> {
    let mut albums = Vec::new();

    for pair in ids.iter().filter(|pair| pair.namespace == UPC_NAMESPACE) {
        let upc = pair.value.trim();
        if upc.is_empty() {
            continue;
        }

        let source: deezer::Album = match api::get(base_url, &upc_path(upc)) {
            Ok(source) => source,
            // Deezer answers an unknown UPC with "no data"; a lookup asks
            // about every id it was given, so one miss is not a failure.
            Err(ProviderError::NotFound) => continue,
            Err(error) => return Err(error),
        };

        albums.push(map::album_ref(&source));
    }

    Ok(albums)
}

/// Builds the path of the album endpoint addressing an album by UPC.
///
/// # Parameters
///
/// - `upc`: Universal Product Code of the album.
///
/// # Returns
///
/// `album/upc:<encoded upc>`.
pub(crate) fn upc_path(upc: &str) -> String {
    format!("album/upc:{}", url::encode_component(upc))
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
pub(crate) fn element_path(kind: &str, id: &str) -> String {
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
pub(crate) fn search_path(kind: &str, text: &str, limit: u32, offset: u32) -> String {
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

    #[test]
    fn recognizes_entity_ids() {
        assert!(is_entity_id("0"));
        assert!(is_entity_id("67238732"));
        assert!(!is_entity_id(""));
        assert!(!is_entity_id("abc"));
        assert!(!is_entity_id("-1"));
        assert!(!is_entity_id("1.5"));
        assert!(!is_entity_id(" 1"));
        assert!(!is_entity_id("hsplugin-conformance-unknown-id"));
    }

    #[test]
    fn rejects_ids_deezer_cannot_have_issued() {
        // No host is bound in a unit test, so reaching the network would fail;
        // an id that is not numeric is answered before any request is built.
        let base_url = "https://api.deezer.com";
        let unknown = "hsplugin-conformance-unknown-id";

        assert!(matches!(
            track(base_url, unknown),
            Err(ProviderError::NotFound)
        ));
        assert!(matches!(
            album(base_url, unknown),
            Err(ProviderError::NotFound)
        ));
        assert!(matches!(
            artist(base_url, unknown),
            Err(ProviderError::NotFound)
        ));
        assert!(matches!(
            artist_albums(base_url, unknown, 10, 0),
            Err(ProviderError::NotFound)
        ));
        assert!(matches!(
            artist_top_tracks(base_url, unknown, 10),
            Err(ProviderError::NotFound)
        ));
    }

    #[test]
    fn builds_upc_paths() {
        assert_eq!(upc_path("886443927087"), "album/upc:886443927087");
        assert_eq!(upc_path("0886443927087"), "album/upc:0886443927087");
        // A UPC can never add a path segment or a query parameter.
        assert_eq!(upc_path("1/2?x=1"), "album/upc:1%2F2%3Fx%3D1");
    }

    #[test]
    fn looks_up_only_the_upc_namespace() {
        // No host is bound in a unit test, so a lookup is exercised only with
        // the ids that send no request: no ids at all, other namespaces and a
        // blank UPC.
        let base_url = "https://api.deezer.com";
        let pair = |namespace: &str, value: &str| IdPair {
            namespace: namespace.to_string(),
            value: value.to_string(),
        };

        assert!(lookup_album(base_url, &[]).unwrap().is_empty());

        let ids = [
            pair("mb-release", "f8b3c5b6-0000-0000-0000-000000000000"),
            pair("mb-release-group", "f8b3c5b6-0000-0000-0000-000000000001"),
            pair("upc", "   "),
        ];

        assert!(lookup_album(base_url, &ids).unwrap().is_empty());
    }

    #[test]
    fn answers_a_zero_limit_without_a_request() {
        // Deezer reads `limit=0` as "no limit", so the empty answer is given
        // here instead of asking for everything.
        let base_url = "https://api.deezer.com";

        assert!(matches!(
            search_tracks(base_url, "hello", 0, 0),
            Ok(tracks) if tracks.is_empty()
        ));
        assert!(matches!(
            search_albums(base_url, "hello", 0, 0),
            Ok(albums) if albums.is_empty()
        ));
        assert!(matches!(
            search_artists(base_url, "hello", 0, 0),
            Ok(artists) if artists.is_empty()
        ));
        assert!(matches!(
            artist_albums(base_url, "27", 0, 0),
            Ok(albums) if albums.is_empty()
        ));
        assert!(matches!(
            artist_top_tracks(base_url, "27", 0),
            Ok(tracks) if tracks.is_empty()
        ));
    }
}
