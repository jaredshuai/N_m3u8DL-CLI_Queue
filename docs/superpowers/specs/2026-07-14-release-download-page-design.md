# Release Download Page Design

## Status

Current executable design as of 2026-07-15.

Release behavior is defined by `.github/workflows/release.yml` and guarded by
`m3u8-queue-downloader/scripts/release-workflow.test.mjs`. This document lists
the required invariants and operator recovery rules without duplicating the
workflow's PowerShell implementation.

## Scope

- Publish the desktop application from `app-v*` tags only.
- Keep `Package GUI` unchanged for daily/test packages and local artifact sync.
- Publish one Windows x64 installer and one Windows x64 portable archive.
- Expose the Latest Release page from the root README without hard-coding a
  future version URL.
- Do not add an updater, GitHub Pages site, code signing, or an untagged release
  path.

## External Tag Protection

Before pushing any release tag, the repository must have one active tag ruleset
with all of these properties:

- target: `tag`
- ref include: `refs/tags/app-v*`
- rules: `update` and `deletion`
- bypass actors: none

The workflow triggers on any push of a matching tag. The external ruleset makes
that tag a create-once reference: after creation it cannot be moved, updated, or
deleted. The workflow verifies tag identity, but the ruleset is the control that
prevents a race between verification points.

## Workflow Boundaries

### Build job

The `windows` job has only `contents: read` permission. It checks out source
without persisted credentials, resolves release metadata, runs frontend and
Rust tests, builds the bundled resources and Tauri application, prepares the two
release assets, verifies them locally, and uploads one short-lived Actions
artifact for the publish job.

The job records `source_sha` from the checked-out commit. GitHub values enter
PowerShell through environment variables. The package version is read by Node,
encoded as Base64, decoded as strict UTF-8, and then validated as an exact
SemVer accepted by this project: stable `X.Y.Z` or an `rc`, `beta`, or `alpha`
prerelease. Whitespace, extra lines, non-ASCII digits, and tag/version mismatch
fail before release outputs are emitted.

### Publish job

The `publish` job depends on the build job and has only `contents: write` plus
`actions: read`. It does not check out source or run a build. Its publication
steps are serialized repository-wide by the `release-publication` concurrency
group with in-progress publication runs left intact.

Its required order is:

1. Download the build artifact.
2. Verify the downloaded local assets.
3. Determine prerelease and Latest policy.
4. Resolve and peel the remote release tag to a commit; compare it with
   `source_sha`.
5. Refuse to continue if any same-tag Release already exists.
6. Create one draft Release with both assets.
7. Verify the draft Release and its assets.
8. Resolve and peel the remote tag again; compare it with `source_sha`.
9. Publish the verified draft.
10. Verify the published Release, asset sizes, prerelease state, and Latest
    result.

`GH_TOKEN` is scoped only to steps that invoke `gh`. Non-`gh` steps receive no
repository token.

## Tag Source Integrity

The publish job reads the remote tag through the GitHub API and recursively
peels annotated tags until it reaches a commit. Cycles, excessive tag depth,
unexpected object types, malformed SHAs, API failures, and a commit different
from `source_sha` all fail closed.

The check runs twice: once immediately before draft creation and once after
draft asset verification immediately before publication.

## Release Assets

From version `0.2.2` onward, the exact public names are:

- `m3u8-queue-downloader_<version>_x64-setup.exe`
- `m3u8-queue-downloader_<version>_portable_x64.zip`

The portable archive includes the application executable, bundled
`N_m3u8DL-CLI_v3.0.2.exe`, and ffmpeg in the runtime locations expected by the
application.

Asset validation has three public-safety stages:

1. **Local:** both the build output and the publish job's downloaded copy must
   contain exactly the two expected names, each with a positive byte size.
2. **Draft:** the private draft must contain exactly those two names, and each
   remote byte size must equal the corresponding downloaded local file.
3. **Published:** the public Release must still contain exactly those two names
   with the same byte sizes.

No draft is made public until the local and draft checks pass.

## Existing Same-Tag Releases

Automation never deletes, replaces, uploads over, or otherwise modifies an
existing same-tag Release, regardless of whether it is draft or published.

- **Existing draft:** an operator must inspect it first. If it is confirmed to
  be an unpublished failed-run artifact, the operator may delete only the draft
  Release while retaining the tag exactly where it is. Do not request any tag
  cleanup and do not move or recreate the tag. Rerun the same tag workflow from
  GitHub Actions afterward.
- **Existing published Release:** never delete or reuse it. Prepare a higher
  application version, create a new tag, and publish a new Release.

## Prerelease And Latest Policy

- Versions containing the accepted `rc`, `beta`, or `alpha` prerelease channel
  are always prereleases and never Latest.
- If no Latest Release exists, the first stable release becomes Latest.
- If a stable version is strictly greater than the current stable Latest
  SemVer, it becomes Latest.
- If a stable version is equal to or lower than the current Latest, including an
  older stable backfill, it is published with `latest=false` and the existing
  Latest remains unchanged.

The global publish concurrency lock ensures the Latest comparison and
publication decision are not made concurrently by two release runs.

## Failure And Recovery

- Invalid metadata, tag/version mismatch, test failure, build failure, or local
  asset failure stops before a Release is created.
- Authentication, rate-limit, malformed API response, and unexpected HTTP
  status failures stop rather than being treated as a missing resource.
- A failure after draft creation may leave a private draft. Recover only through
  the operator-only draft procedure above, then rerun the same workflow.
- A published Release is immutable to this automation. Any correction after
  publication requires a higher version and a new tag.
- Never recover by deleting, moving, or force-updating an `app-v*` tag.

## README Download Entry

The README download section links to:

`https://github.com/jaredshuai/N_m3u8DL-CLI_Queue/releases/latest`

It presents the versioned installer and portable patterns as the primary path
from `0.2.2` onward, states that both packages include N_m3u8DL-CLI and ffmpeg
for Windows x64, and includes one subordinate compatibility note: while Latest
is still `0.2.1`, the portable asset is named `portable.zip` and the installer
must be selected from the actual assets shown on that Release page.

## Verification Contract

The Node contract suite contains nine tests covering the README section,
trigger and permission boundaries, metadata transport, token scope, immutable
release lifecycle, native command exit guards, asset transfer and byte-size
checks, Latest policy, and the two remote tag peel checks. Any workflow change
must update implementation and contract together.
