# local-music-app

A lightweight Windows desktop app for managing a local music library, built with Tauri 2 (Rust core, system WebView2) and Svelte 5.

Current milestone: a real, managed library. Imports copy music into the library folder (originals are never touched), repair common tag problems on the copies, organize files as `Artist/Year - Album/NN Title.ext`, and index everything in SQLite. Artists are matched on MusicBrainz in the background, with headshots from Wikidata/Wikimedia Commons. Tag editing, playback, and visual design come in later milestones. See [the design spec](docs/superpowers/specs/2026-11-16-music-library-design.md).

## Requirements

- Node.js 20+
- Rust (stable, MSVC toolchain) and the [Tauri prerequisites for Windows](https://tauri.app/start/prerequisites/)
- WebView2 runtime (preinstalled on Windows 11)

## Commands

```powershell
npm install
npm run tauri dev      # run the desktop app with hot reload
npm run tauri build    # optimized installer in src-tauri/target/release/bundle
npm test               # frontend unit tests
npm run check          # type check
cd src-tauri; cargo test   # Rust unit tests
```

## Library

- **Import** with the Import button, by dropping folders or files on the window, or by launching with `--import <path>...`. Re-importing the same files is a no-op (sources are recognized by content hash).
- **Library folder** defaults to the Windows Music folder plus `Local Music`. Change it in `%APPDATA%\com.local.musicapp\settings.json` (`libraryRoot`).
- **App data** lives in `%LOCALAPPDATA%\com.local.musicapp`: `library.db` (rebuildable index; file tags are the source of truth) and `cache\` (resized covers and headshots).
- **Tag repairs on import** (copies only): missing album artist (album majority, else the lead credited artist), missing title (from the file name), disc numbers encoded in file names (`301. Title` is disc 3, track 1), and albums where every track shares one number. MP3 tags are read and written with the `id3` crate, which keeps frames it can't decode; a tag that only partly parses is never rewritten.
- **MusicBrainz** lookups are anonymous, rate limited to one request per second, and retried when the service is busy. A match counts as verified when one of your track titles is credited to that artist. Launch with `--refresh-metadata` to redo all lookups.

To preview an import without copying anything: `$env:MUSIC_PREVIEW_DIR="<folder>"; cargo test preview -- --ignored --nocapture` (in `src-tauri`). `roundtrip_real_folder` does the same but also writes the planned tag edits to temp copies and checks nothing else was lost.

Set `MUSIC_PLACEHOLDER_ARTISTS` (up to 20000) before launching to stress-test the rail with generated artists instead of the library. Set `MUSIC_SOAK=<seconds>` to run the soak test on launch for that long, which lets you leak-test a release build unattended (`scripts/measure.ps1 -SoakSeconds 120` does this and logs memory).

## Controls

| Input | Action |
| --- | --- |
| Mouse wheel | One card per notch |
| Trackpad / drag | Free scroll with momentum, snaps to a card |
| Click a card | Focus it; click the focused card to open it |
| Enter | Open the focused artist (Esc closes) |
| Left / Right, Page Up / Down, Home / End | Step 1, step 5, jump to ends |
| Start typing | Filter artists (Esc clears) |
| F3 | Toggle performance overlay |
| F4 (dev builds) | Toggle soak test (random scrolling and filtering, for leak checks) |

## Architecture

- `src-tauri/src/library.rs` plans imports (tag repairs), copies with hash verification, and incrementally scans the library folder.
- `src-tauri/src/db.rs` is the SQLite index; `tags.rs` reads and writes tags; `template.rs` renders Windows-safe paths; `artwork.rs` resizes covers and extracts palettes.
- `src-tauri/src/worker.rs` runs library jobs on one thread and network enrichment (`enrich.rs`, `musicbrainz.rs`) on another, emitting `library-changed` and `library-progress` events.
- `src-tauri/src/perf.rs` measures memory and CPU across the app's whole process tree, including WebView2 child processes.
- `src/lib/ticker.ts` is the single animation loop; it stops entirely when nothing is moving.
- `src/lib/rail/` holds the scroll physics (`physics.ts`), geometry (`layout.ts`), recycled card DOM (`pool.ts`), and input handling (`controller.ts`).
- `src/lib/background/` is the WebGL2 shader, rendered at half resolution and paused when the window is unfocused.
- `src/lib/detail/ArtistDetail.svelte` shows an artist's releases and tracks.
