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

## Windows 1.0.2 shortcut icon update

- Production frontend, Rust release build and NSIS packaging passed.
- Standalone NSIS hook check created a shortcut with the expected absolute ICO path and no expandable icon block.
- Local silent upgrade from 1.0.1 completed with exit code 0; installed executable reports 1.0.2.
- Translation configuration file hash was unchanged after upgrade.
- Desktop and Start menu shortcuts both target the installed executable and its bundled standalone ICO.
- Windows system image list extraction succeeded for the installed desktop shortcut at large, extra-large and jumbo sizes.
- Compared local credentials against the release executable and frontend assets: zero matches.
- Interactive finish-page callback compiles; a full interactive installer walkthrough was not performed.
