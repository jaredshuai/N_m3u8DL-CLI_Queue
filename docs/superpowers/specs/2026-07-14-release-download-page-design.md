# Release Download Page Design

## Status

Current executable design as of 2026-07-15.

Release and `release:prepare` behavior is defined by `.github/workflows/release.yml`
and guarded by `m3u8-queue-downloader/scripts/release-workflow.test.mjs` plus
`m3u8-queue-downloader/scripts/prepare-release.test.mjs`. These three files are
the behavior source of truth. This document lists required invariants and
operator recovery rules without duplicating the workflow's PowerShell blocks.

## Deployed State

- Public Latest Release: `app-v0.2.3`
- Release URL: `https://github.com/jaredshuai/N_m3u8DL-CLI_Queue/releases/tag/app-v0.2.3`
- Successful Release workflow run: `29404677585`
- Release source: `381cb1410740f2edae71d8872a209db1e7e159b2`
- Active tag ruleset: `18966944`
- Repository Immutable Releases: enabled; `app-v0.2.3` verified immutable

The hardened workflow published `app-v0.2.3` with `isImmutable=true`, exact
asset sizes, and matching remote/local SHA256 digests. Because repository
immutability was enabled after `app-v0.2.2` was published, that historical
Release remains mutable. It is not deleted, replaced, or retroactively repaired.

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

Ruleset `18966944` currently satisfies this contract. Before every new tag, the
operator must run the mechanical pre-tag gate described below; a prose reminder
or previously observed repository state is not sufficient.

The workflow triggers on any push of a matching tag. The external ruleset makes
that tag a create-once reference: after creation it cannot be moved, updated, or
deleted. Before draft creation and again before publication, the workflow
verifies tag identity, remote `master` ancestry, the complete ruleset shape, and
repository Immutable Releases. The ruleset remains the control that prevents a
race between verification points.

## Mechanical Pre-Tag Gate

After the version commit is pushed to `master`, and before creating a tag, run
from `m3u8-queue-downloader/`:

```powershell
node scripts/prepare-release.mjs pre-tag <version>
```

The command is read-only apart from `git fetch origin master`: it does not
update version files or create a tag. It fails nonzero unless all checks pass in
this order:

1. Current branch is exactly `master`.
2. `git status --porcelain` is empty.
3. `git fetch origin master` succeeds.
4. `HEAD` exactly equals `origin/master`.
5. The five version files agree and equal the requested strict release version.
6. `app-v<version>` exists neither locally nor under remote
   `refs/tags/app-v<version>`.
7. The operator's local `gh` credentials find exactly one repository tag
   ruleset named `Protect app-v release tags`; its full configuration is active,
   targets tags, includes exactly `refs/tags/app-v*`, has no excludes, has
   exactly update and deletion rules, has no bypass actors, and reports
   `current_user_can_bypass=never`.
8. `GET repos/jaredshuai/N_m3u8DL-CLI_Queue/immutable-releases` with GitHub API
   version `2026-03-10` reports `enabled=true`.

Only a successful gate permits tag creation. Its success output prints a
`git tag app-v<version> <verified-head-sha>` command, so tag creation is bound to
the commit that passed `HEAD == origin/master`, followed by an explicit
`refs/tags/app-v<version>` push. The Release workflow still performs independent
remote checks because pre-tag validation cannot replace publication-time
verification.

## Workflow Boundaries

### Build job

The `windows` job has only `contents: read` permission. It checks out source
without persisted credentials, resolves release metadata, runs frontend and
Rust tests, builds the bundled resources and Tauri application, prepares the two
release assets, verifies them locally, and uploads one short-lived Actions
artifact for the publish job.

Every `uses:` entry is pinned to an approved 40-character commit SHA with a
human-readable version comment. The approved actions are checkout `v4.3.1`,
setup-node `v4.4.0`, cache `v4.3.0`, upload-artifact `v4.6.2`, rust-toolchain
`stable`, and tauri-action `v0.6.2`. The Tauri action remains build-only and
receives only `projectPath`.

- `actions/checkout@34e114876b0b11c390a56381ad16ebd13914f8d5`
- `actions/setup-node@49933ea5288caeca8642d1e84afbd3f7d6820020`
- `actions/cache@0057852bfaa89a56745cba8c7296529d2fc39830`
- `actions/upload-artifact@ea165f8d65b6e75b540449e92b4886f43607fa02`
- `dtolnay/rust-toolchain@4be7066ada62dd38de10e7b70166bc74ed198c30`
- `tauri-apps/tauri-action@84b9d35b5fc46c1e45415bdb6144030364f7ebc5`

The upstream ffmpeg ZIP is cached under
`ffmpeg-upstream-3.0.2-sha256-5005b9d49fad0a4fb2c34eb60fbb25739d00d01651255258c2f408c7ee8dc7be`.
The fetch step only downloads `upstream-bundle.zip` on a cache miss. A separate
step runs after both cache hits and misses, requiring the ZIP to be exactly
`6,846,809` bytes with lowercase SHA256
`5005b9d49fad0a4fb2c34eb60fbb25739d00d01651255258c2f408c7ee8dc7be`
before it replaces the fixed `upstream-bundle` extraction directory. That
cleanup target is guarded as a workspace child. Existing bundled-resource
validation still runs afterward.

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

Ordinary publish steps use the short-lived repository `GITHUB_TOKEN`. The two
read-only safety steps instead require repository secret `RELEASE_POLICY_TOKEN`,
a repository-scoped fine-grained token with only `Contents: read` and
`Administration: read`. A real Actions probe (run `29399211296`) confirmed that
the normal `GITHUB_TOKEN` cannot read the complete ruleset/Immutable management
state. After configuring the fine-grained secret, read-only probe run
`29402129399` passed the complete policy check while the package job remained
skipped. Missing policy credentials fail before any draft Release is created.

Its required order is:

1. Download the build artifact.
2. Verify the downloaded local assets.
3. Determine prerelease and Latest policy.
4. Resolve and peel the remote release tag to a commit; compare it with
   `source_sha`, require it to be reachable from remote `master`, then require
   the exact active no-bypass tag ruleset and Immutable Releases setting.
5. Refuse to continue if any same-tag Release already exists.
6. Create one draft Release with both assets.
7. Verify the draft Release and its assets.
8. Repeat the tag, remote `master` ancestry, complete ruleset, and Immutable
   Releases checks.
9. Publish the verified draft.
10. Verify the published Release, asset sizes and SHA256 digests, immutable
    state, prerelease state, and Latest result.

`GH_TOKEN` is scoped only to steps that invoke `gh`. Non-`gh` steps receive no
repository token, and the policy token is never exposed to Release mutation or
asset-transfer steps.

## Tag Source Integrity

The publish job reads the remote tag through the GitHub API and recursively
peels annotated tags until it reaches a commit. Cycles, excessive tag depth,
unexpected object types, malformed SHAs, API failures, a commit different from
`source_sha`, a commit outside remote `master`, ruleset drift, and disabled
Immutable Releases all fail closed.

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
2. **Draft:** the private draft must contain exactly those two names. Each
   remote byte size must equal the corresponding downloaded local file, and
   each `assets.digest` must equal `sha256:<lowercase local SHA256>`.
3. **Published:** the public Release must still contain exactly those two names
   with the same byte sizes and digests, and `isImmutable` must be true.

Missing, null, or mismatched remote digests fail closed. No draft is made public
until the local and draft checks pass.

## Existing Same-Tag Releases

Automation never deletes, replaces, uploads over, or otherwise modifies an
existing same-tag Release. After the tag API returns HTTP 200, it immediately
runs `gh release view --json isDraft,isImmutable,url`, checks the command exit,
parses the state, emits state-specific recovery guidance, and throws.

- **Existing draft:** an operator must inspect it first. If it is confirmed to
  be an unpublished failed-run artifact, the operator may delete only the draft
  Release while retaining the tag exactly where it is. Do not request any tag
  cleanup and do not delete, move, or recreate the tag. Then run
  `gh run rerun <run-id> --failed` for the same tag workflow.
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
- A published Release is never deleted or reused by this automation. Future
  published Releases must also be repository-immutable; any correction requires
  a higher version and a new tag.
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

The release workflow contract tests cover the README section, trigger and
permission boundaries, approved Action SHA pins, the content-addressed ffmpeg
ZIP and unconditional verification/extraction step, metadata transport, token
scope, existing Release state handling, native command exit guards, exact asset
count/size/digest, repository immutability, Latest policy, and both remote tag
peel checks.

The prepare-release contract separately locks the shared strict version grammar
and safe operator command order, plus all pre-tag failure modes and the exact
git/GitHub API gateway calls through fake dependencies. Accepted versions are
stable `X.Y.Z` or an `rc`, `beta`, or `alpha` prerelease followed by optional
dot-separated SemVer prerelease identifiers. ASCII numeric identifiers cannot
have leading zeroes; preview channels, build metadata, whitespace, Unicode
digits, and malformed or empty identifiers are rejected. Any workflow or
release preparation change must update implementation and its contract
together.
