//! Translation from Deezer API records to catalog records.
//!
//! Deezer ids are numeric and the entity's own id is the Deezer id as a
//! string; the server namespaces that id per provider instance, so the plugin
//! never invents a namespace for the entity. The canonical identifiers Deezer
//! does know - a track's ISRC and an album's UPC - become `isrc` and `upc` id
//! pairs.
//!
//! A credited artist is different: the credit is only a name unless the plugin
//! says which Deezer artist it is, so every credit carries the artist's Deezer
//! id under the `ARTIST_NAMESPACE` that is this plugin's own. The server
//! treats the first identifier of a credit whose namespace is not one of the
//! canonical ones as the id the issuing instance resolves.
//!
//! Every function here is pure, so its unit tests run on the host target.

use hearthsound_plugin::types::{Album, AlbumRef, Artist, ArtistCredit, IdPair, Image, Track};

use crate::deezer;

/// The namespace of the Deezer artist ids an artist credit carries.
///
/// It is deliberately not one of the canonical namespaces the server knows
/// (`isrc`, `upc`, `mb-recording`, `mb-release`, `mb-release-group`,
/// `mb-artist`): a credit's first identifier outside that set is the provider
/// id the server uses to resolve the credited artist.
const ARTIST_NAMESPACE: &str = "deezer";

/// Width and height reported for a `cover_small` or `picture_small` URL.
const SIZE_SMALL: u32 = 56;

/// Width and height reported for a `cover_medium` or `picture_medium` URL.
const SIZE_MEDIUM: u32 = 250;

/// Width and height reported for a `cover_big` or `picture_big` URL.
const SIZE_BIG: u32 = 500;

/// Width and height reported for a `cover_xl` or `picture_xl` URL.
const SIZE_XL: u32 = 1000;

/// Converts a Deezer artist into an artist credit.
///
/// The credit carries the artist's Deezer id under the plugin's own namespace,
/// so the server can resolve the credited artist against this instance instead
/// of showing a name that points at nothing.
///
/// # Parameters
///
/// - `source`: the artist as Deezer returned it.
///
/// # Returns
///
/// The artist's name, credited with its Deezer artist id.
pub fn artist_credit(source: &deezer::Artist) -> ArtistCredit {
    ArtistCredit {
        name: source.name.clone(),
        ids: ids(ARTIST_NAMESPACE, Some(source.id.to_string())),
    }
}

/// Converts the artists of a Deezer record into artist credits.
///
/// # Parameters
///
/// - `source`: the embedded artist, absent from some Deezer responses.
///
/// # Returns
///
/// A one-element list with the artist, or an empty list when the response
/// omitted the artist object.
pub fn artist_credits(source: Option<&deezer::Artist>) -> Vec<ArtistCredit> {
    source.map(artist_credit).into_iter().collect()
}

/// Converts a Deezer artist into a catalog artist.
///
/// # Parameters
///
/// - `source`: the artist as Deezer returned it.
///
/// # Returns
///
/// The artist with its id, name and pictures.
pub fn artist(source: &deezer::Artist) -> Artist {
    Artist {
        id: source.id.to_string(),
        name: source.name.clone(),
        images: picture_images(source),
        ids: Vec::new(),
    }
}

/// Returns the cover images of a Deezer album, ordered small to large.
///
/// # Parameters
///
/// - `source`: the album as Deezer returned it.
///
/// # Returns
///
/// One image per cover URL the response carried, each with its known size.
pub fn cover_images(source: &deezer::Album) -> Vec<Image> {
    images([
        (source.cover_small.as_deref(), SIZE_SMALL),
        (source.cover_medium.as_deref(), SIZE_MEDIUM),
        (source.cover_big.as_deref(), SIZE_BIG),
        (source.cover_xl.as_deref(), SIZE_XL),
    ])
}

/// Returns the pictures of a Deezer artist, ordered small to large.
///
/// # Parameters
///
/// - `source`: the artist as Deezer returned it.
///
/// # Returns
///
/// One image per picture URL the response carried, each with its known size.
pub fn picture_images(source: &deezer::Artist) -> Vec<Image> {
    images([
        (source.picture_small.as_deref(), SIZE_SMALL),
        (source.picture_medium.as_deref(), SIZE_MEDIUM),
        (source.picture_big.as_deref(), SIZE_BIG),
        (source.picture_xl.as_deref(), SIZE_XL),
    ])
}

/// Returns whether a picture URL is one of Deezer's placeholders.
///
/// An artist without a portrait still gets picture URLs, with the hash segment
/// of the path left empty (`.../images/artist//56x56-000000-80-0-0.jpg`); that
/// URL serves a flat black square rather than a portrait. A URL with an empty
/// path segment is such a placeholder, a URL without one is a real picture.
///
/// # Parameters
///
/// - `url`: the picture URL as the API reported it.
///
/// # Returns
///
/// `true` when the URL carries an empty segment in its path.
///
/// # Example
///
/// ```text
/// is_placeholder_picture("https://cdn-images.dzcdn.net/images/artist//56x56.jpg")   -> true
/// is_placeholder_picture("https://cdn-images.dzcdn.net/images/artist/638e69/56x56.jpg") -> false
/// ```
pub fn is_placeholder_picture(url: &str) -> bool {
    let path = url.split_once("://").map_or(url, |(_, rest)| rest);

    path.contains("//")
}

/// Converts a Deezer album into a catalog album reference.
///
/// # Parameters
///
/// - `source`: the album as Deezer returned it.
///
/// # Returns
///
/// The album's id, title, artists, release year and covers.
pub fn album_ref(source: &deezer::Album) -> AlbumRef {
    AlbumRef {
        id: source.id.to_string(),
        title: source.title.clone(),
        artists: artist_credits(source.artist.as_ref()),
        year: year(source.release_date.as_deref()),
        images: cover_images(source),
    }
}

/// Converts a Deezer album and its track list into a catalog album.
///
/// # Parameters
///
/// - `source`: the album as returned by the album endpoint.
/// - `tracks`: the album's tracks, as returned by the album track listing.
///
/// # Returns
///
/// The complete album: metadata from `source`, tracks from `tracks`, each track
/// pointing back at this album.
pub fn album(source: &deezer::Album, tracks: &[deezer::Track]) -> Album {
    let reference = album_ref(source);
    let album_tracks: Vec<Track> = tracks
        .iter()
        .map(|source| track(source, Some(&reference)))
        .collect();

    Album {
        id: reference.id,
        title: reference.title,
        artists: reference.artists,
        release_date: source.release_date.clone(),
        album_type: source.record_type.clone(),
        upc: non_empty(source.upc.as_deref()),
        genres: genres(source),
        images: reference.images,
        tracks: album_tracks,
        ids: album_ids(source),
    }
}

/// Converts a Deezer track into a catalog track.
///
/// # Parameters
///
/// - `source`: the track as Deezer returned it.
/// - `fallback_album`: album reference to use when the response carried no
///   album object, which is the case for the track listing of an album.
///
/// # Returns
///
/// The track with its Deezer id, duration in milliseconds, track and disc
/// numbers, ISRC and explicit flag.
pub fn track(source: &deezer::Track, fallback_album: Option<&AlbumRef>) -> Track {
    Track {
        id: source.id.to_string(),
        title: source.title.clone(),
        artists: artist_credits(source.artist.as_ref()),
        album: source
            .album
            .as_ref()
            .map(album_ref)
            .or_else(|| fallback_album.cloned()),
        duration_ms: source.duration.map(|seconds| seconds.saturating_mul(1_000)),
        disc_number: source.disk_number.map(as_u16),
        track_number: source.track_position.map(as_u16),
        isrc: non_empty(source.isrc.as_deref()),
        explicit: source.explicit_lyrics,
        ids: track_ids(source),
    }
}

/// Returns the genre names of a Deezer album.
///
/// # Parameters
///
/// - `source`: the album as Deezer returned it.
///
/// # Returns
///
/// The genre names in the order Deezer reported them, empty when the response
/// carried none.
pub fn genres(source: &deezer::Album) -> Vec<String> {
    source
        .genres
        .as_ref()
        .map(|genres| {
            genres
                .data
                .iter()
                .filter_map(|genre| non_empty(Some(genre.name.as_str())))
                .collect()
        })
        .unwrap_or_default()
}

/// Returns the canonical id pairs of a Deezer track.
///
/// # Parameters
///
/// - `source`: the track as Deezer returned it.
///
/// # Returns
///
/// A single `isrc` pair when the response carried an ISRC, otherwise nothing.
pub fn track_ids(source: &deezer::Track) -> Vec<IdPair> {
    ids("isrc", non_empty(source.isrc.as_deref()))
}

/// Returns the canonical id pairs of a Deezer album.
///
/// # Parameters
///
/// - `source`: the album as Deezer returned it.
///
/// # Returns
///
/// A single `upc` pair when the response carried a UPC, otherwise nothing.
pub fn album_ids(source: &deezer::Album) -> Vec<IdPair> {
    ids("upc", non_empty(source.upc.as_deref()))
}

/// Returns the year of a Deezer release date.
///
/// # Parameters
///
/// - `release_date`: the date as Deezer formatted it, `YYYY-MM-DD`.
///
/// # Returns
///
/// The year, or `None` when the date is absent or does not start with four
/// digits.
///
/// # Example
///
/// ```text
/// year(Some("2013-05-17")) -> Some(2013)
/// year(Some("unknown"))    -> None
/// year(None)               -> None
/// ```
pub fn year(release_date: Option<&str>) -> Option<u16> {
    release_date?.get(0..4)?.parse().ok()
}

/// Builds an id pair list holding one value.
///
/// # Parameters
///
/// - `namespace`: namespace of the identifier, for example `isrc`.
/// - `value`: the identifier, or `None` when the response omitted it.
///
/// # Returns
///
/// A one-element list, or an empty list for a `None` value.
fn ids(namespace: &str, value: Option<String>) -> Vec<IdPair> {
    value
        .map(|value| {
            vec![IdPair {
                namespace: namespace.to_string(),
                value,
            }]
        })
        .unwrap_or_default()
}

/// Builds an image list from the four sized URLs of a Deezer object.
///
/// # Parameters
///
/// - `entries`: URL and edge length of each size, smallest first.
///
/// # Returns
///
/// One image per present, non-empty URL.
fn images(entries: [(Option<&str>, u32); 4]) -> Vec<Image> {
    entries
        .into_iter()
        .filter_map(|(url, size)| {
            let url = url?.trim();
            if url.is_empty() {
                return None;
            }
            Some(Image {
                url: url.to_string(),
                width: Some(size),
                height: Some(size),
            })
        })
        .collect()
}

/// Returns a trimmed copy of `value`, or `None` when it is absent or blank.
///
/// # Parameters
///
/// - `value`: the string as the API returned it.
///
/// # Returns
///
/// The trimmed string, or `None`.
fn non_empty(value: Option<&str>) -> Option<String> {
    let value = value?.trim();
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}

/// Narrows a Deezer number to the 16-bit catalog field, saturating at its maximum.
///
/// # Parameters
///
/// - `value`: the number as the API returned it.
///
/// # Returns
///
/// The number, or [`u16::MAX`] when it is larger.
fn as_u16(value: u32) -> u16 {
    value.min(u32::from(u16::MAX)) as u16
}

#[cfg(test)]
mod tests {
    use super::*;

    fn search_track() -> deezer::Track {
        let page: deezer::Page<deezer::Track> =
            serde_json::from_str(include_str!("fixtures/search_tracks.json")).unwrap();
        page.data.into_iter().next().unwrap()
    }

    fn album_source() -> deezer::Album {
        serde_json::from_str(include_str!("fixtures/album.json")).unwrap()
    }

    fn album_tracks() -> Vec<deezer::Track> {
        let page: deezer::Page<deezer::Track> =
            serde_json::from_str(include_str!("fixtures/album_tracks.json")).unwrap();
        page.data
    }

    fn artist_source() -> deezer::Artist {
        serde_json::from_str(include_str!("fixtures/artist.json")).unwrap()
    }

    /// Renders id pairs as tuples, because the generated `IdPair` is not comparable.
    fn id_pairs(pairs: &[IdPair]) -> Vec<(&str, &str)> {
        pairs
            .iter()
            .map(|pair| (pair.namespace.as_str(), pair.value.as_str()))
            .collect()
    }

    #[test]
    fn maps_a_search_track() {
        let track = track(&search_track(), None);

        assert_eq!(track.id, "67238732");
        assert_eq!(track.title, "Instant Crush (feat. Julian Casablancas)");
        assert_eq!(track.artists.len(), 1);
        assert_eq!(track.artists[0].name, "Daft Punk");
        assert_eq!(id_pairs(&track.artists[0].ids), vec![("deezer", "27")]);
        assert_eq!(track.duration_ms, Some(337_000));
        assert_eq!(track.isrc.as_deref(), Some("USQX91300105"));
        assert_eq!(track.explicit, Some(false));
        assert_eq!(track.track_number, None);
        assert_eq!(track.disc_number, None);
        assert_eq!(id_pairs(&track.ids), vec![("isrc", "USQX91300105")]);

        let album = track.album.unwrap();
        assert_eq!(album.id, "6575789");
        assert_eq!(album.year, None);
        assert_eq!(album.images.len(), 4);
        assert_eq!(album.images[0].width, Some(56));
        assert_eq!(album.images[3].width, Some(1000));
    }

    #[test]
    fn maps_an_album_with_its_tracks() {
        let album = album(&album_source(), &album_tracks());

        assert_eq!(album.id, "6575789");
        assert_eq!(album.title, "Random Access Memories");
        assert_eq!(album.release_date.as_deref(), Some("2013-05-17"));
        assert_eq!(album.album_type.as_deref(), Some("album"));
        assert_eq!(album.upc.as_deref(), Some("886443927087"));
        assert_eq!(album.genres, vec!["Dance".to_string(), "Pop".to_string()]);
        assert_eq!(id_pairs(&album.ids), vec![("upc", "886443927087")]);
        assert_eq!(album.artists.len(), 1);
        assert_eq!(id_pairs(&album.artists[0].ids), vec![("deezer", "27")]);
        assert_eq!(album.tracks.len(), 2);

        let first = &album.tracks[0];
        assert_eq!(first.track_number, Some(1));
        assert_eq!(first.disc_number, Some(1));
        assert_eq!(first.duration_ms, Some(274_000));
        // The track listing carries the artist, so the credit names its Deezer id.
        assert_eq!(id_pairs(&first.artists[0].ids), vec![("deezer", "27")]);
        // The track listing has no album object, so the album comes from the parent.
        let referenced = first.album.as_ref().unwrap();
        assert_eq!(referenced.id, "6575789");
        assert_eq!(referenced.title, "Random Access Memories");
        assert_eq!(referenced.year, Some(2013));
    }

    #[test]
    fn maps_an_artist() {
        let artist = artist(&artist_source());

        assert_eq!(artist.id, "27");
        assert_eq!(artist.name, "Daft Punk");
        assert!(artist.ids.is_empty());
        assert_eq!(artist.images.len(), 4);
        assert_eq!(artist.images[1].width, Some(250));
    }

    #[test]
    fn spots_placeholder_pictures() {
        assert!(is_placeholder_picture(
            "https://cdn-images.dzcdn.net/images/artist//56x56-000000-80-0-0.jpg"
        ));
        assert!(!is_placeholder_picture(
            "https://cdn-images.dzcdn.net/images/artist/638e69b9caaf9f9f3f8826febea7b543/56x56-000000-80-0-0.jpg"
        ));
    }

    #[test]
    fn credits_carry_the_deezer_artist_id() {
        let source = artist_source();
        let credit = artist_credit(&source);

        assert_eq!(credit.name, "Daft Punk");
        assert_eq!(id_pairs(&credit.ids), vec![("deezer", "27")]);
        assert!(
            !["isrc", "upc", "mb-artist"].contains(&credit.ids[0].namespace.as_str()),
            "the artist id must not claim a canonical namespace"
        );
    }

    #[test]
    fn a_record_without_an_artist_credits_nobody() {
        assert!(artist_credits(None).is_empty());
        assert_eq!(artist_credits(Some(&artist_source())).len(), 1);
    }

    #[test]
    fn maps_an_artist_album_listing_without_artists() {
        let page: deezer::Page<deezer::Album> =
            serde_json::from_str(include_str!("fixtures/artist_albums.json")).unwrap();

        let album = album_ref(&page.data[0]);

        assert_eq!(album.id, "494309801");
        assert_eq!(album.year, Some(2023));
        // Deezer's artist album listing carries no album artist.
        assert!(album.artists.is_empty());
        assert_eq!(album.images.len(), 4);
    }

    #[test]
    fn maps_an_isrc_lookup_response() {
        let source: deezer::Track =
            serde_json::from_str(include_str!("fixtures/track.json")).unwrap();
        let track = track(&source, None);

        assert_eq!(track.id, "67238732");
        assert_eq!(track.track_number, Some(5));
        assert_eq!(track.disc_number, Some(1));
        assert_eq!(track.album.unwrap().year, Some(2013));
    }

    #[test]
    fn blanks_and_absent_fields_map_to_nothing() {
        let source = deezer::Track {
            id: 7,
            title: String::new(),
            duration: None,
            track_position: None,
            disk_number: None,
            isrc: Some("   ".to_string()),
            explicit_lyrics: None,
            artist: None,
            album: None,
        };
        let track = track(&source, None);

        assert_eq!(track.id, "7");
        assert_eq!(track.duration_ms, None);
        assert_eq!(track.isrc, None);
        assert_eq!(track.explicit, None);
        assert!(track.ids.is_empty());
        assert!(track.artists.is_empty());
        assert!(track.album.is_none());
    }

    #[test]
    fn years_come_from_the_first_four_digits() {
        assert_eq!(year(Some("2013-05-17")), Some(2013));
        assert_eq!(year(Some("1999")), Some(1999));
        assert_eq!(year(Some("20")), None);
        assert_eq!(year(Some("unknown")), None);
        assert_eq!(year(None), None);
    }
}
