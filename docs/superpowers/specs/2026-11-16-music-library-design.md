# Music Library Desktop App — Design Spec

Date: 2026-11-16
Status: Approved (functional design). Brand identity and final visuals are out of scope for this spec.

## Goal

A standalone, lightweight Windows desktop app that manages a local music library and makes metadata edits fast. The home screen is a horizontally scrolling rail of artist cards, each showing the artist's name, headshot or brand mark, and a live animated background.

The first deliverable is a gray-box prototype that lets the user judge scroll mechanics, proportions, and performance before any branding work.

## Scope

In scope:
- Finished audio files: MP3, FLAC, M4A/AAC, WAV, OGG Vorbis, Opus.
- Editing tags, embedded artwork, file names, and folder organization.
- A fully managed library folder that the app keeps organized automatically.
- Automatic artist imagery from online sources, with manual override.
- Preview playback while editing.

Out of scope:
- Audio editing (trim, fade, normalize, format conversion).
- DAW project files, stems, samples.
- Multi-artist management (contracts, press kits).
- Streaming services, cloud sync, mobile.
- Brand identity, logo, final color system, final typography (separate phase).

## Technology

- Shell: Tauri 2 (Rust core, system WebView2 for UI).
- UI: Svelte 5 + TypeScript. One WebGL2 canvas for the background shader.
- Database: SQLite via `rusqlite`.
- Tags: `lofty`.
- File watching: `notify`.
- Images: `image` crate for resizing and palette extraction.
- Playback: `rodio` (with `symphonia` decoders), running in the Rust core.

Electron was rejected for size and memory. Flutter desktop was rejected for weaker audio-metadata and filesystem tooling.

## Architecture

The UI never reads audio files directly. It requests summaries and cached thumbnails from the Rust core through Tauri commands, and receives change notifications through Tauri events.

### Rust core modules

| Module | Responsibility |
|---|---|
| `library` | SQLite index of artists, releases, tracks. Rebuildable from file tags at any time. |
| `tags` | Read/write tags and embedded artwork. |
| `organizer` | Path templating, move journal, undo/redo. |
| `watcher` | Detect external changes in the library folder and `_Inbox`. |
| `artwork` | Fetch from MusicBrainz and Fanart.tv, resize to cached sizes, extract palettes. |
| `player` | Decode and play audio. |

### UI components

| Component | Responsibility |
|---|---|
| `Rail` | Virtualized horizontal artist carousel with spring physics and snapping. |
| `ShaderBackground` | Full-window WebGL2 shader driven by the focused artist's palette and photo. |
| `ArtistDetail` | Releases and tracks for one artist; opened by an expanding transition from its card. |
| `TagEditor` | Single-track and batch tag editing with a change preview. |
| `CommandPalette` | Ctrl+K search across artists, releases, tracks, and actions. |
| `PlayerBar` | Now playing, play/pause, seek. |
| `PerfHUD` | Development overlay: FPS, frame time, JS heap, process memory. |

## Data model

- Artist: id, name, sort name, MusicBrainz id, image paths (headshot, logo, background), palette (5 colors), image source (fetched or manual).
- Release: id, artist id, title, year, type (album, EP, single, compilation), artwork path.
- Track: id, release id, file path, title, track number, disc number, duration, format, bitrate, file size, file hash, modified time.
- Journal entry: id, operation group id, operation (move, write tags, write artwork, delete), source path, destination path, state (pending, done, rolled back), timestamp.

File tags are the source of truth. The database is a cache.

## Managed library

- On first run the user picks a library root (for example `E:\Music Library`). The app owns everything under it.
- Path template, user-editable, default:
  `{AlbumArtist}/{Year} - {Album}/{Disc}{Track:02} {Title}.{ext}`
  - `{Disc}` renders only for multi-disc releases, as `1-`.
  - Compilations file under `Various Artists`.
  - Characters invalid on Windows (`<>:"/\|?*`, trailing dots and spaces) are replaced with `_` or stripped. Reserved names (`CON`, `NUL`, etc.) get a trailing `_`.
  - Full paths longer than 240 characters truncate the title segment first.
- Import: drag files or folders onto the window, or drop them into `<root>/_Inbox`. Imports copy by default and keep the originals. Move-on-import is a setting.
- Editing tags that affect the path renames or moves the file to match, after the edit is saved.
- Every file operation is written to the journal as pending before it runs and marked done after. On startup, pending entries are completed or rolled back.
- Undo reverses a whole operation group (tag write plus resulting move).
- Same-volume moves use an atomic rename. Cross-volume moves copy, verify by hash, then delete the source.
- Duplicate detection: same normalized artist, album, title, and duration within 2 seconds, or identical file hash. The user chooses to keep both, replace, or skip.
- External changes under the root are detected by the watcher and reconciled into the index. Files placed outside the template structure are refiled.
- Artist images are stored in the artist folder using Kodi/Plex names: `folder.jpg` (headshot), `logo.png` (brand mark), `fanart.jpg` (background).

## Artist imagery

- Artist identity resolved through MusicBrainz (rate-limited to 1 request per second, with a descriptive User-Agent).
- Images fetched from Fanart.tv: artist thumb (headshot), HD clearlogo (brand mark), artist background. Requires a free personal API key entered once in settings.
- Fetching runs in the background; cards update as images arrive.
- Images the user drags onto a card replace fetched ones and are never overwritten by later fetches.
- Artists with no online match use their release artwork's palette for the shader and show a monogram in place of a headshot.
- Each image is resized once into cached sizes (card size and small) as WebP. The rail never loads full-resolution images.
- A 5-color palette is extracted from each artist's background (or release art fallback) at fetch time and stored in the database.

## Rail mechanics

- Input: vertical mouse wheel maps to horizontal scroll; trackpad horizontal gestures scroll natively; Left/Right arrows and A/D step one card.
- Physics: momentum on release, then a spring settles on the nearest card.
- The card nearest the center is focused: scale 1.0, full opacity. Neighbors: scale 0.88, opacity 0.6.
- Only the focused card shows its live background; others show a still frame.
- Virtualization: render only visible cards plus 2 on each side.
- Enter or click opens `ArtistDetail` with a shared-element transition from the card.
- Typing anywhere on the rail filters it; Ctrl+K opens the command palette.

## Proportions (prototype defaults, to be tuned)

- Minimum window size: 1280×720.
- Top bar: 56px. Player bar: 72px. The rail fills the remaining height.
- Card: 3:4 portrait, height 62% of the window height (about 500×670 at 1080p). Gap: 24px.
- Card content: artist name large, bottom-left; 96px headshot or logo badge above the name; background fills the card.

## Background shader

- One full-window WebGL2 canvas behind the rail.
- Renders a flowing gradient from the focused artist's palette, blended with their background photo.
- Switching focus crossfades palette uniforms and the photo texture over about 600ms.
- Renders at half resolution and upscales.
- Frame rate capped to display refresh. Rendering stops when the window is minimized, hidden, or unfocused.
- Target cost: 1–3% GPU on a modern discrete or integrated GPU.

## Performance budgets

| Metric | Target |
|---|---|
| Cold start to interactive | < 1 s |
| Rail scrolling | Sustained 60 fps; 120 fps on high-refresh displays |
| Idle memory | < 150 MB |
| Idle CPU | ~0% (all animation paused when unfocused) |
| First scan, 10,000 tracks | < 30 s |
| Rescan after changes | Incremental, by modified time |

Rules that make these budgets reachable:
- Animate only `transform` and `opacity` in the DOM.
- Never send full-resolution images to the UI.
- One live shader at a time.
- Database queries for the rail return only the fields a card needs.

## Error handling

- Unreadable or corrupt files are indexed with an error flag and listed in an Issues view; they are never moved automatically.
- A tag write that fails leaves the file untouched and marks the journal entry rolled back.
- Network failures during artwork fetch retry with backoff and never block the UI.
- If the library root is unavailable (drive unplugged), the app opens read-only from the index and shows a banner.

## Testing

- Rust unit tests: path template rendering, invalid characters, reserved names, long paths, multi-disc and compilation cases.
- Rust unit tests: journal recovery after a simulated crash at each step of a move.
- Fixture generator: creates a synthetic library of 10,000 tagged tracks for scan benchmarks.
- `PerfHUD` in development builds for manual scroll and memory checks.

## First milestone: gray-box prototype

- Tauri 2 + Svelte 5 app shell with the final layout regions (top bar, rail, player bar).
- `Rail` with real physics, snapping, focus scaling, and virtualization.
- 500 placeholder artists with generated names and generated gradient palettes.
- `ShaderBackground` driven by the placeholder palettes.
- `PerfHUD` visible.
- No library, tags, or networking yet.

Later milestones, in order: library scan and index; tag editor; managed organizer and journal; artwork fetching; playback; command palette; brand identity and visual design.
