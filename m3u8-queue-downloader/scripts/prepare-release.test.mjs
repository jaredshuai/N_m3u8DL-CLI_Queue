import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';
import { fileURLToPath, pathToFileURL } from 'node:url';

import {
  assertConsistentReleaseVersions,
  ArtifactReplacementTransaction,
  ArtifactValidator,
  GitHubArtifactDownloader,
  JsonVersionFiles,
  main,
  normalizeDownloadedPath,
  PackageRunValidator,
  PackageSyncCliAdapter,
  PackageSyncUseCase,
  replaceArtifactsDirectoryFromDownloadedFiles,
  replaceDirectoryContentsInPlace,
  resolveAllowedArtifactsDirectory,
  ReleasePrepareCliAdapter,
  ReleasePrepareReporter,
  ReleasePrepareUseCase,
  ReleaseReporter,
  validateDownloadedArtifactContents,
  validatePackageRun,
} from './prepare-release.mjs';

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const projectRoot = path.resolve(scriptDir, '..');
const repoRoot = path.resolve(projectRoot, '..');
const workspaceRoot = path.resolve(repoRoot, '..');
const defaultArtifactsDir = path.join(workspaceRoot, 'artifacts');
const prepareReleaseScript = path.join(scriptDir, 'prepare-release.mjs');
const prepareReleaseScriptUrl = pathToFileURL(prepareReleaseScript).href;

const context = {
  cwd: projectRoot,
  projectRoot,
  repoRoot,
  defaultArtifactsDir,
};

test('allows the default workspace artifacts directory', () => {
  assert.equal(
    resolveAllowedArtifactsDirectory(defaultArtifactsDir, context),
    defaultArtifactsDir,
  );
});

test('allows repo artifacts directories and their children', () => {
  assert.equal(
    resolveAllowedArtifactsDirectory('../artifacts', context),
    path.join(repoRoot, 'artifacts'),
  );
  assert.equal(
    resolveAllowedArtifactsDirectory('../artifacts/latest', context),
    path.join(repoRoot, 'artifacts', 'latest'),
  );
  assert.equal(
    resolveAllowedArtifactsDirectory('artifacts', context),
    path.join(projectRoot, 'artifacts'),
  );
});

test('rejects dangerous non-artifacts targets', () => {
  for (const destination of [
    '',
    ' ',
    '.',
    '..',
    projectRoot,
    repoRoot,
    path.parse(projectRoot).root,
  ]) {
    assert.throws(
      () => resolveAllowedArtifactsDirectory(destination, context),
      /Refusing to clear artifacts directory/,
      destination,
    );
  }
});

test('rejects repo paths that are not explicitly artifacts directories', () => {
  assert.throws(
    () => resolveAllowedArtifactsDirectory('../m3u8-queue-downloader/src', context),
    /Refusing to clear artifacts directory/,
  );
});

test('main dispatches package-sync through the CLI adapter', async () => {
  const calls = [];

  await main(
    ['node', prepareReleaseScript, 'package-sync', '--run-id', '123'],
    prepareReleaseScriptUrl,
    {
      packageSync(argv) {
        calls.push(['packageSync', argv]);
      },
      prepareRelease() {
        throw new Error('should not run release prepare for package-sync');
      },
      exit(code) {
        calls.push(['exit', code]);
      },
    },
  );

  assert.deepEqual(calls, [
    ['packageSync', ['--run-id', '123']],
    ['exit', 0],
  ]);
});

test('main dispatches version-check through the version guard', async () => {
  const calls = [];

  await main(
    ['node', prepareReleaseScript, 'version-check'],
    prepareReleaseScriptUrl,
    {
      packageSync() {
        throw new Error('should not run package-sync for version-check');
      },
      prepareRelease() {
        throw new Error('should not run release prepare for version-check');
      },
      versionCheck() {
        calls.push(['versionCheck']);
      },
      exit(code) {
        calls.push(['exit', code]);
      },
    },
  );

  assert.deepEqual(calls, [['versionCheck'], ['exit', 0]]);
});

test('main dispatches semver release preparation through the CLI adapter', async () => {
  const calls = [];

  await main(
    ['node', prepareReleaseScript, '0.2.0'],
    prepareReleaseScriptUrl,
    {
      packageSync() {
        throw new Error('should not run package-sync for release prepare');
      },
      prepareRelease(version) {
        calls.push(['prepareRelease', version]);
      },
      exit(code) {
        calls.push(['exit', code]);
      },
    },
  );

  assert.deepEqual(calls, [
    ['prepareRelease', '0.2.0'],
  ]);
});

test('ReleasePrepareCliAdapter rejects invalid semver before invoking the use case', () => {
  const calls = [];
  const adapter = new ReleasePrepareCliAdapter({
    reporter: {
      usage() {
        calls.push(['usage']);
      },
    },
    releasePrepareUseCase: {
      run() {
        throw new Error('should not prepare release for invalid semver');
      },
    },
    exit(code) {
      calls.push(['exit', code]);
    },
  });

  adapter.run('not-a-version');

  assert.deepEqual(calls, [
    ['usage'],
    ['exit', 1],
  ]);
});

test('ReleasePrepareUseCase updates version files and reports next steps', () => {
  const calls = [];
  const useCase = new ReleasePrepareUseCase({
    versionFiles: {
      updateVersion(version) {
        calls.push(['updateVersion', version]);
        return ['package.json', 'src-tauri/tauri.conf.json'];
      },
    },
    reporter: {
      versionFilesUpdated(files, version) {
        calls.push(['versionFilesUpdated', files, version]);
      },
      nextSteps(version) {
        calls.push(['nextSteps', version]);
      },
    },
  });

  useCase.run({ version: '0.2.0' });

  assert.deepEqual(calls, [
    ['updateVersion', '0.2.0'],
    ['versionFilesUpdated', ['package.json', 'src-tauri/tauri.conf.json'], '0.2.0'],
    ['nextSteps', '0.2.0'],
  ]);
});

test('JsonVersionFiles updates configured release version files', () => {
  const tempRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'release-prepare-'));
  const packageJson = path.join(tempRoot, 'package.json');
  const tauriConfig = path.join(tempRoot, 'src-tauri', 'tauri.conf.json');
  const cargoToml = path.join(tempRoot, 'src-tauri', 'Cargo.toml');
  const cargoLock = path.join(tempRoot, 'src-tauri', 'Cargo.lock');
  fs.mkdirSync(path.dirname(tauriConfig), { recursive: true });
  fs.writeFileSync(packageJson, `${JSON.stringify({ version: '0.1.0', name: 'app' })}\n`);
  fs.writeFileSync(tauriConfig, `${JSON.stringify({ version: '0.1.0', productName: 'app' })}\n`);
  fs.writeFileSync(
    cargoToml,
    '[package]\nname = "m3u8-queue-downloader"\nversion = "0.1.0"\n',
  );
  fs.writeFileSync(
    cargoLock,
    '[[package]]\nname = "dependency"\nversion = "9.9.9"\n\n' +
      '[[package]]\nname = "m3u8-queue-downloader"\nversion = "0.1.0"\n',
  );

  const versionFiles = new JsonVersionFiles({
    rootDir: tempRoot,
    files: [packageJson, tauriConfig, cargoToml, cargoLock],
  });
  const updatedFiles = versionFiles.updateVersion('0.2.0');

  assert.deepEqual(updatedFiles, [
    'package.json',
    path.join('src-tauri', 'tauri.conf.json'),
    path.join('src-tauri', 'Cargo.toml'),
    path.join('src-tauri', 'Cargo.lock'),
  ]);
  assert.equal(JSON.parse(fs.readFileSync(packageJson, 'utf8')).version, '0.2.0');
  assert.equal(JSON.parse(fs.readFileSync(tauriConfig, 'utf8')).version, '0.2.0');
  assert.match(fs.readFileSync(cargoToml, 'utf8'), /^version = "0\.2\.0"$/m);
  const lock = fs.readFileSync(cargoLock, 'utf8');
  assert.match(lock, /name = "m3u8-queue-downloader"\nversion = "0\.2\.0"/);
  assert.match(lock, /name = "dependency"\nversion = "9\.9\.9"/);
  assert.deepEqual(versionFiles.readVersions(), [
    { file: 'package.json', version: '0.2.0' },
    { file: path.join('src-tauri', 'tauri.conf.json'), version: '0.2.0' },
    { file: path.join('src-tauri', 'Cargo.toml'), version: '0.2.0' },
    { file: path.join('src-tauri', 'Cargo.lock'), version: '0.2.0' },
  ]);
  assert.doesNotThrow(() => {
    new JsonVersionFiles({
      rootDir: tempRoot,
      files: [packageJson, tauriConfig, cargoToml, cargoLock],
    }).updateVersion('0.2.0');
  });

  fs.rmSync(tempRoot, { recursive: true, force: true });
});

test('assertConsistentReleaseVersions reports version drift', () => {
  assert.throws(
    () =>
      assertConsistentReleaseVersions([
        { file: 'package.json', version: '0.2.0' },
        { file: path.join('src-tauri', 'Cargo.lock'), version: '0.1.0' },
      ]),
    /package\.json: 0\.2\.0[\s\S]*Cargo\.lock: 0\.1\.0/,
  );
});

test('ReleasePrepareReporter writes usage and release instructions', () => {
  const logs = [];
  const errors = [];
  const reporter = new ReleasePrepareReporter({
    log(message) {
      logs.push(message);
    },
    error(message) {
      errors.push(message);
    },
  });

  reporter.usage();
  reporter.versionFilesUpdated(['package.json'], '0.2.0');
  reporter.nextSteps('0.2.0');

  assert.deepEqual(errors, [
    'Usage: npm run release:prepare -- <semver>',
    'Example: npm run release:prepare -- 0.2.0',
  ]);
  assert.deepEqual(logs, [
    'updated package.json -> 0.2.0',
    '\nNext steps:',
    '  git commit -am "chore(release): v0.2.0"',
    '  git tag app-v0.2.0',
    '  git push origin master app-v0.2.0',
  ]);
});

test('PackageSyncCliAdapter parses argv and resolves artifacts before invoking use case', async () => {
  let request = null;
  const adapter = new PackageSyncCliAdapter({
    defaultArtifactsDir: 'artifacts/default',
    parsePackageArgs(argv) {
      assert.deepEqual(argv, ['--run-id', '123', '--artifacts-dir', 'artifacts/custom']);
      return {
        ...defaultPackageSyncOptions(),
        artifactsDir: 'artifacts/custom',
        runId: 123,
      };
    },
    resolveAllowedArtifactsDirectory(directory) {
      assert.equal(directory, 'artifacts/custom');
      return 'artifacts/resolved-custom';
    },
    packageSyncUseCase: {
      run(actualRequest) {
        request = actualRequest;
      },
    },
  });

  await adapter.run(['--run-id', '123', '--artifacts-dir', 'artifacts/custom']);

  assert.deepEqual(request, {
    options: {
      ...defaultPackageSyncOptions(),
      artifactsDir: 'artifacts/custom',
      runId: 123,
    },
    artifactsDir: 'artifacts/resolved-custom',
  });
});

test('PackageSyncCliAdapter resolves the default artifacts directory', async () => {
  let resolvedDirectory = null;
  const adapter = new PackageSyncCliAdapter({
    defaultArtifactsDir: 'artifacts/default',
    parsePackageArgs() {
      return defaultPackageSyncOptions();
    },
    resolveAllowedArtifactsDirectory(directory) {
      resolvedDirectory = directory;
      return 'artifacts/resolved-default';
    },
    packageSyncUseCase: {
      run() {},
    },
  });

  await adapter.run([]);

  assert.equal(resolvedDirectory, 'artifacts/default');
});

test('replaceArtifactsDirectoryFromDownloadedFiles leaves existing artifacts when download is empty', () => {
  const tempRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'artifact-sync-'));
  const testContext = buildTempArtifactsContext(tempRoot);
  const source = path.join(tempRoot, 'downloaded-empty');
  const destination = path.join(testContext.defaultArtifactsDir, 'latest');
  fs.mkdirSync(source, { recursive: true });
  fs.mkdirSync(destination, { recursive: true });
  fs.writeFileSync(path.join(destination, 'old.txt'), 'old package');

  assert.throws(
    () => replaceArtifactsDirectoryFromDownloadedFiles(source, destination, testContext),
    /Downloaded artifact did not contain any files/,
  );
  assert.equal(fs.readFileSync(path.join(destination, 'old.txt'), 'utf8'), 'old package');

  fs.rmSync(tempRoot, { recursive: true, force: true });
});

test('replaceArtifactsDirectoryFromDownloadedFiles swaps in downloaded files after validation', () => {
  const tempRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'artifact-sync-'));
  const testContext = buildTempArtifactsContext(tempRoot);
  const source = path.join(tempRoot, 'downloaded');
  const destination = path.join(testContext.defaultArtifactsDir, 'latest');
  writeValidDownloadedArtifact(source);
  fs.mkdirSync(destination, { recursive: true });
  fs.writeFileSync(path.join(destination, 'old.txt'), 'old package');

  const files = replaceArtifactsDirectoryFromDownloadedFiles(source, destination, testContext);

  assert.deepEqual(files, [
    path.join(destination, 'm3u8-queue-downloader-portable', 'lib', 'ffmpeg', 'tools', 'ffmpeg', 'bin', 'ffmpeg.exe'),
    path.join(destination, 'm3u8-queue-downloader-portable', 'm3u8-queue-downloader.exe'),
    path.join(destination, 'm3u8-queue-downloader-portable', 'resources', 'N_m3u8DL-CLI_v3.0.2.exe'),
    path.join(destination, 'm3u8-queue-downloader-portable', 'resources', 'ffmpeg.exe'),
    path.join(destination, 'm3u8-queue-downloader_0.1.0_x64-setup.exe'),
  ]);
  assert.equal(
    fs.readFileSync(
      path.join(destination, 'm3u8-queue-downloader-portable', 'm3u8-queue-downloader.exe'),
      'utf8',
    ),
    'portable exe',
  );
  assert.equal(fs.existsSync(path.join(destination, 'old.txt')), false);

  fs.rmSync(tempRoot, { recursive: true, force: true });
});

test('replaceArtifactsDirectoryFromDownloadedFiles validates through artifact validator boundary', () => {
  const tempRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'artifact-sync-'));
  const testContext = buildTempArtifactsContext(tempRoot);
  const source = path.join(tempRoot, 'downloaded');
  const destination = path.join(testContext.defaultArtifactsDir, 'latest');
  let validatedDirectory = null;
  writeValidDownloadedArtifact(source);

  replaceArtifactsDirectoryFromDownloadedFiles(source, destination, {
    ...testContext,
    artifactValidator: {
      validate(directory) {
        validatedDirectory = directory;
        validateDownloadedArtifactContents(directory);
      },
    },
  });

  assert(validatedDirectory, 'expected artifact validator to be called');
  assert(path.basename(validatedDirectory).startsWith('.latest-staging-'));

  fs.rmSync(tempRoot, { recursive: true, force: true });
});

test('replaceArtifactsDirectoryFromDownloadedFiles leaves existing artifacts when required files are missing', () => {
  const tempRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'artifact-sync-'));
  const testContext = buildTempArtifactsContext(tempRoot);
  const source = path.join(tempRoot, 'downloaded');
  const destination = path.join(testContext.defaultArtifactsDir, 'latest');
  fs.mkdirSync(source, { recursive: true });
  fs.writeFileSync(path.join(source, 'm3u8-queue-downloader_0.1.0_x64-setup.exe'), 'installer');
  fs.mkdirSync(destination, { recursive: true });
  fs.writeFileSync(path.join(destination, 'old.txt'), 'old package');

  assert.throws(
    () => replaceArtifactsDirectoryFromDownloadedFiles(source, destination, testContext),
    /Downloaded artifact is missing required files/,
  );
  assert.equal(fs.readFileSync(path.join(destination, 'old.txt'), 'utf8'), 'old package');

  fs.rmSync(tempRoot, { recursive: true, force: true });
});

test('replaceArtifactsDirectoryFromDownloadedFiles falls back when destination rename is blocked', () => {
  const tempRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'artifact-sync-'));
  const testContext = buildTempArtifactsContext(tempRoot);
  const source = path.join(tempRoot, 'downloaded');
  const destination = path.join(testContext.defaultArtifactsDir, 'latest');
  writeValidDownloadedArtifact(source);
  fs.mkdirSync(destination, { recursive: true });
  fs.writeFileSync(path.join(destination, 'old.txt'), 'old package');

  const files = replaceArtifactsDirectoryFromDownloadedFiles(source, destination, {
    ...testContext,
    renameSync(from, to) {
      if (path.resolve(from) === path.resolve(destination)) {
        const err = new Error(`blocked rename to ${to}`);
        err.code = 'EPERM';
        throw err;
      }
      fs.renameSync(from, to);
    },
  });

  assert(files.includes(path.join(destination, 'm3u8-queue-downloader-portable', 'm3u8-queue-downloader.exe')));
  assert.equal(fs.existsSync(path.join(destination, 'old.txt')), false);
  assert.equal(
    fs.readFileSync(
      path.join(destination, 'm3u8-queue-downloader-portable', 'm3u8-queue-downloader.exe'),
      'utf8',
    ),
    'portable exe',
  );

  fs.rmSync(tempRoot, { recursive: true, force: true });
});

test('replaceArtifactsDirectoryFromDownloadedFiles preserves backup when rollback rename fails', () => {
  const tempRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'artifact-sync-'));
  const testContext = buildTempArtifactsContext(tempRoot);
  const source = path.join(tempRoot, 'downloaded');
  const destination = path.join(testContext.defaultArtifactsDir, 'latest');
  let backupPath = null;
  writeValidDownloadedArtifact(source);
  fs.mkdirSync(destination, { recursive: true });
  fs.writeFileSync(path.join(destination, 'old.txt'), 'old package');

  assert.throws(
    () => replaceArtifactsDirectoryFromDownloadedFiles(source, destination, {
      ...testContext,
      renameSync(from, to) {
        if (path.resolve(from) === path.resolve(destination)) {
          backupPath = to;
          fs.renameSync(from, to);
          return;
        }

        if (path.resolve(to) === path.resolve(destination)) {
          throw new Error(path.resolve(from) === path.resolve(backupPath)
            ? 'rollback failed'
            : 'replace failed');
        }

        fs.renameSync(from, to);
      },
    }),
    /failed to restore backup/i,
  );

  assert(backupPath, 'expected backup path to be captured');
  assert.equal(fs.readFileSync(path.join(backupPath, 'old.txt'), 'utf8'), 'old package');
  assert.equal(fs.existsSync(destination), false);

  fs.rmSync(tempRoot, { recursive: true, force: true });
});

test('replaceArtifactsDirectoryFromDownloadedFiles preserves backup when destination reappears before rollback', () => {
  const tempRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'artifact-sync-'));
  const testContext = buildTempArtifactsContext(tempRoot);
  const source = path.join(tempRoot, 'downloaded');
  const destination = path.join(testContext.defaultArtifactsDir, 'latest');
  let backupPath = null;
  writeValidDownloadedArtifact(source);
  fs.mkdirSync(destination, { recursive: true });
  fs.writeFileSync(path.join(destination, 'old.txt'), 'old package');

  assert.throws(
    () => replaceArtifactsDirectoryFromDownloadedFiles(source, destination, {
      ...testContext,
      renameSync(from, to) {
        if (path.resolve(from) === path.resolve(destination)) {
          backupPath = to;
          fs.renameSync(from, to);
          return;
        }

        if (path.resolve(to) === path.resolve(destination)) {
          fs.mkdirSync(destination, { recursive: true });
          fs.writeFileSync(path.join(destination, 'reappeared.txt'), 'new occupant');
          throw new Error('replace failed after destination reappeared');
        }

        fs.renameSync(from, to);
      },
    }),
    /replace failed after destination reappeared/,
  );

  assert(backupPath, 'expected backup path to be captured');
  assert.equal(fs.readFileSync(path.join(backupPath, 'old.txt'), 'utf8'), 'old package');

  fs.rmSync(tempRoot, { recursive: true, force: true });
});

test('replaceArtifactsDirectoryFromDownloadedFiles reports preserved backup when destination reappears before rollback', () => {
  const tempRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'artifact-sync-'));
  const testContext = buildTempArtifactsContext(tempRoot);
  const source = path.join(tempRoot, 'downloaded');
  const destination = path.join(testContext.defaultArtifactsDir, 'latest');
  let backupPath = null;
  writeValidDownloadedArtifact(source);
  fs.mkdirSync(destination, { recursive: true });
  fs.writeFileSync(path.join(destination, 'old.txt'), 'old package');

  assert.throws(
    () => replaceArtifactsDirectoryFromDownloadedFiles(source, destination, {
      ...testContext,
      renameSync(from, to) {
        if (path.resolve(from) === path.resolve(destination)) {
          backupPath = to;
          fs.renameSync(from, to);
          return;
        }

        if (path.resolve(to) === path.resolve(destination)) {
          fs.mkdirSync(destination, { recursive: true });
          throw new Error('replace failed after destination reappeared');
        }

        fs.renameSync(from, to);
      },
    }),
    (err) =>
      err.message.includes('Backup was preserved at') &&
      backupPath &&
      err.message.includes(backupPath),
  );

  fs.rmSync(tempRoot, { recursive: true, force: true });
});

test('normalizeDownloadedPath rejects traversal after stripping artifact prefixes', () => {
  assert.throws(
    () => normalizeDownloadedPath('.portable-dist/../evil.exe'),
    /Refusing unsafe artifact path/,
  );
  assert.throws(
    () => normalizeDownloadedPath('src-tauri/target/release/bundle/nsis/../../evil.exe'),
    /Refusing unsafe artifact path/,
  );
});

test('validateDownloadedArtifactContents rejects zero-byte required files', () => {
  const tempRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'artifact-sync-'));
  const source = path.join(tempRoot, 'downloaded');
  writeValidDownloadedArtifact(source);
  fs.writeFileSync(
    path.join(source, 'm3u8-queue-downloader-portable', 'resources', 'ffmpeg.exe'),
    '',
  );

  assert.throws(
    () => validateDownloadedArtifactContents(source),
    /Downloaded artifact has empty required files: m3u8-queue-downloader-portable\/resources\/ffmpeg\.exe/,
  );

  fs.rmSync(tempRoot, { recursive: true, force: true });
});

test('ArtifactValidator delegates downloaded artifact validation', () => {
  const tempRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'artifact-sync-'));
  const source = path.join(tempRoot, 'downloaded');
  writeValidDownloadedArtifact(source);
  fs.writeFileSync(
    path.join(source, 'm3u8-queue-downloader-portable', 'resources', 'ffmpeg.exe'),
    '',
  );

  assert.throws(
    () => new ArtifactValidator().validate(source),
    /Downloaded artifact has empty required files: m3u8-queue-downloader-portable\/resources\/ffmpeg\.exe/,
  );

  fs.rmSync(tempRoot, { recursive: true, force: true });
});

test('ArtifactReplacementTransaction replaces downloaded artifacts with injected validator', () => {
  const tempRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'artifact-sync-'));
  const testContext = buildTempArtifactsContext(tempRoot);
  const source = path.join(tempRoot, 'downloaded');
  const destination = path.join(testContext.defaultArtifactsDir, 'latest');
  let validatedDirectory = null;
  writeValidDownloadedArtifact(source);

  const transaction = new ArtifactReplacementTransaction({
    ...testContext,
    artifactValidator: {
      validate(directory) {
        validatedDirectory = directory;
        validateDownloadedArtifactContents(directory);
      },
    },
  });

  const files = transaction.replaceFromDirectory(source, destination);

  assert(validatedDirectory, 'expected injected artifact validator to be called');
  assert(files.includes(path.join(destination, 'm3u8-queue-downloader_0.1.0_x64-setup.exe')));

  fs.rmSync(tempRoot, { recursive: true, force: true });
});

test('GitHubArtifactDownloader delegates replacement to artifact transaction', () => {
  const transaction = new ArtifactReplacementTransaction();
  let downloaded = null;
  const downloader = new GitHubArtifactDownloader({
    artifactReplacementTransaction: transaction,
    getRunArtifact(repo, runId) {
      assert.equal(repo, 'owner/repo');
      assert.equal(runId, 123);
      return { name: 'm3u8-queue-downloader-windows' };
    },
    downloadArtifact(repo, runId, artifactName, destination, actualTransaction) {
      downloaded = { repo, runId, artifactName, destination, actualTransaction };
      return ['artifact/setup.exe'];
    },
  });

  const result = downloader.downloadRunArtifact('owner/repo', 123, 'artifacts/latest');

  assert.deepEqual(result, {
    artifact: { name: 'm3u8-queue-downloader-windows' },
    downloadedFiles: ['artifact/setup.exe'],
  });
  assert.deepEqual(downloaded, {
    repo: 'owner/repo',
    runId: 123,
    artifactName: 'm3u8-queue-downloader-windows',
    destination: 'artifacts/latest',
    actualTransaction: transaction,
  });
});

test('ReleaseReporter writes package sync progress and artifact summary', () => {
  const logs = [];
  const reporter = new ReleaseReporter((message) => logs.push(message));

  reporter.workflowQueued({ url: 'https://example.test/run/queued' });
  reporter.artifactSynced({
    run: { url: 'https://example.test/run/123' },
    artifact: { name: 'm3u8-queue-downloader-windows' },
    artifactsDir: 'artifacts/latest',
    downloadedFiles: ['setup.exe', 'portable/app.exe'],
  });

  assert.deepEqual(logs, [
    'Workflow queued: https://example.test/run/queued',
    'Run URL: https://example.test/run/123',
    'Artifact: m3u8-queue-downloader-windows',
    'Artifacts directory: artifacts/latest',
    'Downloaded: setup.exe',
    'Downloaded: portable/app.exe',
  ]);
});

test('replaceArtifactsDirectoryFromDownloadedFiles rolls back failed in-place replacement', () => {
  const tempRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'artifact-sync-'));
  const testContext = buildTempArtifactsContext(tempRoot);
  const source = path.join(tempRoot, 'downloaded');
  const destination = path.join(testContext.defaultArtifactsDir, 'latest');
  let replacementFailed = false;
  writeValidDownloadedArtifact(source);
  fs.mkdirSync(destination, { recursive: true });
  fs.writeFileSync(path.join(destination, 'old.txt'), 'old package');

  assert.throws(
    () => replaceArtifactsDirectoryFromDownloadedFiles(source, destination, {
      ...testContext,
      renameSync(from, to) {
        if (path.resolve(from) === path.resolve(destination)) {
          const err = new Error(`blocked rename to ${to}`);
          err.code = 'EPERM';
          throw err;
        }
        fs.renameSync(from, to);
      },
      replaceDirectoryContentsInPlace(from, to) {
        if (!replacementFailed && path.resolve(to) === path.resolve(destination)) {
          replacementFailed = true;
          fs.writeFileSync(path.join(destination, 'partial.txt'), 'partial');
          throw new Error('copy failed');
        }
        replaceDirectoryContentsInPlace(from, to);
      },
    }),
    /copy failed/,
  );

  assert.equal(fs.readFileSync(path.join(destination, 'old.txt'), 'utf8'), 'old package');
  assert.equal(fs.existsSync(path.join(destination, 'partial.txt')), false);

  fs.rmSync(tempRoot, { recursive: true, force: true });
});

test('replaceDirectoryContentsInPlace removes stale files without replacing the root directory', () => {
  const tempRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'artifact-sync-'));
  const source = path.join(tempRoot, 'source');
  const destination = path.join(tempRoot, 'destination');
  fs.mkdirSync(path.join(source, 'new-dir'), { recursive: true });
  fs.mkdirSync(path.join(destination, 'old-dir'), { recursive: true });
  fs.writeFileSync(path.join(source, 'new-dir', 'new.txt'), 'new');
  fs.writeFileSync(path.join(destination, 'old-dir', 'old.txt'), 'old');

  replaceDirectoryContentsInPlace(source, destination);

  assert.equal(fs.readFileSync(path.join(destination, 'new-dir', 'new.txt'), 'utf8'), 'new');
  assert.equal(fs.existsSync(path.join(destination, 'old-dir')), false);

  fs.rmSync(tempRoot, { recursive: true, force: true });
});

test('validatePackageRun rejects a successful run from the wrong workflow', () => {
  assert.throws(
    () => validatePackageRun(
      {
        status: 'completed',
        conclusion: 'success',
        workflowPath: '.github/workflows/other.yml',
        headBranch: 'master',
        headSha: 'abc123',
      },
      {
        workflow: 'package_gui.yml',
        ref: 'master',
        sha: 'abc123',
      },
    ),
    /does not belong to workflow package_gui.yml/,
  );
});

test('validatePackageRun rejects a successful run from a stale sha', () => {
  assert.throws(
    () => validatePackageRun(
      {
        status: 'completed',
        conclusion: 'success',
        workflowPath: '.github/workflows/package_gui.yml',
        headBranch: 'master',
        headSha: 'old',
      },
      {
        workflow: 'package_gui.yml',
        ref: 'master',
        sha: 'new',
      },
    ),
    /does not match expected sha/,
  );
});

test('validatePackageRun accepts a successful run matching workflow ref and sha', () => {
  assert.doesNotThrow(() =>
    validatePackageRun(
      {
        status: 'completed',
        conclusion: 'success',
        workflowPath: '.github/workflows/package_gui.yml',
        headBranch: 'master',
        headSha: 'abc123',
      },
      {
        workflow: 'package_gui.yml',
        ref: 'master',
        sha: 'abc123',
      },
    ),
  );
});

test('PackageRunValidator delegates workflow run validation', () => {
  assert.throws(
    () => new PackageRunValidator().validate(
      {
        status: 'completed',
        conclusion: 'success',
        workflowPath: '.github/workflows/package_gui.yml',
        headBranch: 'master',
        headSha: 'old',
      },
      {
        workflow: 'package_gui.yml',
        ref: 'master',
        sha: 'new',
      },
    ),
    /does not match expected sha/,
  );
});

test('PackageSyncUseCase validates supplied run id before downloading artifacts', async () => {
  const logs = [];
  let validated = null;
  let downloaded = null;
  const run = {
    status: 'completed',
    conclusion: 'success',
    workflowPath: '.github/workflows/package_gui.yml',
    headBranch: 'master',
    headSha: 'abc123',
    url: 'https://example.test/run/123',
  };
  const deps = buildPackageSyncDependencies({
    workflowRunGateway: buildWorkflowRunGateway({
      currentHeadSha: () => 'abc123',
      getRunView(repo, runId) {
        assert.equal(repo, 'owner/repo');
        assert.equal(runId, 123);
        return run;
      },
    }),
    artifactDownloader: {
      downloadRunArtifact(repo, runId, destination) {
        assert.equal(repo, 'owner/repo');
        assert.equal(runId, 123);
        downloaded = { repo, runId, destination };
        return {
          artifact: { name: 'm3u8-queue-downloader-windows' },
          downloadedFiles: ['artifact/setup.exe'],
        };
      },
    },
    runValidator: {
      validate(actualRun, expected) {
        assert.equal(actualRun, run);
        validated = expected;
      },
    },
    releaseReporter: new ReleaseReporter((message) => logs.push(message)),
  });

  await new PackageSyncUseCase(deps).run({
    options: {
      ...defaultPackageSyncOptions(),
      ref: 'master',
      runId: 123,
    },
    artifactsDir: path.join(defaultArtifactsDir, 'resolved'),
  });

  assert.deepEqual(validated, {
    workflow: 'package_gui.yml',
    ref: 'master',
    sha: 'abc123',
    runId: 123,
  });
  assert.deepEqual(downloaded, {
    repo: 'owner/repo',
    runId: 123,
    destination: path.join(defaultArtifactsDir, 'resolved'),
  });
  assert(logs.includes('Run URL: https://example.test/run/123'));
  assert(logs.includes('Downloaded: artifact/setup.exe'));
});

test('PackageSyncUseCase can queue workflow and return without waiting', async () => {
  let runWorkflowArgs = null;
  let waitedForNewRun = false;
  const deps = buildPackageSyncDependencies({
    workflowRunGateway: buildWorkflowRunGateway({
      currentBranch: () => 'master',
      currentHeadSha: () => 'abc123',
      getWorkflowRuns: () => [{ databaseId: 1 }],
      runWorkflow(repo, workflow, ref, runTests) {
        runWorkflowArgs = { repo, workflow, ref, runTests };
      },
      waitForNewRun(repo, workflow, ref, beforeIds) {
        waitedForNewRun = true;
        assert.deepEqual([...beforeIds], ['1']);
        return {
          databaseId: 2,
          url: `https://example.test/${repo}/${workflow}/${ref}`,
        };
      },
      waitForRunCompletion() {
        throw new Error('should not wait for completion when --no-wait is set');
      },
    }),
    artifactDownloader: {
      downloadRunArtifact() {
        throw new Error('should not fetch artifacts when --no-wait is set');
      },
    },
    releaseReporter: new ReleaseReporter(() => {}),
  });

  await new PackageSyncUseCase(deps).run({
    options: {
      ...defaultPackageSyncOptions(),
      noWait: true,
    },
    artifactsDir: path.join(defaultArtifactsDir, 'resolved'),
  });

  assert.deepEqual(runWorkflowArgs, {
    repo: 'owner/repo',
    workflow: 'package_gui.yml',
    ref: 'master',
    runTests: true,
  });
  assert.equal(waitedForNewRun, true);
});

function buildTempArtifactsContext(tempRoot) {
  const projectRoot = path.join(tempRoot, 'repo', 'm3u8-queue-downloader');
  const repoRoot = path.dirname(projectRoot);
  const defaultArtifactsDir = path.join(tempRoot, 'artifacts');
  fs.mkdirSync(projectRoot, { recursive: true });
  return {
    cwd: projectRoot,
    projectRoot,
    repoRoot,
    defaultArtifactsDir,
  };
}

function defaultPackageSyncOptions() {
  return {
    repo: 'owner/repo',
    workflow: 'package_gui.yml',
    ref: null,
    skipTests: false,
    artifactsDir: null,
    runId: null,
    sha: null,
    pollSeconds: 15,
    timeoutMinutes: 45,
    noWait: false,
  };
}

function buildPackageSyncDependencies(overrides = {}) {
  return {
    workflowRunGateway: buildWorkflowRunGateway(),
    artifactDownloader: {
      downloadRunArtifact() {
        throw new Error('unexpected downloadRunArtifact call');
      },
    },
    runValidator: {
      validate() {
        throw new Error('unexpected runValidator call');
      },
    },
    releaseReporter: new ReleaseReporter(() => {}),
    ...overrides,
  };
}

function buildWorkflowRunGateway(overrides = {}) {
  return {
    ensureAvailable() {},
    currentBranch() {
      throw new Error('unexpected currentBranch call');
    },
    currentHeadSha() {
      throw new Error('unexpected currentHeadSha call');
    },
    getRunView() {
      throw new Error('unexpected getRunView call');
    },
    getWorkflowRuns() {
      throw new Error('unexpected getWorkflowRuns call');
    },
    runWorkflow() {
      throw new Error('unexpected runWorkflow call');
    },
    waitForNewRun() {
      throw new Error('unexpected waitForNewRun call');
    },
    waitForRunCompletion() {
      throw new Error('unexpected waitForRunCompletion call');
    },
    ...overrides,
  };
}

function writeValidDownloadedArtifact(source) {
  fs.mkdirSync(path.join(source, 'm3u8-queue-downloader-portable', 'resources'), {
    recursive: true,
  });
  fs.mkdirSync(
    path.join(
      source,
      'm3u8-queue-downloader-portable',
      'lib',
      'ffmpeg',
      'tools',
      'ffmpeg',
      'bin',
    ),
    { recursive: true },
  );
  fs.writeFileSync(
    path.join(source, 'm3u8-queue-downloader-portable', 'm3u8-queue-downloader.exe'),
    'portable exe',
  );
  fs.writeFileSync(
    path.join(source, 'm3u8-queue-downloader-portable', 'resources', 'N_m3u8DL-CLI_v3.0.2.exe'),
    'cli exe',
  );
  fs.writeFileSync(
    path.join(source, 'm3u8-queue-downloader-portable', 'resources', 'ffmpeg.exe'),
    'ffmpeg exe',
  );
  fs.writeFileSync(
    path.join(
      source,
      'm3u8-queue-downloader-portable',
      'lib',
      'ffmpeg',
      'tools',
      'ffmpeg',
      'bin',
      'ffmpeg.exe',
    ),
    'default ffmpeg exe',
  );
  fs.writeFileSync(path.join(source, 'm3u8-queue-downloader_0.1.0_x64-setup.exe'), 'installer');
}
