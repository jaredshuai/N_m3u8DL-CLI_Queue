import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';
import { fileURLToPath } from 'node:url';

import {
  replaceArtifactsDirectoryFromDownloadedFiles,
  resolveAllowedArtifactsDirectory,
} from './prepare-release.mjs';

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const projectRoot = path.resolve(scriptDir, '..');
const repoRoot = path.resolve(projectRoot, '..');
const workspaceRoot = path.resolve(repoRoot, '..');
const defaultArtifactsDir = path.join(workspaceRoot, 'artifacts');

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
  fs.mkdirSync(path.join(source, 'portable'), { recursive: true });
  fs.writeFileSync(path.join(source, 'portable', 'app.exe'), 'new package');
  fs.mkdirSync(destination, { recursive: true });
  fs.writeFileSync(path.join(destination, 'old.txt'), 'old package');

  const files = replaceArtifactsDirectoryFromDownloadedFiles(source, destination, testContext);

  assert.deepEqual(files, [path.join(destination, 'portable', 'app.exe')]);
  assert.equal(fs.readFileSync(path.join(destination, 'portable', 'app.exe'), 'utf8'), 'new package');
  assert.equal(fs.existsSync(path.join(destination, 'old.txt')), false);

  fs.rmSync(tempRoot, { recursive: true, force: true });
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
