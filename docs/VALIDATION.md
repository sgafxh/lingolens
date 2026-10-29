# Validation — 2026-10-22

- pnpm build: TypeScript and Vite production build passed.
- Standalone OCR/segmentation: 15 passed, 0 failed, 1 ignored manual inspection test.
- Full Windows Rust unit suite: 27 passed, 0 failed, 1 ignored. Two existing compiler warnings remain (unused autostart import and mutable builder).
- Common credential-pattern scan: no matches in 113 candidate files and 5 existing commits. This is a heuristic check.

The full suite includes the standalone tests; do not add these counts. No paid API requests were made. Packaged installation, live OCR interaction, macOS builds and end-to-end translation remain to be verified before a binary release.

## Translation settings update

Frontend build passed. Windows Rust suite: 30 passed, 0 failed, 1 ignored. New tests cover key preservation/replacement, input validation and preservation of unrelated settings. Native UI interaction and live provider requests have not been exercised in this update.

## Windows 1.0.0 binary release

- `pnpm tauri build --bundles nsis`: production frontend and optimized Rust build passed.
- `cargo test --offline --locked --manifest-path src-tauri/Cargo.toml --lib`: 30 passed, 0 failed, 1 ignored.
- NSIS installer completed with exit code 0 and registered LingoLens in the current user's installed apps.
- Release executable launched and rendered the translation-service settings, shortcuts and version 1.0.0.
- Compared local credentials against the release executable and frontend assets: zero matches.

Live OCR-to-provider translation and macOS builds remain untested in this release.
