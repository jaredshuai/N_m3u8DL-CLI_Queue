import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';

import {
  buildFfmpegCandidates,
  copyResource,
  DEFAULT_FFMPEG_RELATIVE_PATH,
  resolveRequiredFfmpeg,
  stageBundledResources,
} from './stage-bundled-resources.mjs';

test('buildFfmpegCandidates prefers the bundled original before CI-provided ffmpeg', () => {
  const tempRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'stage-resources-'));
  const resourcesDir = path.join(tempRoot, 'src-tauri', 'resources');
  const workspaceRoot = path.join(tempRoot, 'workspace');
  const bundled = path.join(resourcesDir, 'ffmpeg.exe');
  const ciFfmpeg = path.join(tempRoot, 'chocolatey', 'bin', 'ffmpeg.exe');

  fs.mkdirSync(resourcesDir, { recursive: true });
  fs.mkdirSync(path.dirname(ciFfmpeg), { recursive: true });
  fs.writeFileSync(bundled, 'original ffmpeg');
  fs.writeFileSync(ciFfmpeg, 'ci ffmpeg');

  const selected = resolveRequiredFfmpeg(
    buildFfmpegCandidates({
      args: { ffmpeg: ciFfmpeg },
      env: {},
      resourcesDir,
      workspaceRoot,
      pathCandidates: [],
    }),
  );

  assert.equal(selected, bundled);

  fs.rmSync(tempRoot, { recursive: true, force: true });
});

test('copyResource skips when source and destination are the same file', () => {
  const tempRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'stage-resources-'));
  const source = path.join(tempRoot, 'ffmpeg.exe');
  fs.writeFileSync(source, 'original ffmpeg');

  copyResource(source, source);

  assert.equal(fs.readFileSync(source, 'utf8'), 'original ffmpeg');

  fs.rmSync(tempRoot, { recursive: true, force: true });
});

test('stageBundledResources writes ffmpeg to the CLI default relative path', () => {
  const tempRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'stage-resources-'));
  const cliSource = path.join(tempRoot, 'N_m3u8DL-CLI.exe');
  const ffmpegSource = path.join(tempRoot, 'ffmpeg.exe');
  fs.writeFileSync(cliSource, 'cli');
  fs.writeFileSync(ffmpegSource, 'ffmpeg');

  stageBundledResources({
    root: tempRoot,
    argv: ['--cli', cliSource, '--ffmpeg', ffmpegSource],
    env: {},
    pathCandidates: [],
  });

  assert.equal(
    fs.readFileSync(
      path.join(tempRoot, 'src-tauri', DEFAULT_FFMPEG_RELATIVE_PATH),
      'utf8',
    ),
    'ffmpeg',
  );

  fs.rmSync(tempRoot, { recursive: true, force: true });
});
