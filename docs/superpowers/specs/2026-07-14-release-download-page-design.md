# Release Download Page Design

## Status

Approved direction: follow the tag-driven GitHub Release pattern used by
`jaredshuai/gemini-oauth-switcher`, adapted to this Tauri project.

## Context

The repository already has a `Release` workflow and published releases, but
the workflow lets `tauri-action` create the Release before the portable archive
is prepared. The installer is uploaded first and a generic `portable.zip` is
added later. The root README also has no user-facing download entry.

The next release must include the task-termination fixes currently on `master`,
so the package version will advance from `0.2.1` to `0.2.2` rather than moving
or replacing the existing `app-v0.2.1` tag.

## Goals

- Create a standard GitHub Release from an `app-vX.Y.Z` tag.
- Publish one Windows x64 installer and one Windows x64 portable archive.
- Give both assets explicit versioned names.
- Generate release notes from commits since the previous release.
- Fail before Release creation when the tag, version, or assets are invalid.
- Add a prominent README download section that sends users to Latest Release.

## Non-Goals

- No standalone GitHub Pages site.
- No in-app updater.
- No change to the `Package GUI` workflow used for daily test packages.
- No code-signing work in this change.
- No rewrite of existing release tags or assets.

## Release Flow

1. `npm run release:prepare -- 0.2.2` updates all four version files.
2. The version bump and workflow changes are committed to `master`.
3. Tag `app-v0.2.2` is created and pushed.
4. The `Release` workflow validates that the tag matches the package version.
5. Tests run and Tauri builds the NSIS installer without creating a Release.
6. The workflow prepares and compresses the portable directory.
7. A verification step requires exactly one non-empty installer and one
   non-empty portable archive.
8. `softprops/action-gh-release@v2` creates the GitHub Release, generates notes,
   and uploads both verified assets in one publishing step.

## Assets

For version `0.2.2`, the Release page exposes:

- `m3u8-queue-downloader_0.2.2_x64-setup.exe`
- `m3u8-queue-downloader_0.2.2_portable_x64.zip`

The portable archive contains the application executable, bundled
`N_m3u8DL-CLI_v3.0.2.exe`, and both required ffmpeg locations already used by
the packaging workflow.

## README Entry

Add a `下载` section near the top of `README.md` with a link to:

`https://github.com/jaredshuai/N_m3u8DL-CLI_Queue/releases/latest`

The section explains which asset is the installer and which is the portable
archive. It links to the Release page rather than embedding a version-specific
asset URL, so future releases do not require README edits.

## Failure Handling

- Tag/version mismatch stops the workflow before building or publishing.
- Missing, duplicate, or zero-byte assets stop the workflow before Release
  creation.
- Release creation happens after both artifacts are ready, preventing a page
  that temporarily contains only one download type.
- A failed workflow leaves the previous Latest Release unchanged.

## Verification

- Run `npm test` and `cargo test --locked` locally before tagging.
- Run `npm run build` locally for the frontend production build.
- Confirm the Release workflow succeeds for `app-v0.2.2`.
- Use `gh release view app-v0.2.2` to verify the release is published and has
  exactly the installer and portable assets.
- Download or synchronize both assets and verify they are non-empty.

