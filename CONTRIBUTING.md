# Contributing

Please include your OS, reproduction steps, expected behavior, and actual behavior in bug reports. Use synthetic text when reporting OCR or translation issues. Do not attach API keys, local config files, or unredacted diagnostic logs.

For changes, keep the scope focused and describe the user-visible effect. Run pnpm build for frontend changes and relevant Rust tests for backend changes. OCR and segmentation tests can run without Tauri through scripts/core_tests.rs; full backend tests require the platform's Tauri prerequisites.

Contributions are accepted under the project's MIT license. Preserve upstream attribution and include licenses for bundled third-party data.
