# Hearthsound Deezer catalog plugin

A [Hearthsound](https://github.com/hearthsound) catalog plugin backed by the
[Deezer public API](https://developers.deezer.com/api). It compiles to a
WebAssembly component that implements the `catalog` and `artwork` provider
interfaces, so a Hearthsound server can search Deezer, browse Deezer tracks,
albums and artists, resolve ISRCs to tracks, and use Deezer covers and artist
portraits for its artwork layer.

The plugin needs no Deezer account and no API key: every endpoint it calls is
part of the public, unauthenticated API.

## Requirements

- Rust 1.99 or newer, with the `wasm32-wasip2` target. The included
  `rust-toolchain.toml` pins both; `rustup toolchain install` fetches them.

## Build

```console
cargo build --release --locked --target wasm32-wasip2
```

The component lands at
`target/wasm32-wasip2/release/hearthsound_plugin_deezer.wasm`.

## Check and pack

Install the plugin CLI from the SDK repository:

```console
cargo install --locked --git https://github.com/hearthsound/plugin-sdk --tag v0.3.0 hearthsound-plugin-cli
```

Then validate the manifest and the component:

```console
hsplugin check
```

It prints `ok: app.hearthsound.deezer 0.3.0 (provides catalog, artwork)` on
success.

Finally, build the archive you install on a server:

```console
hsplugin pack --out-dir dist
```

`dist/app.hearthsound.deezer-0.3.0.hsplugin` contains the manifest and the
component.

## Tests

The mapping and URL-building logic runs on the host target, so its unit tests
need no WebAssembly toolchain and no network:

```console
cargo test
```

## Configuration

| Instance setting | Default | Meaning |
|---|---|---|
| `baseUrl` | `https://api.deezer.com` | Base URL of the Deezer API. Point it at a mirror or a test double to keep traffic off Deezer. |

The instance requests a `host-http` rate limit of 8 requests per second with a
burst of 8, which keeps any 5-second window below the 50 requests Deezer
publishes (the host enforces the limit per instance, so run one instance against
the public API: two instances share the server's per-IP quota without adding
capacity). The same limiter paces the image requests the server makes on the
instance's behalf, because they use the instance's HTTP permission.

## Notes

- Deezer's terms forbid caching its images, so the manifest declares
  `catalog.imageCachePolicy: "no-store"` and the server forwards proxied
  artwork without storing it.
- Images come from `cdn-images.dzcdn.net`; the manifest allowlists
  `*.dzcdn.net` for them.
- Deezer issues its own ids, so the plugin reports them as the entity ids and
  lets the server namespace them per provider instance. The canonical
  identifiers Deezer does know (a track's ISRC, an album's UPC) are reported as
  `isrc` and `upc` id pairs.
- An album's track list is read from `/album/{id}/tracks`, not from the album
  response: the embedded list carries no ISRCs, track numbers or disc numbers
  and stops at 25 entries, while the dedicated listing carries all three and
  pages 100 at a time.
- An artist's album listing carries no album artist, so the album references it
  returns have no artist credits; every other response reports the artist.
- ISRCs are normalized before a lookup (hyphens and spaces removed, upper-cased)
  because Deezer answers a hyphenated code with "no data".
- The artwork interface answers a subject from its own id when the server holds
  one, otherwise from the album's UPC, otherwise from Deezer's advanced search;
  a search result counts only when its title, and the artist it credits, are the
  ones the subject names. An artist without a portrait is answered with nothing,
  because Deezer's placeholder URLs serve a flat black square rather than a
  picture.
- Deezer answers most errors with HTTP status 200 and an `error` object, and its
  gateway answers a quota violation with the same body on a forbidden status.
  The plugin reads the body before the status, so "no data" becomes a not-found
  and a quota error becomes rate-limited with a 5-second delay (one quota window)
  whichever status carried it; a bare 403 is also rate-limited, because the only
  reason the API refuses a request without naming a different failure is that
  the caller is over the quota. Everything else becomes an unavailable provider.

## Layout

| Path | Contents |
|---|---|
| `Cargo.toml` | Crate metadata and the SDK dependency. |
| `src/lib.rs` | The plugin: configuration, lifecycle and the catalog and artwork interfaces. |
| `src/api.rs` | The HTTP transport and Deezer's error envelope. |
| `src/artwork.rs` | The artwork interface: album covers and artist portraits. |
| `src/catalog.rs` | One function per catalog operation, over the API endpoints. |
| `src/deezer.rs` | Serde types for Deezer API responses. |
| `src/map.rs` | Deezer records to catalog records. |
| `src/url.rs` | Base URL validation and query encoding. |
| `manifest.json` | The plugin manifest the server reads. |

## License

Licensed under either of Apache License, Version 2.0 or MIT license at your
option.
