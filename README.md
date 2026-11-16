# local-music-app

A lightweight Windows desktop app for managing a local music library, built with Tauri 2 (Rust core, system WebView2) and Svelte 5.

This is the gray-box prototype: real rail mechanics, proportions, background shader, and performance overlay, driven by 500 generated placeholder artists. Library scanning, tag editing, file organization, artwork fetching, playback, and visual design come in later milestones. See [the design spec](docs/superpowers/specs/2026-11-16-music-library-design.md).

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

Set `MUSIC_PLACEHOLDER_ARTISTS` (up to 20000) before launching to stress-test the rail with more artists. Set `MUSIC_SOAK=<seconds>` to run the soak test on launch for that long, which lets you leak-test a release build unattended (`scripts/measure.ps1 -SoakSeconds 120` does this and logs memory).

## Controls

| Input | Action |
| --- | --- |
| Mouse wheel | One card per notch |
| Trackpad / drag | Free scroll with momentum, snaps to a card |
| Click a card | Focus it |
| Left / Right, Page Up / Down, Home / End | Step 1, step 5, jump to ends |
| Start typing | Filter artists (Esc clears) |
| F3 | Toggle performance overlay |
| F4 (dev builds) | Toggle soak test (random scrolling and filtering, for leak checks) |

## Architecture

- `src-tauri/src/placeholder.rs` generates deterministic placeholder artists.
- `src-tauri/src/perf.rs` measures memory and CPU across the app's whole process tree, including WebView2 child processes.
- `src/lib/ticker.ts` is the single animation loop; it stops entirely when nothing is moving.
- `src/lib/rail/` holds the scroll physics (`physics.ts`), geometry (`layout.ts`), recycled card DOM (`pool.ts`), and input handling (`controller.ts`).
- `src/lib/background/` is the WebGL2 shader, rendered at half resolution and paused when the window is unfocused.
