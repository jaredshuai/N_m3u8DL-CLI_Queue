import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';
import { fileURLToPath } from 'node:url';

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(scriptDir, '..', '..');
const workflow = normalizeNewlines(
  fs.readFileSync(path.join(repoRoot, '.github', 'workflows', 'release.yml'), 'utf8'),
);
const readme = normalizeNewlines(
  fs.readFileSync(path.join(repoRoot, 'README.md'), 'utf8'),
);
const downloadSection = extractMarkdownSection(readme, '下载');
const onBlock = extractIndentedBlock(workflow, /^on:\s*$/, 0);
const jobsBlock = extractIndentedBlock(workflow, /^jobs:\s*$/, 0);
const windowsJob = jobsBlock && extractIndentedBlock(jobsBlock.text, /^  windows:\s*$/, 2);
const publishJob = jobsBlock && extractIndentedBlock(jobsBlock.text, /^  publish:\s*$/, 2);
const windowsStepsBlock = windowsJob &&
  extractIndentedBlock(windowsJob.text, /^    steps:\s*$/, 4);
const publishStepsBlock = publishJob &&
  extractIndentedBlock(publishJob.text, /^    steps:\s*$/, 4);
const windowsSteps = windowsStepsBlock ? extractNamedSteps(windowsStepsBlock.text) : [];
const publishSteps = publishStepsBlock ? extractNamedSteps(publishStepsBlock.text) : [];
const allSteps = [...windowsSteps, ...publishSteps];
const approvedActionPins = new Map([
  ['actions/checkout', {
    sha: '34e114876b0b11c390a56381ad16ebd13914f8d5',
    comment: 'v4.3.1',
  }],
  ['actions/setup-node', {
    sha: '49933ea5288caeca8642d1e84afbd3f7d6820020',
    comment: 'v4.4.0',
  }],
  ['actions/cache', {
    sha: '0057852bfaa89a56745cba8c7296529d2fc39830',
    comment: 'v4.3.0',
  }],
  ['actions/upload-artifact', {
    sha: 'ea165f8d65b6e75b540449e92b4886f43607fa02',
    comment: 'v4.6.2',
  }],
  ['dtolnay/rust-toolchain', {
    sha: '4be7066ada62dd38de10e7b70166bc74ed198c30',
    comment: 'stable',
  }],
  ['tauri-apps/tauri-action', {
    sha: '84b9d35b5fc46c1e45415bdb6144030364f7ebc5',
    comment: 'v0.6.2',
  }],
]);

test('README exposes the latest release download page and both package choices', () => {
  assert.ok(downloadSection, 'README must contain one ## 下载 section');
  assert.deepEqual(
    extractMarkdownLinks(downloadSection),
    ['https://github.com/jaredshuai/N_m3u8DL-CLI_Queue/releases/latest'],
    'download section must link only to this repository latest Release page',
  );
  assert.deepEqual(
    extractInlineCode(downloadSection),
    [
      'm3u8-queue-downloader_<版本>_x64-setup.exe',
      'm3u8-queue-downloader_<版本>_portable_x64.zip',
      'portable.zip',
    ],
    'download section must list both 0.2.2+ asset patterns and the 0.2.1 legacy portable name',
  );
  assert.match(
    downloadSection,
    /^从 0\.2\.2 起，Release 使用以下版本化资产名：$/m,
  );
  assert.match(
    downloadSection,
    /^- \*\*安装版\*\*：`m3u8-queue-downloader_<版本>_x64-setup\.exe`$/m,
  );
  assert.match(
    downloadSection,
    /^- \*\*便携版\*\*：`m3u8-queue-downloader_<版本>_portable_x64\.zip`，解压后直接运行$/m,
  );
  assert.match(
    downloadSection,
    /^两个版本都已内置 N_m3u8DL-CLI 和 ffmpeg，仅支持 Windows x64。$/m,
  );
  assert.match(
    downloadSection,
    /^兼容提示：若 Latest 仍为 0\.2\.1，其便携包资产名为 `portable\.zip`；安装包请按该 Release 页面列出的实际资产选择。$/m,
  );
});

test('release workflow has a read-only build job and a minimal publish job', () => {
  assert.ok(onBlock, 'release workflow must define one top-level on block');
  const onShape = significantLines(onBlock.text).join('\n');
  assert.match(
    onShape,
    /^on:\n  push:\n    tags:\n      - ['"]app-v\*['"]$/,
    'top-level on block must contain only push.tags with app-v*',
  );

  assert.ok(jobsBlock, 'release workflow must define one top-level jobs block');
  assert.deepEqual(
    extractChildKeys(jobsBlock.text, 2),
    ['windows', 'publish'],
    'release workflow must contain only windows and publish jobs',
  );
  assert.ok(windowsJob, 'jobs must contain one windows build job');
  assert.ok(publishJob, 'jobs must contain one publish job');
  assert.equal(
    extractIndentedBlock(workflow, /^concurrency:\s*$/, 0),
    null,
    'build jobs must not be serialized by workflow-level concurrency',
  );
  assert.match(publishJob.text, /^    needs:\s*windows\s*$/m);
  assert.match(publishJob.text, /^    runs-on:\s*windows-latest\s*$/m);
  assert.deepEqual(
    trimmedBlockLines(requireBlock(publishJob.text, /^    concurrency:\s*$/, 4)),
    ['concurrency:', 'group: release-publication', 'cancel-in-progress: false'],
    'all publish jobs must share one repository-wide publication lock',
  );

  assert.deepEqual(
    trimmedBlockLines(requireBlock(windowsJob.text, /^    permissions:\s*$/, 4)),
    ['permissions:', 'contents: read'],
    'windows permissions must contain only contents: read',
  );
  assert.deepEqual(
    trimmedBlockLines(requireBlock(publishJob.text, /^    permissions:\s*$/, 4)),
    ['permissions:', 'contents: write', 'actions: read'],
    'publish permissions must contain only contents: write and actions: read',
  );

  assert.deepEqual(
    trimmedBlockLines(requireBlock(windowsJob.text, /^    outputs:\s*$/, 4)),
    [
      'outputs:',
      'version: ${{ steps.release.outputs.version }}',
      'source_sha: ${{ steps.release.outputs.source_sha }}',
      'installer: ${{ steps.release.outputs.installer }}',
      'portable: ${{ steps.release.outputs.portable }}',
    ],
    'windows job must expose all validated release metadata',
  );

  const checkout = requireStep(windowsSteps, 'Checkout');
  assert.match(
    checkout.text,
    /^        uses:\s*actions\/checkout@34e114876b0b11c390a56381ad16ebd13914f8d5\s+# v4\.3\.1\s*$/m,
  );
  assert.deepEqual(
    trimmedBlockLines(requireBlock(checkout.text, /^        with:\s*$/, 8)),
    ['with:', 'persist-credentials: false'],
    'checkout must not persist repository credentials',
  );
  assert.equal(
    publishSteps.some((step) => /actions\/checkout@/i.test(step.text)),
    false,
    'publish job must not checkout source code',
  );
  assert.doesNotMatch(
    publishJob.text,
    /^ {8}uses:|\b(?:npm|cargo)\b|tauri-action@/m,
    'publish job must not run actions, npm, cargo, or Tauri builds',
  );

  const tauri = requireStep(windowsSteps, 'Build Tauri release bundles');
  assert.deepEqual(
    trimmedBlockLines(requireBlock(tauri.text, /^        with:\s*$/, 8)),
    ['with:', 'projectPath: m3u8-queue-downloader'],
    'Tauri action must receive only projectPath',
  );
  assert.equal(extractIndentedBlock(tauri.text, /^        env:\s*$/, 8), null);
  assert.doesNotMatch(
    tauri.text,
    /GITHUB_TOKEN|GH_TOKEN|tagName|releaseId|releaseDraft|prerelease|releaseName|releaseBody/,
    'Tauri action must not receive a token or release publishing inputs',
  );
});

test('release workflow pins every action to an approved commit SHA', () => {
  const useLines = workflow.split('\n').filter((line) => /^\s*uses:\s*/.test(line));
  const observedCounts = new Map();

  for (const line of useLines) {
    const match = /^\s*uses:\s*([^@\s]+)@([^\s#]+)(?:\s+#\s*(\S.*?))?\s*$/.exec(line);
    assert.ok(match, `unable to parse action use: ${line}`);
    const [, action, ref, comment] = match;
    assert.match(ref, /^[0-9a-f]{40}$/, `${action} must use a full lowercase commit SHA`);

    const approved = approvedActionPins.get(action);
    assert.ok(approved, `unapproved action in release workflow: ${action}`);
    assert.equal(ref, approved.sha, `${action} must use the approved commit`);
    assert.equal(comment, approved.comment, `${action} must retain its version comment`);
    observedCounts.set(action, (observedCounts.get(action) ?? 0) + 1);
  }

  assert.deepEqual(
    Object.fromEntries(observedCounts),
    {
      'actions/checkout': 1,
      'actions/setup-node': 1,
      'dtolnay/rust-toolchain': 1,
      'actions/cache': 1,
      'actions/upload-artifact': 2,
      'tauri-apps/tauri-action': 1,
    },
  );

  const rust = requireStep(windowsSteps, 'Setup Rust');
  assert.deepEqual(
    trimmedBlockLines(requireBlock(rust.text, /^        with:\s*$/, 8)),
    ['with:', 'toolchain: stable'],
    'the pinned rust-toolchain action must still select stable explicitly',
  );
});

test('ffmpeg upstream bundle is content-addressed and verified before extraction', () => {
  const cache = requireStep(windowsSteps, 'Cache ffmpeg upstream bundle');
  assert.deepEqual(
    trimmedBlockLines(requireBlock(cache.text, /^        with:\s*$/, 8)),
    [
      'with:',
      'path: upstream-bundle.zip',
      'key: ffmpeg-upstream-3.0.2-sha256-5005b9d49fad0a4fb2c34eb60fbb25739d00d01651255258c2f408c7ee8dc7be',
    ],
  );

  const fetch = requireStep(windowsSteps, 'Fetch ffmpeg from upstream release');
  assert.match(fetch.text, /^        if:\s*steps\.cache-ffmpeg\.outputs\.cache-hit != 'true'\s*$/m);
  const fetchRun = normalizePowerShell(extractRunBlock(fetch.text) ?? '');
  assert.match(fetchRun, /gh release download 3\.0\.2/);
  assert.match(fetchRun, /--output upstream-bundle\.zip/);
  assert.doesNotMatch(
    fetchRun,
    /expectedBundleSize|expectedBundleSha256|Get-FileHash|Expand-Archive|Remove-Item/,
    'cache miss step must only download the ZIP',
  );

  const verify = requireStep(windowsSteps, 'Verify and extract ffmpeg upstream bundle');
  assert.equal(
    extractIndentedBlock(verify.text, /^        if:\s*/, 8),
    null,
    'verification must run on both cache hits and cache misses',
  );
  const verifyRun = normalizePowerShell(extractRunBlock(verify.text) ?? '');
  assert.match(verifyRun, /\$expectedBundleSize\s*=\s*6846809/);
  assert.match(
    verifyRun,
    /\$expectedBundleSha256\s*=\s*'5005b9d49fad0a4fb2c34eb60fbb25739d00d01651255258c2f408c7ee8dc7be'/,
  );
  assert.match(verifyRun, /Test-Path\s+-LiteralPath\s+\$bundlePath\s+-PathType\s+Leaf/);
  assert.match(verifyRun, /Get-Item\s+-LiteralPath\s+\$bundlePath/);
  assert.match(verifyRun, /\[int64\]\$bundle\.Length\s+-ne\s*\[int64\]\$expectedBundleSize/);
  assert.match(
    verifyRun,
    /Get-FileHash\s+-LiteralPath\s+\$bundle\.FullName\s+-Algorithm\s+SHA256/,
  );
  assert.match(verifyRun, /\.Hash\.ToLowerInvariant\(\)/);
  assert.match(verifyRun, /\$bundleSha256\s+-cne\s+\$expectedBundleSha256/);
  assert.match(verifyRun, /function\s+Assert-WorkspaceChild/);
  assert.match(verifyRun, /StartsWith\(\$workspacePrefix/);
  assert.match(
    verifyRun,
    /\$upstreamBundleDir\s*=\s*Assert-WorkspaceChild\s+\(Join-Path\s+\$workspaceRoot\s+'upstream-bundle'\)/,
  );
  assert.match(
    verifyRun,
    /Remove-Item\s+-LiteralPath\s+\$upstreamBundleDir\s+-Recurse\s+-Force/,
  );
  assert.match(
    verifyRun,
    /Expand-Archive\s+-LiteralPath\s+\$bundle\.FullName\s+-DestinationPath\s+\$upstreamBundleDir\s+-Force/,
  );

  const sizeIndex = verifyRun.indexOf('$expectedBundleSize');
  const hashIndex = verifyRun.indexOf('Get-FileHash');
  const cleanupIndex = verifyRun.indexOf('Remove-Item');
  const expandIndex = verifyRun.indexOf('Expand-Archive');
  assert.ok(
    sizeIndex >= 0 && sizeIndex < hashIndex && hashIndex < cleanupIndex && cleanupIndex < expandIndex,
    'ffmpeg ZIP must be verified before the guarded extraction directory is replaced',
  );

  const build = requireStep(windowsSteps, 'Build bundled legacy CLI');
  assert.ok(
    cache.start < fetch.start && fetch.start < verify.start && verify.start < build.start,
    'cache/download/verify-extract/build steps must remain in safety order',
  );
});

test('release metadata is validated before outputs and run scripts read expressions only through env', () => {
  for (const step of allSteps) {
    const run = extractStepRun(step.text);
    if (run) {
      assert.doesNotMatch(
        run,
        /\$\{\{/,
        `${step.name} must not interpolate GitHub expressions into PowerShell source`,
      );
    }
  }

  const checkout = requireStep(windowsSteps, 'Checkout');
  const metadata = requireStep(windowsSteps, 'Resolve release metadata');
  assert.deepEqual(
    trimmedBlockLines(requireBlock(metadata.text, /^        env:\s*$/, 8)),
    ['env:', 'RELEASE_TAG: ${{ github.ref_name }}'],
  );
  const metadataRun = extractRunBlock(metadata.text) ?? '';
  assert.ok(checkout.start < metadata.start, 'source SHA metadata must run after checkout');
  assert.match(metadataRun, /\$sourceShaOutput\s*=\s*@\(git\s+rev-parse\s+HEAD\)/);
  assertNativeCommandExitChecked(metadataRun, 'git rev-parse HEAD', 'Resolve release metadata');
  assert.match(metadataRun, /\$sourceShaOutput\.Count\s+-ne\s+1/);
  assert.match(metadataRun, /\$sourceSha\s*=\s*\[string\]\$sourceShaOutput\[0\]/);
  assert.match(metadataRun, /\$sourceSha\s+-cnotmatch\s+'\\A\[0-9A-Fa-f\]\{40\}\\z'/);
  assert.match(metadataRun, /"source_sha=\$sourceSha"\s*>>\s*\$env:GITHUB_OUTPUT/);
  assert.doesNotMatch(metadataRun, /\bnode\s+-p\b/);
  assert.match(
    metadataRun,
    /\$versionBase64Output\s*=\s*@\(node\s+-e\s+"[^"]+"\)/,
  );
  assert.match(metadataRun, /typeof\s+version\s*!==\s*'string'/);
  assert.match(metadataRun, /process\.stdout\.write\s*\(/);
  assert.match(metadataRun, /Buffer\.from\(version,\s*'utf8'\)\.toString\('base64'\)/);
  assert.match(metadataRun, /\$LASTEXITCODE\s+-eq\s+2/);
  assert.match(metadataRun, /throw\s+'Package version must be a string\.'/);
  assert.match(metadataRun, /throw\s+'Failed to read and Base64-encode the package version\.'/);
  assert.match(metadataRun, /\$versionBase64Output\.Count\s+-ne\s+1/);
  assert.match(metadataRun, /\[string\]::IsNullOrWhiteSpace\(\$versionBase64\)/);
  assert.match(
    metadataRun,
    /\[Convert\]::FromBase64String\(\$versionBase64\)/,
  );
  assert.match(
    metadataRun,
    /\[System\.Text\.UTF8Encoding\]::new\(\$false,\s*\$true\)/,
  );
  assert.match(metadataRun, /\$version\s*=\s*\$utf8\.GetString\(\$versionBytes\)/);
  assert.match(metadataRun, /throw\s+"Failed to decode package version Base64 as UTF-8:/);
  assert.doesNotMatch(metadataRun, /\.Trim\s*\(/);
  const versionPattern = metadataRun.match(
    /\$version\s+-cnotmatch\s+'(\\A[^\n]*\(rc\|beta\|alpha\)[^\n]*\\z)'/,
  )?.[1];
  assert.ok(versionPattern, 'metadata step must strictly validate the raw version line');
  assert.equal(
    versionPattern,
    String.raw`\A(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)(?:-(rc|beta|alpha)(?:\.(?:0|[1-9][0-9]*|[0-9A-Za-z-]*[A-Za-z-][0-9A-Za-z-]*))*)?\z`,
  );
  assert.doesNotMatch(versionPattern, /\\d/, 'version regex must use ASCII [0-9], not \\d');
  assertMetadataExtractionCases(metadataRun);
  assert.match(metadataRun, /\$env:RELEASE_TAG\s+-cne\s+\$expectedTag/);
  assert.match(metadataRun, /"version=\$version"\s*>>\s*\$env:GITHUB_OUTPUT/);
  assert.match(
    metadataRun,
    /"installer=m3u8-queue-downloader_\$\{version\}_x64-setup\.exe"\s*>>\s*\$env:GITHUB_OUTPUT/,
  );
  assert.match(
    metadataRun,
    /"portable=m3u8-queue-downloader_\$\{version\}_portable_x64\.zip"\s*>>\s*\$env:GITHUB_OUTPUT/,
  );
  assert.ok(
    metadataRun.indexOf('$version -cnotmatch') < metadataRun.indexOf('"version=$version"'),
    'version validation must happen before writing any release outputs',
  );

  for (const name of ['Prepare release assets', 'Verify local release assets']) {
    const step = requireStep(windowsSteps, name);
    assert.deepEqual(
      trimmedBlockLines(requireBlock(step.text, /^        env:\s*$/, 8)),
      [
        'env:',
        'INSTALLER_ASSET: ${{ steps.release.outputs.installer }}',
        'PORTABLE_ASSET: ${{ steps.release.outputs.portable }}',
      ],
      `${name} must receive asset names through step env`,
    );
  }

  const publishEnv = requireBlock(publishJob.text, /^    env:\s*$/, 4);
  assert.deepEqual(
    trimmedBlockLines(publishEnv),
    [
      'env:',
      'RELEASE_TAG: ${{ github.ref_name }}',
      'RELEASE_REPO: ${{ github.repository }}',
      'RELEASE_RUN_ID: ${{ github.run_id }}',
      'RELEASE_VERSION: ${{ needs.windows.outputs.version }}',
      'SOURCE_SHA: ${{ needs.windows.outputs.source_sha }}',
      'INSTALLER_ASSET: ${{ needs.windows.outputs.installer }}',
      'PORTABLE_ASSET: ${{ needs.windows.outputs.portable }}',
    ],
    'publish job env must contain only non-secret release metadata',
  );
  assert.doesNotMatch(publishEnv.text, /secrets\.|GH_TOKEN|GITHUB_TOKEN/);
});

test('GitHub token is step-scoped only where the step actually calls gh', () => {
  const policyTokenSteps = new Set([
    'Verify release tag source',
    'Reverify release tag source',
  ]);
  for (const job of [windowsJob, publishJob]) {
    const jobEnv = extractIndentedBlock(job.text, /^    env:\s*$/, 4);
    if (jobEnv) {
      assert.doesNotMatch(jobEnv.text, /GH_TOKEN|GITHUB_TOKEN|RELEASE_POLICY_TOKEN/);
    }
  }

  const tokenSteps = [];
  for (const step of allSteps) {
    const run = extractRunBlock(step.text) ?? '';
    const env = extractIndentedBlock(step.text, /^        env:\s*$/, 8);
    const tokenBindings = env?.text.match(
      /^ {10}GH_TOKEN:\s*\$\{\{ secrets\.(?:GITHUB_TOKEN|RELEASE_POLICY_TOKEN) \}\}\s*$/gm,
    ) ?? [];
    const callsGh = run.split('\n').some(isGhCommandLine);
    assert.equal(
      tokenBindings.length,
      callsGh ? 1 : 0,
      `${step.name} must have one step token exactly when its run block calls gh`,
    );
    if (tokenBindings.length === 1) {
      const expectedSecret = policyTokenSteps.has(step.name)
        ? 'RELEASE_POLICY_TOKEN'
        : 'GITHUB_TOKEN';
      assert.equal(
        tokenBindings[0].trim(),
        `GH_TOKEN: \${{ secrets.${expectedSecret} }}`,
        `${step.name} must use ${expectedSecret}`,
      );
      tokenSteps.push(step.name);
    }
    if (callsGh) assertNativeGhExitChecked(run, step.name);
  }

  assert.deepEqual(
    tokenSteps,
    [
      'Fetch ffmpeg from upstream release',
      'Download release assets',
      'Determine publication policy',
      'Verify release tag source',
      'Stage draft GitHub Release',
      'Verify draft release assets',
      'Reverify release tag source',
      'Publish verified GitHub Release',
      'Verify published GitHub Release',
    ],
    'only gh command steps may receive GH_TOKEN',
  );
  assert.equal((workflow.match(/^\s*GH_TOKEN:/gm) ?? []).length, tokenSteps.length);
  assert.equal((workflow.match(/^\s*GITHUB_TOKEN:/gm) ?? []).length, 0);
  assert.equal(
    (workflow.match(/secrets\.RELEASE_POLICY_TOKEN/g) ?? []).length,
    policyTokenSteps.size,
  );
});

test('release lifecycle never mutates an existing release and publishes only after verification', () => {
  assert.doesNotMatch(workflow, /\bgh\s+release\s+delete\b/i);
  assert.doesNotMatch(workflow, /--cleanup-tag\b/i);
  assert.doesNotMatch(workflow, /--clobber\b/i);
  assert.doesNotMatch(workflow, /\bportable\.zip\b/i);

  const stage = requireStep(publishSteps, 'Stage draft GitHub Release');
  const verifyDraft = requireStep(publishSteps, 'Verify draft release assets');
  const verifyTag = requireStep(publishSteps, 'Verify release tag source');
  const reverifyTag = requireStep(publishSteps, 'Reverify release tag source');
  const publish = requireStep(publishSteps, 'Publish verified GitHub Release');
  const verifyPublished = requireStep(publishSteps, 'Verify published GitHub Release');
  const normalizedRuns = allSteps.map((step) =>
    normalizePowerShell(extractStepRun(step.text) ?? ''));
  assert.equal(
    normalizedRuns.join('\n').match(/\bgh\s+release\s+create\b/g)?.length ?? 0,
    1,
    'workflow must contain exactly one gh release create command',
  );
  const createSteps = allSteps.filter((step) =>
    /\bgh\s+release\s+create\b/.test(normalizePowerShell(extractStepRun(step.text) ?? '')));
  assert.deepEqual(createSteps.map((step) => step.name), ['Stage draft GitHub Release']);

  const stageRun = normalizePowerShell(extractRunBlock(stage.text) ?? '');
  const createCommand = stageRun.split('\n').find((line) =>
    /\bgh\s+release\s+create\b/.test(line)) ?? '';
  for (const flag of ['--draft', '--verify-tag', '--generate-notes']) {
    assert.match(createCommand, new RegExp(`(?:^|\\s)${flag}(?=\\s|$)`));
  }
  assert.match(createCommand, /\$env:RELEASE_TAG\s+\$installerPath\s+\$portablePath/);
  assert.match(
    stageRun,
    /\$installerPath\s*=\s*Join-Path\s+'release-assets'\s+\$env:INSTALLER_ASSET/,
  );
  assert.match(
    stageRun,
    /\$portablePath\s*=\s*Join-Path\s+'release-assets'\s+\$env:PORTABLE_ASSET/,
  );
  assertHttpStatusHelper(stageRun, 'Stage draft GitHub Release');
  assert.match(stageRun, /\[Uri\]::EscapeDataString\(\$env:RELEASE_TAG\)/);
  assert.match(stageRun, /releases\/tags\/\$encodedTag/);
  assert.match(stageRun, /switch\s*\(\$existingResponse\.Status\)/);
  assert.match(
    stageRun,
    /['"]200['"]\s*\{[\s\S]*?gh\s+release\s+view\s+\$env:RELEASE_TAG[\s\S]*?--json\s+isDraft,isImmutable,url/,
  );
  assert.match(stageRun, /\$existingViewExit\s*=\s*\$LASTEXITCODE/);
  assert.match(stageRun, /\$existingRelease\s*=\s*\$existingJson\s*\|\s*ConvertFrom-Json/);
  assert.match(stageRun, /\$existingRelease\.url/);
  assert.match(stageRun, /\$existingRelease\.isImmutable/);

  const existingViewIndex = stageRun.indexOf('gh release view');
  const existingExitIndex = stageRun.indexOf('$existingViewExit = $LASTEXITCODE');
  const existingParseIndex = stageRun.indexOf('$existingRelease = $existingJson | ConvertFrom-Json');
  assert.ok(
    existingViewIndex >= 0 &&
      existingViewIndex < existingExitIndex &&
      existingExitIndex < existingParseIndex,
    'existing release view must be exit-checked before its JSON is parsed',
  );

  const draftBranch = /if\s*\(\[bool\]\$existingRelease\.isDraft\)\s*\{([\s\S]*?)\n\s*\}/
    .exec(stageRun)?.[1] ?? '';
  assert.match(draftBranch, /Automation will not modify/i);
  assert.match(draftBranch, /delete only the draft Release object/i);
  assert.match(draftBranch, /never clean up, delete, or move the tag/i);
  assert.match(draftBranch, /gh run rerun \$env:RELEASE_RUN_ID --failed/);
  assert.doesNotMatch(draftBranch, /higher version|new tag/i);

  const publishedGuidance = stageRun.slice(stageRun.indexOf('Published Release'));
  assert.match(publishedGuidance, /never delete or reuse/i);
  assert.match(publishedGuidance, /higher version/i);
  assert.match(publishedGuidance, /new tag/i);
  assert.doesNotMatch(publishedGuidance, /delete only the draft Release object/i);
  assert.match(stageRun, /['"]404['"]\s*\{\s*\}/);
  assert.match(stageRun, /Unable to parse GitHub API HTTP status/);
  assert.match(stageRun, /GitHub API returned HTTP \$status/);
  assert.ok(
    stageRun.indexOf('$existingResponse') < stageRun.indexOf('gh release create'),
    'existing release check must happen before create',
  );

  const editPattern = /\bgh\s+release\s+edit\b|@\(\s*'release'\s*,\s*'edit'\s*,[^\n]*'--draft=false'/g;
  assert.equal(
    normalizedRuns.join('\n').match(editPattern)?.length ?? 0,
    1,
    'workflow must contain exactly one public release edit definition',
  );
  const editDefinitions = allSteps.filter((step) => {
    const run = normalizePowerShell(extractStepRun(step.text) ?? '');
    return /\bgh\s+release\s+edit\b/.test(run) ||
      /@\(\s*'release'\s*,\s*'edit'\s*,[^\n]*'--draft=false'/.test(run);
  });
  assert.deepEqual(editDefinitions.map((step) => step.name), ['Publish verified GitHub Release']);
  assert.match(extractRunBlock(publish.text) ?? '', /&\s+gh\s+@publishArgs/);
  assert.ok(verifyTag.start < stage.start, 'tag source must be verified before create');
  assert.ok(
    verifyDraft.start < reverifyTag.start && reverifyTag.start < publish.start,
    'tag source must be reverified after draft verification and before edit',
  );
  assert.ok(publish.start < verifyPublished.start);
  assert.match(extractRunBlock(verifyDraft.text) ?? '', /if\s*\(-not\s+\$release\.isDraft\)/);
  const verifyPublishedRun = extractRunBlock(verifyPublished.text) ?? '';
  assert.match(verifyPublishedRun, /if\s*\(\$release\.isDraft\)/);
  assert.match(verifyPublishedRun, /if\s*\(-not\s+\[bool\]\$release\.isImmutable\)/);
  assert.match(
    verifyPublishedRun,
    /Repository Immutable Releases must be enabled before creating the tag/i,
  );

  assert.deepEqual(
    publishSteps.map((step) => step.name),
    [
      'Download release assets',
      'Verify local release assets',
      'Determine publication policy',
      'Verify release tag source',
      'Stage draft GitHub Release',
      'Verify draft release assets',
      'Reverify release tag source',
      'Publish verified GitHub Release',
      'Verify published GitHub Release',
    ],
    'publish job must download, verify, stage, verify, publish, then verify again',
  );
});

test('every native node command in pwsh blocks has an immediate exit-code guard', () => {
  const guardedCommands = [];
  for (const step of allSteps) {
    if (!/^ {8}shell:\s*pwsh\s*$/m.test(step.text)) continue;
    const run = extractRunBlock(step.text) ?? '';
    for (const command of assertNativeNodeExitChecked(run, step.name)) {
      guardedCommands.push({ step: step.name, command });
    }
  }

  assert.deepEqual(
    guardedCommands.map(({ step }) => step),
    [
      'Resolve release metadata',
      'Build bundled legacy CLI',
      'Build bundled legacy CLI',
    ],
    'metadata and both bundled-resource node commands must be guarded',
  );
  assert.match(guardedCommands[0].command, /node\s+-e\b/);
  assert.match(guardedCommands[1].command, /node\s+scripts\/stage-bundled-resources\.mjs\b/);
  assert.match(guardedCommands[2].command, /node\s+scripts\/validate-bundled-resources\.mjs\b/);

  const buildRun = extractRunBlock(
    requireStep(windowsSteps, 'Build bundled legacy CLI').text,
  ) ?? '';
  assert.match(buildRun, /throw\s+'Failed to stage bundled runtime resources\.'/);
  assert.match(buildRun, /throw\s+'Failed to validate bundled runtime resources\.'/);
});

test('release assets are transferred explicitly and verified by exact name, size, and digest', () => {
  const upload = requireStep(windowsSteps, 'Upload release assets');
  assert.match(
    upload.text,
    /^        uses:\s*actions\/upload-artifact@ea165f8d65b6e75b540449e92b4886f43607fa02\s+# v4\.6\.2\s*$/m,
  );
  assert.deepEqual(
    trimmedBlockLines(requireBlock(upload.text, /^        with:\s*$/, 8)),
    [
      'with:',
      'name: release-assets',
      'path: release-assets/*',
      'if-no-files-found: error',
      'retention-days: 1',
    ],
  );

  const download = requireStep(publishSteps, 'Download release assets');
  assert.match(
    normalizePowerShell(extractRunBlock(download.text) ?? ''),
    /gh run download \$env:RELEASE_RUN_ID --name release-assets --dir release-assets --repo \$env:RELEASE_REPO/,
  );

  for (const step of [
    requireStep(windowsSteps, 'Verify local release assets'),
    requireStep(publishSteps, 'Verify local release assets'),
  ]) {
    assertLocalAssetVerification(extractRunBlock(step.text) ?? '', step.name);
  }

  for (const step of [
    requireStep(publishSteps, 'Verify draft release assets'),
    requireStep(publishSteps, 'Verify published GitHub Release'),
  ]) {
    assertRemoteAssetVerification(extractRunBlock(step.text) ?? '', step.name);
  }

  const verifyDraftRun = extractRunBlock(
    requireStep(publishSteps, 'Verify draft release assets').text,
  ) ?? '';
  assert.match(verifyDraftRun, /--json\s+isDraft,isPrerelease,assets/);

  const verifyPublishedRun = extractRunBlock(
    requireStep(publishSteps, 'Verify published GitHub Release').text,
  ) ?? '';
  assert.match(
    verifyPublishedRun,
    /--json\s+isDraft,isPrerelease,isImmutable,assets/,
  );
});

test('publication policy serializes Latest decisions and handles ascending and descending versions', () => {
  const channel = requireStep(publishSteps, 'Determine publication policy');
  assert.match(channel.text, /^        id:\s*channel\s*$/m);
  const channelRun = extractRunBlock(channel.text) ?? '';
  assertHttpStatusHelper(channelRun, 'Determine publication policy');
  assert.match(channelRun, /repos\/\$env:RELEASE_REPO\/releases\/latest/);
  assert.match(channelRun, /\$isPrerelease\s*=\s*\$env:RELEASE_VERSION\s+-cmatch/);
  assert.match(channelRun, /\$markLatest\s*=\s*\$false/);
  assert.match(channelRun, /if\s*\(-not\s+\$isPrerelease\)/);
  assert.match(channelRun, /function\s+Compare-SemVerCore/);
  assert.match(channelRun, /\.Length\s+-gt\s+\$rightPart\.Length/);
  assert.match(channelRun, /\.Length\s+-lt\s+\$rightPart\.Length/);
  assert.match(channelRun, /\[string\]::CompareOrdinal\(\$leftPart,\s*\$rightPart\)/);
  assert.doesNotMatch(channelRun, /\[(?:u?int(?:32|64)?|bigint)\]/i);
  assert.match(channelRun, /\$comparison\s+-gt\s+0[\s\S]*?\$markLatest\s*=\s*\$true/);
  assert.match(channelRun, /\$comparison\s+-lt\s+0[\s\S]*?\$markLatest\s*=\s*\$false/);
  assert.match(channelRun, /\$comparison\s+-eq\s+0[\s\S]*?\$markLatest\s*=\s*\$false/);
  assert.match(channelRun, /\\Aapp-v\(\(\?:0\|\[1-9\]\[0-9\]\*\)/);
  assert.match(channelRun, /"mark_latest=\$\(\$markLatest\.ToString\(\)\.ToLowerInvariant\(\)\)"/);
  assert.match(channelRun, /"expected_latest_tag=\$expectedLatestTag"/);

  const publishRun = extractRunBlock(
    requireStep(publishSteps, 'Publish verified GitHub Release').text,
  ) ?? '';
  assert.match(
    publishRun,
    /\$isPrerelease\s*=\s*\$env:RELEASE_VERSION\s+-cmatch\s+'-\(\?:rc\|beta\|alpha\)\(\?:\\\.\|\$\)'/,
  );
  assert.match(publishRun, /if\s*\(\$isPrerelease\)\s*\{[\s\S]*?'--prerelease'[\s\S]*?'--latest=false'/);
  assert.match(publishRun, /elseif\s*\(\$env:MARK_LATEST\s+-ceq\s+'true'\)[\s\S]*?'--prerelease=false'[\s\S]*?'--latest'/);
  assert.match(publishRun, /elseif\s*\(\$env:MARK_LATEST\s+-ceq\s+'false'\)[\s\S]*?'--prerelease=false'[\s\S]*?'--latest=false'/);
  assert.deepEqual(
    trimmedBlockLines(requireBlock(
      requireStep(publishSteps, 'Publish verified GitHub Release').text,
      /^        env:\s*$/,
      8,
    )),
    ['env:', 'GH_TOKEN: ${{ secrets.GITHUB_TOKEN }}', 'MARK_LATEST: ${{ steps.channel.outputs.mark_latest }}'],
  );

  const verifyStep = requireStep(publishSteps, 'Verify published GitHub Release');
  const verifyRun = extractRunBlock(verifyStep.text) ?? '';
  assert.match(
    verifyRun,
    /\$expectedPrerelease\s*=\s*\$env:RELEASE_VERSION\s+-cmatch\s+'-\(\?:rc\|beta\|alpha\)\(\?:\\\.\|\$\)'/,
  );
  assert.match(verifyRun, /if\s*\(\[bool\]\$release\.isPrerelease\s+-ne\s+\$expectedPrerelease\)/);
  assertHttpStatusHelper(verifyRun, 'Verify published GitHub Release');
  assert.match(verifyRun, /if\s*\(\$env:MARK_LATEST\s+-ceq\s+'true'\)/);
  assert.match(verifyRun, /\$latestTag\s+-cne\s+\$env:RELEASE_TAG/);
  assert.match(verifyRun, /elseif\s*\(\$env:MARK_LATEST\s+-ceq\s+'false'\)/);
  assert.match(verifyRun, /\$latestTag\s+-cne\s+\$env:EXPECTED_LATEST_TAG/);
  assert.match(verifyRun, /\$latestResponse\.Status\s+-cne\s+'404'/);
  assert.deepEqual(
    trimmedBlockLines(requireBlock(verifyStep.text, /^        env:\s*$/, 8)),
    [
      'env:',
      'GH_TOKEN: ${{ secrets.GITHUB_TOKEN }}',
      'MARK_LATEST: ${{ steps.channel.outputs.mark_latest }}',
      'EXPECTED_LATEST_TAG: ${{ steps.channel.outputs.expected_latest_tag }}',
    ],
  );
});

test('release tag source is peeled to the build commit before create and before publish', () => {
  for (const name of ['Verify release tag source', 'Reverify release tag source']) {
    const run = extractRunBlock(requireStep(publishSteps, name).text) ?? '';
    assert.match(run, /\[Uri\]::EscapeDataString\(\$env:RELEASE_TAG\)/);
    assert.match(run, /git\/ref\/tags\/\$encodedTag/);
    assert.match(run, /git\/tags\/\$sha/);
    assert.match(run, /\$maxTagDepth\s*=\s*8/);
    assert.match(run, /HashSet\[string\]/);
    assert.match(run, /\.Add\(\$sha\)/);
    assert.match(run, /-ceq\s+'commit'/);
    assert.match(run, /-cne\s+'tag'/);
    assert.match(run, /cycle|too deep/i);
    assert.match(run, /\$tagCommit\s+-ine\s+\$env:SOURCE_SHA/);
    assert.match(run, /app-v\* update\/delete ruleset/i);
    assert.match(run, /\$apiExit\s*=\s*\$LASTEXITCODE/);
  }
});

test('release safety prerequisites are enforced before create and before publish', () => {
  const stage = requireStep(publishSteps, 'Stage draft GitHub Release');
  const publish = requireStep(publishSteps, 'Publish verified GitHub Release');

  for (const name of ['Verify release tag source', 'Reverify release tag source']) {
    const step = requireStep(publishSteps, name);
    const run = normalizePowerShell(extractRunBlock(step.text) ?? '');

    assert.match(
      run,
      /gh api --method GET -H 'X-GitHub-Api-Version: 2026-03-10' \$endpoint/,
    );
    assert.match(run, /\[string\]::IsNullOrWhiteSpace\(\$env:GH_TOKEN\)/);
    assert.match(run, /RELEASE_POLICY_TOKEN secret is not configured/);
    assert.match(run, /git\/ref\/heads\/master/);
    assert.match(
      run,
      /repos\/\$env:RELEASE_REPO\/compare\/\$tagCommit\.\.\.\$masterSha/,
    );
    assert.match(run, /\$comparison\.merge_base_commit\.sha\s+-ine\s+\$tagCommit/);
    assert.match(run, /repos\/\$env:RELEASE_REPO\/rulesets/);
    assert.match(run, /Protect app-v release tags/);
    assert.match(run, /\$matchingRulesets\.Count\s+-ne\s+1/);
    assert.match(run, /\$ruleset\.enforcement\s+-cne\s+'active'/);
    assert.match(run, /\$ruleset\.target\s+-cne\s+'tag'/);
    assert.match(run, /refs\/tags\/app-v\*/);
    assert.match(run, /\$exclude\.Count\s+-ne\s+0/);
    assert.match(run, /\$ruleTypes\s+-cne\s+'deletion,update'/);
    assert.match(run, /\$ruleset\.bypass_actors\.Count\s+-ne\s+0/);
    assert.match(run, /\$ruleset\.current_user_can_bypass\s+-cne\s+'never'/);
    assert.match(run, /repos\/\$env:RELEASE_REPO\/immutable-releases/);
    assert.match(run, /\$immutableReleases\.enabled\s+-ne\s+\$true/);

    if (name.startsWith('Reverify')) {
      assert.ok(step.start < publish.start, 'safety prerequisites must be reverified before publish');
    } else {
      assert.ok(step.start < stage.start, 'safety prerequisites must be verified before draft create');
    }
  }
});

function assertLocalAssetVerification(run, stepName) {
  assert.match(run, /\$env:INSTALLER_ASSET/);
  assert.match(run, /\$env:PORTABLE_ASSET/);
  assert.match(run, /\$files\.Count\s+-ne\s+2/);
  assert.match(run, /\$_\.Name\s+-ceq\s+\$name/);
  assert.match(run, /\$matches\.Count\s+-ne\s+1/);
  assert.match(run, /\$matches\[0\]\.Length\s+-le\s+0/);
  assert.match(run, /throw\s+"Missing, duplicate, or empty release asset:/);
  assert.ok(stepName);
}

function assertRemoteAssetVerification(run, stepName) {
  assert.match(run, /\$env:INSTALLER_ASSET/);
  assert.match(run, /\$env:PORTABLE_ASSET/);
  assert.match(run, /Get-Item\s+-LiteralPath/);
  assert.match(run, /Get-FileHash\s+-LiteralPath\s+\$path\s+-Algorithm\s+SHA256/);
  assert.match(run, /\.Hash\.ToLowerInvariant\(\)/);
  assert.match(run, /Digest\s*=\s*"sha256:\$hash"/);
  assert.match(run, /\.Length\s+-le\s+0/);
  assert.match(run, /\$assets\.Count\s+-ne\s+2/);
  assert.match(run, /\$_\.name\s+-ceq\s+\$expectedAsset\.Name/);
  assert.match(run, /\$matches\.Count\s+-ne\s+1/);
  assert.match(
    run,
    /\[int64\]\$matches\[0\]\.size\s+-ne\s+\[int64\]\$expectedAsset\.Length/,
    `${stepName} must compare remote size with the local release-assets file length`,
  );
  assert.match(run, /\$remoteDigest\s*=\s*\[string\]\$matches\[0\]\.digest/);
  assert.match(run, /\[string\]::IsNullOrWhiteSpace\(\$remoteDigest\)/);
  assert.match(run, /\$remoteDigest\s+-cne\s+\$expectedAsset\.Digest/);
  assert.match(run, /Remote release asset digest/i);
}

function assertNativeGhExitChecked(run, stepName) {
  const lines = normalizePowerShell(run).split('\n');
  for (let index = 0; index < lines.length; index += 1) {
    if (!isGhCommandLine(lines[index])) continue;
    let next = index + 1;
    while (next < lines.length && !lines[next].trim()) next += 1;
    assert.match(
      lines[next] ?? '',
      /\$LASTEXITCODE/,
      `${stepName} must capture or check LASTEXITCODE immediately after each gh command`,
    );
  }
}

function assertNativeCommandExitChecked(run, commandPattern, stepName) {
  const lines = normalizePowerShell(run).split('\n');
  const commandIndex = lines.findIndex((line) => line.includes(commandPattern));
  assert.ok(commandIndex >= 0, `${stepName} must run ${commandPattern}`);
  const guardIndex = nextSignificantLine(lines, commandIndex + 1);
  assert.match(
    lines[guardIndex] ?? '',
    /\$LASTEXITCODE/,
    `${stepName} must check LASTEXITCODE immediately after ${commandPattern}`,
  );
}

function assertHttpStatusHelper(run, stepName) {
  assert.match(run, /function\s+Invoke-GitHubApiStatus/);
  assert.match(run, /&\s+gh\s+api\s+-i\s+\$endpoint/);
  assert.match(run, /\$apiExit\s*=\s*\$LASTEXITCODE/);
  assert.match(run, /HTTP\/\\S\+\\s\+\(\[0-9\]\{3\}\)/);
  assert.match(run, /Unable to parse GitHub API HTTP status/);
  assert.match(run, /\$status\s+-ceq\s+'200'/);
  assert.match(run, /\$status\s+-ceq\s+'404'/);
  assert.match(run, /GitHub API returned HTTP \$status/);
  assert.match(run, /\$responseText/);
  const allowedStatuses = [...run.matchAll(/\$status\s+-ceq\s+'([0-9]{3})'/g)]
    .map((match) => match[1]);
  assert.deepEqual(
    [...new Set(allowedStatuses)],
    ['200', '404'],
    `${stepName} must allow only HTTP 200 and 404; auth, rate-limit, and 5xx fail`,
  );
}

function assertNativeNodeExitChecked(run, stepName) {
  const lines = normalizePowerShell(run).split('\n');
  const commands = [];
  for (let index = 0; index < lines.length; index += 1) {
    if (!isNodeCommandLine(lines[index])) continue;
    const guardIndex = nextSignificantLine(lines, index + 1);
    assert.match(
      lines[guardIndex] ?? '',
      /^\s*if\s*\(\s*\$LASTEXITCODE\s+-ne\s+0\s*\)\s*\{\s*$/,
      `${stepName} must check LASTEXITCODE immediately after ${lines[index].trim()}`,
    );
    const guardEnd = findClosingBrace(lines, guardIndex);
    const guardBody = lines.slice(guardIndex + 1, guardEnd).join('\n');
    assert.match(
      guardBody,
      /^\s*throw\s+(['"]).+\1\s*$/m,
      `${stepName} node exit-code guard must throw a clear error`,
    );
    commands.push(lines[index].trim());
  }
  return commands;
}

function isNodeCommandLine(line) {
  const source = line.trim();
  return /^(?:&\s+)?node(?:\.exe)?\s+/.test(source) ||
    /^\$[A-Za-z_][A-Za-z0-9_]*\s*=\s*@\(\s*(?:&\s+)?node(?:\.exe)?\s+/.test(source);
}

function nextSignificantLine(lines, start) {
  let index = start;
  while (index < lines.length) {
    const source = lines[index].trim();
    if (source && !source.startsWith('#')) return index;
    index += 1;
  }
  return lines.length;
}

function findClosingBrace(lines, start) {
  let depth = 0;
  for (let index = start; index < lines.length; index += 1) {
    for (const character of lines[index]) {
      if (character === '{') depth += 1;
      if (character === '}') depth -= 1;
    }
    if (index > start && depth === 0) return index;
  }
  return lines.length;
}

function assertMetadataExtractionCases(metadataRun) {
  const tempRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'release-workflow-metadata-'));
  const packageDir = path.join(tempRoot, 'm3u8-queue-downloader');
  const packagePath = path.join(packageDir, 'package.json');
  const outputPath = path.join(tempRoot, 'github-output.txt');
  fs.mkdirSync(packageDir, { recursive: true });
  fs.writeFileSync(packagePath, JSON.stringify({ version: '0.0.0' }), 'utf8');
  initializeProbeRepository(tempRoot);

  try {
    for (const version of [
      '1.2.3',
      '1.2.3-rc',
      '1.2.3-rc.1',
      '1.2.3-beta.0.exp.sha-5114f85',
      '1.2.3-alpha.001a',
    ]) {
      const result = runMetadataProbe(
        metadataRun, tempRoot, packagePath, outputPath, version, `app-v${version}`,
      );
      assert.equal(result.status, 0, metadataProbeFailure(version, result));
      const outputs = normalizeNewlines(fs.readFileSync(outputPath, 'utf8')).split('\n');
      assert.ok(outputs.includes(`version=${version}`));
    }

    for (const version of [
      '01.2.3',
      '1.02.3',
      '1.2.03',
      '1.2.3-preview',
      '1.2.3-preview.1',
      '1.2.3+build.1',
      '1.2.3-rc+build.1',
      '1.2.3-rc.01',
      '1.2.3-rc.',
      '1.2.3-rc..1',
      '1.2.3-..',
      '1.2.3\r',
      '1.2.3\n',
      ' 1.2.3',
      '1.2.3 ',
      '\uFF11.\uFF12.\uFF13',
    ]) {
      const result = runMetadataProbe(
        metadataRun, tempRoot, packagePath, outputPath, version, 'app-v0.0.0',
      );
      assert.notEqual(result.status, 0, `expected rejected metadata version: ${JSON.stringify(version)}`);
      assert.match(
        `${result.stderr}\n${result.stdout}`,
        /Unsupported package version format:/,
        metadataProbeFailure(version, result),
      );
    }

    const typeResult = runMetadataProbe(
      metadataRun, tempRoot, packagePath, outputPath, 123, 'app-v0.0.0',
    );
    assert.notEqual(typeResult.status, 0, 'non-string package version must fail');
    assert.match(
      `${typeResult.stderr}\n${typeResult.stdout}`,
      /Package version must be a string\./,
      metadataProbeFailure(123, typeResult),
    );
  } finally {
    fs.rmSync(tempRoot, { recursive: true, force: true });
  }
}

function initializeProbeRepository(cwd) {
  for (const args of [
    ['init', '--quiet'],
    ['config', 'user.name', 'release-workflow-test'],
    ['config', 'user.email', 'release-workflow-test@example.invalid'],
    ['add', '.'],
    ['commit', '--quiet', '-m', 'probe'],
  ]) {
    const result = spawnSync('git', args, { cwd, encoding: 'utf8', windowsHide: true });
    assert.equal(
      result.status,
      0,
      `failed to initialize metadata probe repository: git ${args.join(' ')}\n${result.stderr}`,
    );
  }
}

function runMetadataProbe(metadataRun, cwd, packagePath, outputPath, version, releaseTag) {
  fs.writeFileSync(packagePath, JSON.stringify({ version }), 'utf8');
  fs.rmSync(outputPath, { force: true });
  return spawnSync(
    'pwsh',
    ['-NoProfile', '-NonInteractive', '-Command', metadataRun],
    {
      cwd,
      encoding: 'utf8',
      env: {
        ...process.env,
        GITHUB_OUTPUT: outputPath,
        RELEASE_TAG: releaseTag,
      },
      windowsHide: true,
    },
  );
}

function metadataProbeFailure(version, result) {
  return `metadata probe failed for ${JSON.stringify(version)}:\n${result.stderr || result.stdout}`;
}

function isGhCommandLine(line) {
  return /(?:^\s*|=\s*&\s+|^\s*&\s+)gh(?:\s+(?:release|run|api)\b|\s+@publishArgs\b)/
    .test(line);
}

function normalizePowerShell(run) {
  return run.replace(/\x60[ \t]*\n[ \t]*/g, ' ');
}

function requireStep(steps, name) {
  const matches = steps.filter((step) => step.name === name);
  assert.equal(matches.length, 1, `expected exactly one ${name} step`);
  return matches[0];
}

function requireBlock(source, headerPattern, indent) {
  const block = extractIndentedBlock(source, headerPattern, indent);
  assert.ok(block, `expected block matching ${headerPattern}`);
  return block;
}

function significantLines(value) {
  return value.split('\n').filter((line) =>
    line.trim() && !line.trimStart().startsWith('#'));
}

function trimmedBlockLines(block) {
  return significantLines(block.text).map((line) => line.trim());
}

function extractChildKeys(source, indent) {
  const prefix = ' '.repeat(indent);
  const pattern = new RegExp(`^${prefix}([A-Za-z0-9_-]+):\\s*$`);
  return source.split('\n').flatMap((line) => {
    const match = pattern.exec(line);
    return match ? [match[1]] : [];
  });
}

function normalizeNewlines(value) {
  return value.replace(/\r\n?/g, '\n');
}

function extractMarkdownSection(source, heading) {
  const lines = source.split('\n');
  const starts = lines.flatMap((line, index) =>
    line === `## ${heading}` ? [index] : []);
  if (starts.length !== 1) return null;

  let end = starts[0] + 1;
  while (end < lines.length && !/^##\s+/.test(lines[end])) end += 1;
  return lines.slice(starts[0], end).join('\n');
}

function extractMarkdownLinks(source) {
  return [...source.matchAll(/\[[^\]]+\]\((https?:\/\/[^)]+)\)/g)]
    .map((match) => match[1]);
}

function extractInlineCode(source) {
  return [...source.matchAll(/`([^`]+)`/g)].map((match) => match[1]);
}

function extractIndentedBlock(source, headerPattern, indent) {
  const lines = source.split('\n');
  const starts = lines.flatMap((line, index) => headerPattern.test(line) ? [index] : []);
  if (starts.length !== 1) return null;

  let end = starts[0] + 1;
  while (end < lines.length) {
    const line = lines[end];
    const significant = line.trim() && !line.trimStart().startsWith('#');
    if (significant && /^ */.exec(line)[0].length <= indent) break;
    end += 1;
  }
  return { start: starts[0], text: lines.slice(starts[0], end).join('\n') };
}

function extractNamedSteps(stepsBlock) {
  const lines = stepsBlock.split('\n');
  const starts = lines.flatMap((line, index) => /^ {6}-\s+\S/.test(line) ? [index] : []);
  return starts.flatMap((start, index) => {
    const end = starts[index + 1] ?? lines.length;
    const match =
      /^ {6}- name:\s*(?:"([^"]+)"|'([^']+)'|(.+?))\s*$/.exec(lines[start]);
    return match ? [{
      name: (match[1] ?? match[2] ?? match[3]).trim(),
      start,
      text: lines.slice(start, end).join('\n'),
    }] : [];
  });
}

function extractRunBlock(stepText) {
  const run = /^ {8}run:\s*\|[+-]?\s*$/m.exec(stepText);
  return run ? stepText.slice(run.index + run[0].length) : null;
}

function extractStepRun(stepText) {
  const block = extractRunBlock(stepText);
  if (block !== null) return block;
  return /^ {8}run:\s*(\S.*)$/m.exec(stepText)?.[1] ?? null;
}
