# Changelog

All notable changes to PortableHub are documented here. Format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/); versions follow SemVer.

## [1.14.1] - 2026-10-31

### Added
- Settings explains that the program folder is the folder containing
  `PortableHub.exe` and moves with the app; the apps folder defaults to the
  relative `Apps` path.
- Test asserting the default apps folder is relative and `discover_roots` is
  empty.

### Fixed
- Release binaries remap local cargo/rustup/workspace paths and fail the
  release-path audit if a machine-specific build path remains.
- Source comments and release artifacts no longer expose local install paths
  or usernames.

## [1.14.0] - 2026-10-31

### Added
- Command Deck main layout: persistent library rail, application grid, and a
  selection inspector with launch, reveal, tags, category, autostart, and edit
  actions; multi-selection exposes batch tag/group/category actions.
- Three built-in themes: **Pixel Dark** (default), **Warm Paper**, and **Cold
  Slate**. Switch them in Settings → Appearance.
- Card footer readiness/missing state, plus compact horizontal card metadata.
- `get_icons` command and lazy per-page icon loading.

### Changed
- Main toolbar and settings now use the warm-black / amber design system with
  square 1px edges, pixel display type, and a readable system body font.
- The library rail is always visible; selecting a group enables that grouping
  mode instead of requiring the toolbar toggle first.
- Application list and icon loading run off the UI thread; scanning is
  asynchronous and shows a disabled “Scanning…” state.

### Fixed
- Clicking **Scan** no longer freezes the window on large application folders.
- Application startup no longer extracts icons for the whole library at once.
- Release builds now remap local cargo/workspace paths and fail verification if a
  machine-specific build path remains in the binary.

## [1.13.0] - 2026-10-31

### Added
- Lazy icon loading for mounted cards.
- Async `list_apps` and scan commands.

### Fixed
- UI freeze while scanning large folders.

## [1.12.0] - 2026-10-31

### Added
- Built-in **Autostart** virtual tag in the tag filter; cycles include →
  exclude → off and combines with user tags, search, and groups.

## [1.11.0] - 2026-10-31

### Changed
- Autostart entries are checked and launched whenever the hub starts, not only
  during Windows login. Running processes are detected and skipped.

[1.14.0]: https://github.com/yrui-ql/portablehub/compare/v1.13.0...v1.14.0
[1.13.0]: https://github.com/yrui-ql/portablehub/compare/v1.12.0...v1.13.0
[1.12.0]: https://github.com/yrui-ql/portablehub/compare/v1.11.0...v1.12.0
[1.11.0]: https://github.com/yrui-ql/portablehub/releases/tag/v1.11.0
