import assert from 'node:assert/strict';
import path from 'node:path';
import test from 'node:test';
import { fileURLToPath } from 'node:url';

import { resolveAllowedArtifactsDirectory } from './prepare-release.mjs';

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
