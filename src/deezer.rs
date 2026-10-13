//! Serde types for the payloads of the Deezer public API.
//!
//! The types mirror the wire format rather than the catalog records: ids stay
//! numeric, durations stay in seconds, and every field the same object does not
//! always carry is an [`Option`]. Deezer returns the same shape with different
//! field sets depending on the endpoint (a track inside an album listing has no
//! `isrc`, an album inside an artist listing has no `artist`), so the translation
//! into catalog records - which is where the defaults are chosen - lives in
//! [`crate::map`].
//!
//! Unknown fields are ignored, so a new Deezer field never breaks the parse.

use serde::Deserialize;

/// A page of results, as returned by every Deezer list endpoint.
///
/// # Example
///
/// A search page looks like this:
///
/// ```text
/// {"data": [{"id": 67238732, "title": "Instant Crush", "duration": 337}], "total": 177}
/// ```
///
/// and parses into a [`Page`] with one [`Track`] and `total` of `Some(177)`.
#[derive(Debug, Clone, Deserialize)]
#[serde(bound(deserialize = "T: Deserialize<'de>"))]
pub struct Page<T> {
    /// The entries in this page.
    #[serde(default)]
    pub data: Vec<T>,
    /// Total number of entries the query matches, when the endpoint reports it.
    #[serde(default)]
    pub total: Option<u64>,
}

/// The `error` object of a Deezer error response.
///
/// Deezer answers most failures with HTTP status 200 and a body of
/// `{"error": {...}}`, so this is the only reliable signal that a call failed.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Error {
    /// Error type, for example `DataException` or `ParameterException`.
    #[serde(default, rename = "type")]
    pub kind: Option<String>,
    /// Human-readable message.
    #[serde(default)]
    pub message: Option<String>,
    /// Numeric error code; `800` means "no data" and `4` means "quota exceeded".
    #[serde(default)]
    pub code: Option<i64>,
}

/// An artist, as embedded in track and album responses.
#[derive(Debug, Clone, Deserialize)]
pub struct Artist {
    /// Deezer artist id.
    pub id: u64,
    /// Display name; empty when the response omitted it.
    #[serde(default)]
    pub name: String,
    /// 56x56 picture URL.
    #[serde(default)]
    pub picture_small: Option<String>,
    /// 250x250 picture URL.
    #[serde(default)]
    pub picture_medium: Option<String>,
    /// 500x500 picture URL.
    #[serde(default)]
    pub picture_big: Option<String>,
    /// 1000x1000 picture URL.
    #[serde(default)]
    pub picture_xl: Option<String>,
}

/// A track, as returned by a track endpoint or embedded in another response.
#[derive(Debug, Clone, Deserialize)]
pub struct Track {
    /// Deezer track id.
    pub id: u64,
    /// Track title, including any version suffix such as `(feat. ...)`.
    #[serde(default)]
    pub title: String,
    /// Track length in seconds.
    #[serde(default)]
    pub duration: Option<u32>,
    /// Position of the track on its disc; absent from search results.
    #[serde(default)]
    pub track_position: Option<u32>,
    /// Disc number; absent from search results.
    #[serde(default)]
    pub disk_number: Option<u32>,
    /// International Standard Recording Code; absent from artist top tracks.
    #[serde(default)]
    pub isrc: Option<String>,
    /// Whether Deezer flags the track as explicit.
    #[serde(default)]
    pub explicit_lyrics: Option<bool>,
    /// Performing artist; absent from some listings.
    #[serde(default)]
    pub artist: Option<Artist>,
    /// Containing album; absent from album track listings.
    #[serde(default)]
    pub album: Option<Album>,
}

/// An album, as returned by an album endpoint or embedded in another response.
#[derive(Debug, Clone, Deserialize)]
pub struct Album {
    /// Deezer album id.
    pub id: u64,
    /// Album title.
    #[serde(default)]
    pub title: String,
    /// Universal Product Code; only the album endpoints report it.
    #[serde(default)]
    pub upc: Option<String>,
    /// Release date, formatted `YYYY-MM-DD`.
    #[serde(default)]
    pub release_date: Option<String>,
    /// Deezer's release type, for example `album`, `single` or `ep`.
    #[serde(default)]
    pub record_type: Option<String>,
    /// Main artist; absent from an artist's album listing.
    #[serde(default)]
    pub artist: Option<Artist>,
    /// Genres of the album; only the album endpoints report them.
    #[serde(default)]
    pub genres: Option<Genres>,
    /// 56x56 cover URL.
    #[serde(default)]
    pub cover_small: Option<String>,
    /// 250x250 cover URL.
    #[serde(default)]
    pub cover_medium: Option<String>,
    /// 500x500 cover URL.
    #[serde(default)]
    pub cover_big: Option<String>,
    /// 1000x1000 cover URL.
    #[serde(default)]
    pub cover_xl: Option<String>,
}

/// The `genres` object of an album response.
#[derive(Debug, Clone, Deserialize)]
pub struct Genres {
    /// The album's genres.
    #[serde(default)]
    pub data: Vec<Genre>,
}

/// A genre, as embedded in an album response.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Genre {
    /// Genre name, for example `Dance`.
    #[serde(default)]
    pub name: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_search_page() {
        let page: Page<Track> =
            serde_json::from_str(include_str!("fixtures/search_tracks.json")).unwrap();

        assert_eq!(page.total, Some(177));
        assert_eq!(page.data.len(), 1);
        let track = &page.data[0];
        assert_eq!(track.id, 67_238_732);
        assert_eq!(track.title, "Instant Crush (feat. Julian Casablancas)");
        assert_eq!(track.duration, Some(337));
        assert_eq!(track.isrc.as_deref(), Some("USQX91300105"));
        assert_eq!(track.explicit_lyrics, Some(false));
        // A search result carries no track number, disc number or album release date.
        assert_eq!(track.track_position, None);
        assert_eq!(track.disk_number, None);
        assert_eq!(track.album.as_ref().unwrap().release_date, None);
    }

    #[test]
    fn parses_the_album_endpoint() {
        let album: Album = serde_json::from_str(include_str!("fixtures/album.json")).unwrap();

        assert_eq!(album.id, 6_575_789);
        assert_eq!(album.upc.as_deref(), Some("886443927087"));
        assert_eq!(album.release_date.as_deref(), Some("2013-05-17"));
        assert_eq!(album.record_type.as_deref(), Some("album"));
        assert_eq!(album.artist.as_ref().unwrap().name, "Daft Punk");
        let genres = album.genres.unwrap();
        assert_eq!(genres.data.len(), 2);
        assert_eq!(genres.data[1].name, "Pop");
        assert!(album.cover_xl.is_some());
    }

    #[test]
    fn parses_an_album_track_listing() {
        let page: Page<Track> =
            serde_json::from_str(include_str!("fixtures/album_tracks.json")).unwrap();

        assert_eq!(page.total, Some(13));
        let first = &page.data[0];
        assert_eq!(first.track_position, Some(1));
        assert_eq!(first.disk_number, Some(1));
        assert_eq!(first.isrc.as_deref(), Some("USQX91300101"));
        // The track listing of an album has no album object of its own.
        assert!(first.album.is_none());
    }

    #[test]
    fn parses_the_artist_endpoint_and_top_tracks() {
        let artist: Artist = serde_json::from_str(include_str!("fixtures/artist.json")).unwrap();
        assert_eq!(artist.id, 27);
        assert_eq!(artist.name, "Daft Punk");
        assert!(artist.picture_small.is_some());

        let top: Page<Track> =
            serde_json::from_str(include_str!("fixtures/artist_top.json")).unwrap();
        assert_eq!(top.total, Some(100));
        // Top tracks report no ISRC but do embed the album.
        assert_eq!(top.data[0].isrc, None);
        assert_eq!(
            top.data[0].album.as_ref().unwrap().title,
            "Random Access Memories"
        );
    }

    #[test]
    fn parses_search_pages_for_albums_and_artists() {
        let albums: Page<Album> =
            serde_json::from_str(include_str!("fixtures/search_albums.json")).unwrap();
        // A searched album carries its artist, but no UPC and no release date.
        assert_eq!(albums.data[0].upc, None);
        assert_eq!(albums.data[0].release_date, None);
        assert_eq!(albums.data[0].artist.as_ref().unwrap().name, "Daft Punk");

        let artists: Page<Artist> =
            serde_json::from_str(include_str!("fixtures/search_artists.json")).unwrap();
        assert_eq!(artists.total, Some(16));
        assert_eq!(artists.data[0].id, 27);
        assert_eq!(artists.data[0].name, "Daft Punk");
        assert!(artists.data[0].picture_xl.is_some());
    }

    #[test]
    fn parses_an_artist_album_listing() {
        let page: Page<Album> =
            serde_json::from_str(include_str!("fixtures/artist_albums.json")).unwrap();

        let album = &page.data[0];
        assert_eq!(album.title, "Random Access Memories (Drumless Edition)");
        assert_eq!(album.release_date.as_deref(), Some("2023-11-17"));
        // An artist's album listing reports no album artist.
        assert!(album.artist.is_none());
    }

    #[test]
    fn parses_the_error_object() {
        let value: serde_json::Value =
            serde_json::from_str(include_str!("fixtures/error_no_data.json")).unwrap();
        let error: Error = serde_json::from_value(value["error"].clone()).unwrap();

        assert_eq!(error.kind.as_deref(), Some("DataException"));
        assert_eq!(error.code, Some(800));
        assert_eq!(error.message.as_deref(), Some("no data"));
    }

    #[test]
    fn ignores_unknown_fields() {
        let page: Page<Genre> = serde_json::from_str(
            r#"{"data":[{"name":"Pop","picture":"https://example.org/p.png"}],"total":null}"#,
        )
        .unwrap();

        assert_eq!(page.data[0].name, "Pop");
        assert_eq!(page.total, None);
    }
}
