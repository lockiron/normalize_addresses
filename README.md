Rust port (offline & heuristic) of [geolonia/normalize-japanese-addresses](https://github.com/geolonia/normalize-japanese-addresses).

## What this does
- Normalizes text to NFKC, trims, collapses whitespace.
- Splits into `prefecture`, `city/ward/郡`, `town` (町/村/丁目), and the remaining `rest`.
- Provides a CLI that prints JSON.

## What this does *not* do yet
- No external datasets or geocoding; results are heuristic and may be ambiguous.
- Does not resolve kana/romaji variants, old municipality names, or block-level geocoding.
- No API client; everything is offline.

## Usage
```sh
cargo run -- \"東京都渋谷区神南１丁目１９−１１\"
```

Output:
```json
{
  \"prefecture\": \"東京都\",
  \"city\": \"渋谷区\",
  \"town\": \"神南1丁目\",
  \"rest\": \"19-11\",
  \"original\": \" 東京都渋谷区神南１丁目１９−１１ \"
}
```

## Tests
`cargo test` (requires network once to fetch crates).

## Extending toward feature parity
- Replace the regex-based splitters with a dictionary-driven parser (e.g., CSV of municipalities).
- Add kana/romaji normalization.
- Wire to Geolonia API or local tile data for lat/lng enrichment.
- Implement address range normalization (番地/号) and building names.
