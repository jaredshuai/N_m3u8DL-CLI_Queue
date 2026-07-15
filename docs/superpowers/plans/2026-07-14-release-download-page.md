# Release Download Page Implementation Plan

## Status

- Task 1: complete
- Task 2: complete
- Task 3: complete
- Task 4: complete
- Task 5: complete
- Task 6: complete
- Task 7: complete
- Task 8: complete

The executable behavior source of truth is `.github/workflows/release.yml` plus
`m3u8-queue-downloader/scripts/release-workflow.test.mjs` and
`m3u8-queue-downloader/scripts/prepare-release.test.mjs`. This plan records task
state, required invariants, and operator commands without copying workflow
PowerShell blocks.

## Goal

Publish `app-v0.2.2` from an immutable tag with a verified Windows x64 installer
and portable archive, expose the Latest Release page from README, and preserve a
clear recovery path for failed draft publication without modifying release tags
or published Releases.

## Current Workflow Contract

All release workflow contract tests cover the README entry, job/permission and
token boundaries, approved Action commit pins, the content-addressed ffmpeg ZIP
and unconditional verification/extraction step, metadata transport, existing
Release handling, native-command exit guards, exact asset name/size/digest,
repository immutability, Latest policy, and both remote tag peel checks.

All prepare-release contract tests separately lock the shared strict version
grammar, the executable command order printed by
`ReleasePrepareReporter.nextSteps`, every pre-tag failure/success path, and the
exact read-only git/GitHub API calls through fake dependencies.

The workflow order guarded by those tests is:

1. Build job: checkout without persisted credentials, resolve metadata, test,
   build, prepare assets, verify local assets, upload the Actions artifact.
2. Publish job: download the artifact, verify local assets, determine
   prerelease/Latest policy, verify the remote tag source, create a draft,
   verify draft asset names/sizes/digests, reverify the remote tag source,
   publish, then verify the public Release including `isImmutable=true`.

The publish job is serialized by `release-publication`. Repository credentials
are available only to individual steps that invoke `gh`; the two read-only
policy checks use a separate repository-scoped `RELEASE_POLICY_TOKEN` with
`Contents: read` and `Administration: read`.

## Task 1: Release Workflow Contract

**Status:** complete

**Files:**

- `.github/workflows/release.yml`
- `m3u8-queue-downloader/scripts/release-workflow.test.mjs`

Completed outcomes:

- All executable release workflow contract tests pass together.
- Metadata uses environment boundaries, Base64 version transport, strict UTF-8,
  strict project SemVer, and guarded native commands.
- Existing Releases are never mutated by automation.
- Local, draft, and published assets are verified by exact names and byte size;
  draft and published assets also require exact SHA256 digests.
- Latest policy and both remote tag peel checks are contract-protected.

## Task 2: Hardened Tag-Only Release Workflow

**Status:** complete

**File:** `.github/workflows/release.yml`

Completed outcomes:

- The only trigger is an `app-v*` tag push.
- The build job has `contents: read`; the separate publish job has only
  `contents: write` and `actions: read`.
- The build job emits validated version, `source_sha`, installer name, and
  portable name outputs.
- The publish job does not check out or build source.
- The first stable Release becomes Latest when no Latest exists; a higher stable
  advances Latest; equal or lower stable backfill keeps the current Latest;
  prerelease never becomes Latest.
- Any existing same-tag Release stops automation before draft creation.

## Task 3: README Download Entry And Release Documentation

**Status:** complete

**Files:**

- `README.md`
- `AGENTS.md`
- `m3u8-queue-downloader/scripts/release-workflow.test.mjs`
- `docs/superpowers/specs/2026-07-14-release-download-page-design.md`
- `docs/superpowers/plans/2026-07-14-release-download-page.md`

Completed outcomes:

- README links to this repository's Latest Release page.
- From `0.2.2`, README presents the two versioned asset patterns as the primary
  path and states Windows x64 plus bundled N_m3u8DL-CLI and ffmpeg support.
- A short compatibility note explains that a Latest Release still at `0.2.1`
  uses `portable.zip`, while its installer must be selected from that Release's
  actual asset list.
- The README contract extracts only the `## 下载` section and checks the exact
  URL, asset names, compatibility note, platform, and bundled components.
- AGENTS and design documentation use the workflow plus both contract tests as
  their behavior source of truth.

## Task 4: Prepare Version 0.2.2

**Status:** complete

Completion record: version `0.2.2` was prepared in exactly the five version
files, with `package-lock.json` refreshed using `--package-lock-only
--ignore-scripts`. The version commit was pushed to `master` before any tag was
created.

**Version files in the final Task 4 commit:**

- `m3u8-queue-downloader/package.json`
- `m3u8-queue-downloader/package-lock.json`
- `m3u8-queue-downloader/src-tauri/tauri.conf.json`
- `m3u8-queue-downloader/src-tauri/Cargo.toml`
- `m3u8-queue-downloader/src-tauri/Cargo.lock`

1. From `m3u8-queue-downloader`, update `package.json`, `tauri.conf.json`,
   `Cargo.toml`, and `Cargo.lock`:

   ```powershell
   npm run release:prepare -- 0.2.2
   ```

2. Update only the npm lockfile metadata without running lifecycle scripts:

   ```powershell
   npm install --package-lock-only --ignore-scripts
   ```

3. Confirm the four release versions agree, then use one multiline `rg`
   invocation to verify that both the package-lock root version and
   `packages[""]` version are `0.2.2`:

   ```powershell
   npm run check:versions
   rg -n -U -e '\A\{\r?\n  "name": "m3u8-queue-downloader",\r?\n  "version": "0\.2\.2",' -e '  "packages": \{\r?\n    "": \{\r?\n      "name": "m3u8-queue-downloader",\r?\n      "version": "0\.2\.2",' package-lock.json
   ```

   Expected: exactly two matches, one for each required package-lock location.

4. Review the diff and confirm no workflow, generated schema, or unrelated file
   changed.

5. Commit exactly the five version files:

   ```powershell
   git add package.json package-lock.json src-tauri/tauri.conf.json src-tauri/Cargo.toml src-tauri/Cargo.lock
   git commit -m "chore(release): v0.2.2"
   ```

Do not create or push the tag during Task 4.

## Task 5: Verify The Release Candidate

**Status:** complete

Completion record: the release candidate checks completed before tag creation.
No local Tauri package was substituted for the GitHub Actions Release build.

Run from `m3u8-queue-downloader` unless noted otherwise:

1. Verify all release/README contract tests:

   ```powershell
   node --test scripts/release-workflow.test.mjs
   ```

2. Run the complete frontend/script suite and static checks:

   ```powershell
   npm test
   ```

3. Build the frontend production bundle:

   ```powershell
   npm run build
   ```

4. Run the Rust tests using the same command as the Release build job:

   ```powershell
   cargo test --manifest-path src-tauri/Cargo.toml -- --nocapture
   ```

5. From the repository root, verify whitespace and repository scope:

   ```powershell
   git diff --check
   git status --short --branch
   ```

Do not substitute a local Tauri package for the GitHub Actions Release build;
this machine's local packaging remains secondary to the tag workflow.

## Task 6: Configure Tag Protection And Publish app-v0.2.2

**Status:** complete

Completion record:

- Active no-bypass tag ruleset: `18966944`
- Successful Release workflow run: `29387505665`
- Public Release: `https://github.com/jaredshuai/N_m3u8DL-CLI_Queue/releases/tag/app-v0.2.2`
- Public Latest: `app-v0.2.2`

Repository Immutable Releases was enabled after `app-v0.2.2` was published, so
that existing Release remains mutable. It is not deleted, replaced, or reused,
and its protected tag is never moved. Future workflow runs require the newly
published Release to report `isImmutable=true`.

### 1. Put the reviewed commits on master

After Tasks 1-5 are reviewed and integrated, update and push `master` before
creating the release tag.

```powershell
git push origin master
```

### 2. External controls used for app-v0.2.2

Task 6 configured and verified ruleset `18966944` before creating
`app-v0.2.2`. Repository Immutable Releases was enabled later, so the existing
Release remains mutable. This records the historical publication; it is not the
procedure for future tags.

Future tags must use
`node scripts/prepare-release.mjs pre-tag <version>` after pushing `master`.
That gate mechanically verifies clean `master`, `HEAD == origin/master`, all
five version files, local/remote tag absence, the complete named ruleset, and
the immutable-releases API before tag creation.

### 3. Create and push the immutable tag

Confirm the release commit is the intended `master` commit, then run:

```powershell
git tag app-v0.2.2
git push origin app-v0.2.2
```

The push triggers the Release workflow. The ruleset prevents later tag update or
deletion, so recovery must never move or recreate this tag.

### 4. Identify and watch the matching workflow run

```powershell
$tagSha = git rev-list -n 1 app-v0.2.2
gh run list --workflow release.yml --event push --limit 20 --json databaseId,headSha,status,conclusion,url
gh run watch <matching-run-id> --exit-status
```

Select the run whose `headSha` equals `$tagSha`.

### 5. Verify the public Release

```powershell
gh release view app-v0.2.2 --json url,name,tagName,isDraft,isPrerelease,assets
gh api repos/jaredshuai/N_m3u8DL-CLI_Queue/releases/latest --jq .tag_name
```

Expected Release state:

- `isDraft` is false.
- `isPrerelease` is false.
- Exactly these two non-empty assets exist:
  - `m3u8-queue-downloader_0.2.2_x64-setup.exe`
  - `m3u8-queue-downloader_0.2.2_portable_x64.zip`
- If no Latest existed, or the previous Latest was lower than `0.2.2`, Latest is
  `app-v0.2.2`.
- If a higher stable Latest already exists, `0.2.2` is a backfill with
  `latest=false`; the higher Latest remains unchanged.

Download both assets to a temporary directory and confirm their names and
positive byte sizes:

```powershell
gh release download app-v0.2.2 --dir <temporary-directory>
```

### 6. Recover only through the supported path

- **Failure before any Release exists:** fix the cause and rerun the same Actions
  run with `gh run rerun <run-id> --failed`.
- **Existing draft from the failed run:** inspect it first. If it is confirmed
  unpublished and disposable, delete only that draft Release in GitHub, retain
  the existing tag, do not request tag cleanup, then run
  `gh run rerun <run-id> --failed`.
- **Publish job failed after a successful windows job:** `--failed` preserves the
  successful build job and its existing `release-assets` artifact, then reruns
  the failed publish path. Do not request a full rerun: rebuilding would attempt
  to upload the same fixed `upload-artifact` v4.6.2 artifact name again.
- **Windows build job failed:** `--failed` reruns that failed job and the
  necessary downstream publish work after the build succeeds.
- **Existing published Release:** do not delete, replace, or reuse it. Return to
  Task 4 with a higher version and publish from a new tag.
- **Build source or code must change:** prepare a higher version and new tag.
  Never rewrite, move, or recreate the protected existing tag.

The workflow itself never performs existing-Release cleanup, and the recovery
procedure never uses a full workflow rerun.

## Task 7: Harden Future Release Supply Chain

**Status:** complete

**Files:**

- `.github/workflows/release.yml`
- `m3u8-queue-downloader/scripts/release-workflow.test.mjs`
- `m3u8-queue-downloader/scripts/prepare-release.mjs`
- `m3u8-queue-downloader/scripts/prepare-release.test.mjs`
- `AGENTS.md`
- `docs/superpowers/specs/2026-07-14-release-download-page-design.md`
- `docs/superpowers/plans/2026-07-14-release-download-page.md`

Completed outcomes:

- `release:prepare` now accepts exactly the workflow version grammar and prints
  the lockfile/check/stage/commit/push/pre-tag-gate/tag sequence in that order.
- `node scripts/prepare-release.mjs pre-tag <version>` is the only pre-tag
  operator gate. It requires clean `master` at `origin/master`, validates all
  five version files and tag absence, reads the exact named no-bypass ruleset,
  and requires repository Immutable Releases through API version `2026-03-10`.
  On success it prints tag commands bound to the verified `origin/master` SHA;
  the earlier `release:prepare` output no longer prints an unbound tag command.
  Unit tests inject fake git/gh dependencies; Task 7 does not run the real gate
  from its feature branch.
- Before draft creation and again before publication, the Release workflow
  independently requires tag/source equality, remote `master` ancestry, the
  complete named no-bypass ruleset, and enabled repository Immutable Releases.
  A direct tag push therefore cannot bypass the local operator gate.
- A real Actions probe run (`29399211296`) confirmed that the ordinary
  `GITHUB_TOKEN` cannot expose the complete policy state. The two safety steps
  therefore require repository secret `RELEASE_POLICY_TOKEN`, scoped only to
  this repository with `Contents: read` and `Administration: read`; all Release
  mutations continue to use the short-lived `GITHUB_TOKEN`. After the secret
  was configured, read-only probe run `29402129399` passed the full ruleset and
  Immutable Releases checks with the Windows package job skipped.
- Existing same-tag draft and published Releases produce opposite, explicit
  recovery instructions after a read-only state query; automation always throws
  without mutation.
- Draft and published assets require exact remote `assets.digest` values derived
  from local SHA256; future published Releases also require `isImmutable=true`.
- Every Release workflow action is pinned to the approved commit:
  - `actions/checkout@34e114876b0b11c390a56381ad16ebd13914f8d5` (`v4.3.1`)
  - `actions/setup-node@49933ea5288caeca8642d1e84afbd3f7d6820020` (`v4.4.0`)
  - `actions/cache@0057852bfaa89a56745cba8c7296529d2fc39830` (`v4.3.0`)
  - `actions/upload-artifact@ea165f8d65b6e75b540449e92b4886f43607fa02` (`v4.6.2`)
  - `dtolnay/rust-toolchain@4be7066ada62dd38de10e7b70166bc74ed198c30` (`stable`)
  - `tauri-apps/tauri-action@84b9d35b5fc46c1e45415bdb6144030364f7ebc5` (`v0.6.2`)
- The ffmpeg cache key includes SHA256
  `5005b9d49fad0a4fb2c34eb60fbb25739d00d01651255258c2f408c7ee8dc7be`;
  the cache stores only `upstream-bundle.zip`, and every cache hit/miss verifies
  that digest and exactly `6,846,809` bytes before guarded replacement of the
  extraction directory.
- Task 7 itself did not trigger a Release workflow or create a tag. Read-only
  `Package GUI` probes were run only to verify token permissions and the
  configured policy secret.

## Task 8: Publish Hardened v0.2.3

**Status:** complete

Completed outcomes:

- Version commit `381cb1410740f2edae71d8872a209db1e7e159b2` updated exactly the five
  release version files to `0.2.3` and passed frontend, Svelte, Vite, Rust,
  actionlint, and diff checks before tagging.
- The mechanical pre-tag gate passed against clean `master == origin/master`,
  ruleset `18966944`, enabled Immutable Releases, and absent local/remote
  `app-v0.2.3` refs before creating the SHA-bound tag.
- Release workflow run `29404677585` completed successfully. The published
  stable Release is non-draft, non-prerelease, immutable, and current Latest.
- Installer `m3u8-queue-downloader_0.2.3_x64-setup.exe` is `8,513,342` bytes
  with SHA256 `d546ad9286bc7ab3426c66a036a99b59a1f3b1bf72b3c8b74f95beafe45a5370`.
- Portable `m3u8-queue-downloader_0.2.3_portable_x64.zip` is `16,743,818`
  bytes with SHA256
  `c922ac2fdba8a4ba513e72b018e812211587ccd63887745ff119bcf9b5b54469`.
  Independent download inspection confirmed the application executable,
  bundled ffmpeg, and `N_m3u8DL-CLI_v3.0.2.exe` are present.
