# Release Download Page Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Publish `app-v0.2.2` through a tag-only GitHub Release that exposes a verified Windows installer and portable archive, with a README download entry.

**Architecture:** Keep daily packaging in `Package GUI`. Refactor only the tag-triggered `Release` workflow so Tauri builds locally, a draft Release stages both verified assets, and a final explicit publish step makes the page public. Add a small Node contract test that guards the workflow ordering and README link without introducing a YAML dependency.

**Tech Stack:** GitHub Actions, PowerShell, GitHub CLI, Tauri Action, Node.js built-in test runner, Markdown, existing release preparation script.

---

## File Map

- Modify `.github/workflows/release.yml`: tag-only build, local asset verification, draft staging, remote verification, final publication.
- Create `m3u8-queue-downloader/scripts/release-workflow.test.mjs`: static release workflow and README contract tests.
- Modify `README.md`: user-facing Latest Release download section.
- Modify version files through `npm run release:prepare -- 0.2.2`:
  `package.json`, `src-tauri/tauri.conf.json`, `src-tauri/Cargo.toml`, and
  `src-tauri/Cargo.lock`.
- Refresh `m3u8-queue-downloader/package-lock.json` after the version bump.
- Update `AGENTS.md` only if release asset names or documented release behavior become inaccurate.

## Chunk 1: Release Workflow Contract

### Task 1: Add Failing Workflow And README Contract Tests

**Files:**
- Create: `m3u8-queue-downloader/scripts/release-workflow.test.mjs`
- Test: `m3u8-queue-downloader/scripts/release-workflow.test.mjs`

- [ ] **Step 1: Write the failing tests**

```js
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import test from 'node:test';
import { fileURLToPath } from 'node:url';

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(scriptDir, '..', '..');
const workflow = fs.readFileSync(
  path.join(repoRoot, '.github', 'workflows', 'release.yml'),
  'utf8',
);

test('release workflow publishes only app version tags', () => {
  assert.match(workflow, /tags:\s*\n\s*- ['"]app-v\*['"]/);
  assert.doesNotMatch(workflow, /workflow_dispatch:/);

  const windowsJob = workflow.indexOf('  windows:');
  const steps = workflow.indexOf('    steps:', windowsJob);
  const jobToken = workflow.indexOf(
    '      GH_TOKEN: ${{ secrets.GITHUB_TOKEN }}',
    windowsJob,
  );
  assert.ok(windowsJob >= 0);
  assert.ok(windowsJob < jobToken);
  assert.ok(jobToken < steps);
});

test('release workflow verifies assets before staging and publishes only after remote verification', () => {
  const localVerify = workflow.indexOf('Verify local release assets');
  const draftCreate = workflow.indexOf('Stage draft GitHub Release');
  const remoteVerify = workflow.indexOf('Verify draft release assets');
  const publish = workflow.indexOf('Publish verified GitHub Release');

  assert.ok(localVerify >= 0);
  assert.ok(localVerify < draftCreate);
  assert.ok(draftCreate < remoteVerify);
  assert.ok(remoteVerify < publish);
  assert.match(workflow, /gh release create[\s\S]*--draft[\s\S]*--verify-tag[\s\S]*--generate-notes/);
  assert.match(workflow, /gh release edit[\s\S]*--draft=false/);
});

test('release workflow names installer and portable assets explicitly', () => {
  assert.match(workflow, /m3u8-queue-downloader_\$\{version\}_x64-setup\.exe/);
  assert.match(workflow, /m3u8-queue-downloader_\$\{version\}_portable_x64\.zip/);
});
```

- [ ] **Step 2: Run the focused test and verify RED**

Run:

```powershell
node --test scripts/release-workflow.test.mjs
```

Working directory: `m3u8-queue-downloader`

Expected: failures for `workflow_dispatch`, missing staged publication steps,
and generic `portable.zip`.

### Task 2: Refactor The Tag-Only Release Workflow

**Files:**
- Modify: `.github/workflows/release.yml`
- Test: `m3u8-queue-downloader/scripts/release-workflow.test.mjs`

- [ ] **Step 1: Restrict release creation to immutable tags**

Replace the trigger with:

```yaml
on:
  push:
    tags:
      - 'app-v*'
```

Remove the manual draft/prerelease inputs. Prerelease state is derived from the
tag suffix. Add a job-level token so every `gh` step is authenticated:

```yaml
jobs:
  windows:
    permissions:
      contents: write
    env:
      GH_TOKEN: ${{ secrets.GITHUB_TOKEN }}
```

The Tauri Action may continue receiving `GITHUB_TOKEN` in its own step env;
`GH_TOKEN` is specifically for GitHub CLI commands across staging,
verification, and publication steps.

- [ ] **Step 2: Resolve and validate release metadata before tests/build**

Add a PowerShell step with id `release`:

```powershell
$version = node -p "require('./m3u8-queue-downloader/package.json').version"
$expectedTag = "app-v$version"
if ('${{ github.ref_name }}' -ne $expectedTag) {
  throw "Release tag ${{ github.ref_name }} does not match package version $version."
}

"version=$version" >> $env:GITHUB_OUTPUT
"installer=m3u8-queue-downloader_${version}_x64-setup.exe" >> $env:GITHUB_OUTPUT
"portable=m3u8-queue-downloader_${version}_portable_x64.zip" >> $env:GITHUB_OUTPUT
```

- [ ] **Step 3: Make Tauri Action build without publishing**

Keep `tauri-apps/tauri-action@v0` with `projectPath`, but remove `tagName`,
`releaseName`, `releaseBody`, `releaseDraft`, and `prerelease` inputs.

- [ ] **Step 4: Stage the installer and portable archive under exact names**

Replace the existing portable preparation/upload steps with one root-level
PowerShell step:

```powershell
$workspaceRoot = (Resolve-Path '.').Path
$projectRoot = Join-Path $workspaceRoot 'm3u8-queue-downloader'
$portableRoot = Join-Path $projectRoot '.portable-dist'
$portableDir = Join-Path $portableRoot 'm3u8-queue-downloader-portable'
$releaseAssetsRoot = Join-Path $workspaceRoot 'release-assets'

function Assert-WorkspaceChild([string]$path) {
  $fullPath = [System.IO.Path]::GetFullPath($path)
  $workspacePrefix = $workspaceRoot.TrimEnd('\') + '\'
  if (-not $fullPath.StartsWith($workspacePrefix, [System.StringComparison]::OrdinalIgnoreCase)) {
    throw "Refusing to modify path outside workspace: $fullPath"
  }
  return $fullPath
}

$portableRoot = Assert-WorkspaceChild $portableRoot
$releaseAssetsRoot = Assert-WorkspaceChild $releaseAssetsRoot
foreach ($target in @($portableRoot, $releaseAssetsRoot)) {
  if (Test-Path -LiteralPath $target) {
    Remove-Item -LiteralPath $target -Recurse -Force
  }
}

New-Item -ItemType Directory -Path $portableDir -Force | Out-Null
New-Item -ItemType Directory -Path $releaseAssetsRoot -Force | Out-Null

$releaseRoot = Join-Path $projectRoot 'src-tauri/target/release'
Copy-Item -LiteralPath (Join-Path $releaseRoot 'm3u8-queue-downloader.exe') `
  -Destination $portableDir -Force

Get-ChildItem -LiteralPath $releaseRoot -Filter '*.dll' -File | ForEach-Object {
  Copy-Item -LiteralPath $_.FullName -Destination (Join-Path $portableDir $_.Name) -Force
}

$resources = Join-Path $releaseRoot 'resources'
if (-not (Test-Path -LiteralPath $resources)) {
  throw "Portable resources directory was not found: $resources"
}
Copy-Item -LiteralPath $resources -Destination (Join-Path $portableDir 'resources') `
  -Recurse -Force

$defaultFfmpegDir = Join-Path $portableDir 'lib/ffmpeg/tools/ffmpeg/bin'
New-Item -ItemType Directory -Path $defaultFfmpegDir -Force | Out-Null
Copy-Item -LiteralPath (Join-Path $portableDir 'resources/ffmpeg.exe') `
  -Destination (Join-Path $defaultFfmpegDir 'ffmpeg.exe') -Force

$installerDir = Join-Path $releaseRoot 'bundle/nsis'
$installers = @(Get-ChildItem -LiteralPath $installerDir -Filter '*-setup.exe' -File)
if ($installers.Count -ne 1) {
  throw "Expected exactly one NSIS installer, found $($installers.Count)."
}

$installerTarget = Join-Path $releaseAssetsRoot '${{ steps.release.outputs.installer }}'
$portableTarget = Join-Path $releaseAssetsRoot '${{ steps.release.outputs.portable }}'
Copy-Item -LiteralPath $installers[0].FullName -Destination $installerTarget -Force
Compress-Archive -Path (Join-Path $portableDir '*') -DestinationPath $portableTarget -Force
```

- [ ] **Step 5: Verify local assets before creating any Release**

Add `Verify local release assets` that requires:

```powershell
$expected = @(
  '${{ steps.release.outputs.installer }}',
  '${{ steps.release.outputs.portable }}'
)
$files = @(Get-ChildItem -LiteralPath 'release-assets' -File)
if ($files.Count -ne 2) { throw 'Expected exactly two release assets.' }
foreach ($name in $expected) {
  $matches = @($files | Where-Object Name -eq $name)
  if ($matches.Count -ne 1 -or $matches[0].Length -le 0) {
    throw "Missing, duplicate, or empty release asset: $name"
  }
}
```

- [ ] **Step 6: Stage a draft Release and support safe retries**

Add `Stage draft GitHub Release` using `GH_TOKEN`:

```powershell
$tag = '${{ github.ref_name }}'
$repo = '${{ github.repository }}'
$existingJson = & gh release view $tag --repo $repo --json isDraft 2>$null
$existingExit = $LASTEXITCODE

if ($existingExit -eq 0) {
  $existing = $existingJson | ConvertFrom-Json
  if (-not $existing.isDraft) {
    throw "Published release already exists for $tag; refusing to modify it."
  }
  & gh release delete $tag --repo $repo --yes
  if ($LASTEXITCODE -ne 0) {
    throw "Failed to delete stale draft release for $tag."
  }
}

& gh release create $tag `
  'release-assets/${{ steps.release.outputs.installer }}' `
  'release-assets/${{ steps.release.outputs.portable }}' `
  --repo $repo `
  --title 'm3u8 Queue Downloader v${{ steps.release.outputs.version }}' `
  --draft `
  --verify-tag `
  --generate-notes
if ($LASTEXITCODE -ne 0) {
  throw "Failed to stage draft GitHub Release for $tag."
}
```

`gh release delete` is intentionally called without `--cleanup-tag`, so the
immutable release tag remains intact. A failed lookup is allowed to fall
through to `gh release create`; creation still fails safely on network or API
errors.

- [ ] **Step 7: Verify remote draft assets**

Add `Verify draft release assets`:

```powershell
$tag = '${{ github.ref_name }}'
$repo = '${{ github.repository }}'
$releaseJson = & gh release view $tag --repo $repo --json isDraft,isPrerelease,assets
if ($LASTEXITCODE -ne 0) {
  throw "Failed to read draft release for $tag."
}

$release = $releaseJson | ConvertFrom-Json
if (-not $release.isDraft) {
  throw "Release $tag became public before asset verification."
}

$expected = @(
  '${{ steps.release.outputs.installer }}',
  '${{ steps.release.outputs.portable }}'
)
$assets = @($release.assets)
if ($assets.Count -ne 2) {
  throw "Expected exactly two remote release assets, found $($assets.Count)."
}
foreach ($name in $expected) {
  $matches = @($assets | Where-Object name -eq $name)
  if ($matches.Count -ne 1 -or [int64]$matches[0].size -le 0) {
    throw "Missing, duplicate, or empty remote release asset: $name"
  }
}
```

- [ ] **Step 8: Publish only the verified draft**

Add `Publish verified GitHub Release`:

```powershell
$tag = '${{ github.ref_name }}'
$args = @('release', 'edit', $tag, '--repo', '${{ github.repository }}', '--draft=false')
if ($tag -match '-(rc|beta|alpha)') {
  $args += '--prerelease'
} else {
  $args += '--prerelease=false'
  $args += '--latest'
}
& gh @args
if ($LASTEXITCODE -ne 0) { throw 'Failed to publish verified GitHub Release.' }
```

- [ ] **Step 9: Run focused tests and verify GREEN**

Run:

```powershell
node --test scripts/release-workflow.test.mjs
```

Expected: 3 tests pass.

- [ ] **Step 10: Commit the workflow contract**

```powershell
git add .github/workflows/release.yml m3u8-queue-downloader/scripts/release-workflow.test.mjs
git commit -m "ci: publish verified release downloads"
```

## Chunk 2: User Download Entry And Version

### Task 3: Add The README Download Section

**Files:**
- Modify: `README.md`
- Test: `m3u8-queue-downloader/scripts/release-workflow.test.mjs`

- [ ] **Step 1: Add the failing README contract test**

Extend `release-workflow.test.mjs`:

```js
const readme = fs.readFileSync(path.join(repoRoot, 'README.md'), 'utf8');

test('README exposes the latest release download page and both package choices', () => {
  assert.match(readme, /## 下载/);
  assert.match(readme, /releases\/latest/);
  assert.match(readme, /安装版/);
  assert.match(readme, /便携版/);
});
```

- [ ] **Step 2: Run the README contract test and verify RED**

Run:

```powershell
node --test scripts/release-workflow.test.mjs
```

Expected: the three workflow tests pass and the README test fails because the
download section is absent.

- [ ] **Step 3: Add a concise download section after the introduction**

```markdown
## 下载

前往 [GitHub Releases 最新版本](https://github.com/jaredshuai/N_m3u8DL-CLI_Queue/releases/latest) 下载：

- **安装版**：`m3u8-queue-downloader_<版本>_x64-setup.exe`
- **便携版**：`m3u8-queue-downloader_<版本>_portable_x64.zip`，解压后直接运行

两个版本都已内置 N_m3u8DL-CLI 和 ffmpeg，仅支持 Windows x64。
```

- [ ] **Step 4: Run the README contract test and verify GREEN**

Run:

```powershell
node --test scripts/release-workflow.test.mjs
```

Expected: all 4 tests pass.

- [ ] **Step 5: Commit the README entry and its contract test**

```powershell
git add README.md m3u8-queue-downloader/scripts/release-workflow.test.mjs
git commit -m "docs: add release download entry"
```

### Task 4: Prepare Version 0.2.2

**Files:**
- Modify: `m3u8-queue-downloader/package.json`
- Modify: `m3u8-queue-downloader/package-lock.json`
- Modify: `m3u8-queue-downloader/src-tauri/tauri.conf.json`
- Modify: `m3u8-queue-downloader/src-tauri/Cargo.toml`
- Modify: `m3u8-queue-downloader/src-tauri/Cargo.lock`

- [ ] **Step 1: Run the repository release preparation command**

```powershell
npm run release:prepare -- 0.2.2
```

Working directory: `m3u8-queue-downloader`

- [ ] **Step 2: Refresh npm lockfile metadata**

```powershell
npm install --package-lock-only --ignore-scripts
```

- [ ] **Step 3: Verify all version-bearing files**

```powershell
npm run check:versions
rg -n '"version": "0\.2\.2"' package.json package-lock.json src-tauri/tauri.conf.json
rg -n '^version = "0\.2\.2"$' src-tauri/Cargo.toml src-tauri/Cargo.lock
```

Expected: version guard passes; package and lock metadata report `0.2.2`.

- [ ] **Step 4: Commit the release version**

```powershell
git add package.json package-lock.json src-tauri/tauri.conf.json src-tauri/Cargo.toml src-tauri/Cargo.lock
git commit -m "chore(release): v0.2.2"
```

## Chunk 3: Verification And Publication

### Task 5: Verify The Release Candidate

**Files:**
- Verify all modified files; no new edits expected.

- [ ] **Step 1: Run frontend tests and checks**

```powershell
npm test
```

Expected: version check, Svelte check, Node tests, and script tests pass.

- [ ] **Step 2: Run the frontend production build**

```powershell
npm run build
```

Expected: Vite exits 0; the existing mixed dynamic/static import warning is
allowed.

- [ ] **Step 3: Run Rust verification**

```powershell
cargo check --locked --manifest-path src-tauri/Cargo.toml
cargo test --locked --manifest-path src-tauri/Cargo.toml
```

Expected: all Rust and architecture tests pass.

- [ ] **Step 4: Verify repository state**

```powershell
git diff --check
git status --short --branch
```

Expected: no whitespace errors and only committed release work.

### Task 6: Push And Publish app-v0.2.2

**Files:**
- No source edits.

- [ ] **Step 1: Push master**

```powershell
git push origin master
```

- [ ] **Step 2: Create and push the immutable release tag**

```powershell
git tag app-v0.2.2
git push origin app-v0.2.2
```

- [ ] **Step 3: Identify and watch the Release workflow**

```powershell
$tagSha = git rev-list -n 1 app-v0.2.2
$run = $null
for ($attempt = 0; $attempt -lt 12 -and $null -eq $run; $attempt++) {
  $runs = gh run list --workflow release.yml --event push --limit 20 `
    --json databaseId,headSha,status,conclusion,url | ConvertFrom-Json
  $run = $runs | Where-Object headSha -eq $tagSha | Select-Object -First 1
  if ($null -eq $run) {
    Start-Sleep -Seconds 5
  }
}
if ($null -eq $run) {
  throw "Release workflow run was not found for tag SHA $tagSha."
}
gh run watch $run.databaseId --exit-status
if ($LASTEXITCODE -ne 0) {
  throw "Release workflow failed: $($run.url)"
}
```

Expected: workflow conclusion `success`.

- [ ] **Step 4: Verify the public Release page**

```powershell
gh release view app-v0.2.2 --json url,name,tagName,isDraft,isPrerelease,assets
```

Expected:

- `isDraft: false`
- `isPrerelease: false`
- exactly two non-empty assets with the approved installer and portable names

- [ ] **Step 5: Download and validate both published assets**

```powershell
$tempRoot = [System.IO.Path]::GetFullPath([System.IO.Path]::GetTempPath())
$validationRoot = Join-Path $tempRoot "m3u8-release-verify-$([guid]::NewGuid())"
$expectedNames = @(
  'm3u8-queue-downloader_0.2.2_x64-setup.exe',
  'm3u8-queue-downloader_0.2.2_portable_x64.zip'
)

New-Item -ItemType Directory -Path $validationRoot -Force | Out-Null
try {
  gh release download app-v0.2.2 --dir $validationRoot
  if ($LASTEXITCODE -ne 0) {
    throw 'Failed to download published release assets.'
  }

  $files = @(Get-ChildItem -LiteralPath $validationRoot -File)
  if ($files.Count -ne 2) {
    throw "Expected exactly two downloaded assets, found $($files.Count)."
  }
  foreach ($name in $expectedNames) {
    $matches = @($files | Where-Object Name -eq $name)
    if ($matches.Count -ne 1 -or $matches[0].Length -le 0) {
      throw "Missing, duplicate, or empty downloaded asset: $name"
    }
  }
} finally {
  $resolvedValidationRoot = [System.IO.Path]::GetFullPath($validationRoot)
  $tempPrefix = $tempRoot.TrimEnd('\') + '\'
  if (-not $resolvedValidationRoot.StartsWith(
    $tempPrefix,
    [System.StringComparison]::OrdinalIgnoreCase
  )) {
    throw "Refusing to remove validation path outside temp: $resolvedValidationRoot"
  }
  if (Test-Path -LiteralPath $resolvedValidationRoot) {
    Remove-Item -LiteralPath $resolvedValidationRoot -Recurse -Force
  }
}
```
