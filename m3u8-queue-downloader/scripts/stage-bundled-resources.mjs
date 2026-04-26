#!/usr/bin/env node
import fs from 'node:fs';
import path from 'node:path';
import process from 'node:process';
import { execFileSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

if (isMainModule()) {
  stageBundledResources();
}

export function stageBundledResources({
  root = process.cwd(),
  argv = process.argv.slice(2),
  env = process.env,
  pathCandidates = findFfmpegOnPath(),
} = {}) {
  const resourcesDir = path.join(root, 'src-tauri', 'resources');
  const repoRoot = path.resolve(root, '..');
  const workspaceRoot = path.resolve(repoRoot, '..');
  const args = parseArgs(argv);

  const cliSource = resolveRequiredFile('cli', [
    args.cli,
    env.BUNDLED_CLI_PATH,
    path.join(repoRoot, 'N_m3u8DL-CLI', 'bin', 'Release', 'N_m3u8DL-CLI.exe'),
    path.join(resourcesDir, 'N_m3u8DL-CLI_v3.0.2.exe'),
  ]);

  const ffmpegSource = resolveRequiredFfmpeg(
    buildFfmpegCandidates({
      args,
      env,
      resourcesDir,
      workspaceRoot,
      pathCandidates,
    }),
  );

  const optionalConfigSource = resolveOptionalFile([
    args.config,
    env.BUNDLED_CONFIG_PATH,
    path.join(resourcesDir, 'config.txt'),
  ]);

  fs.mkdirSync(resourcesDir, { recursive: true });
  copyResource(cliSource, path.join(resourcesDir, 'N_m3u8DL-CLI_v3.0.2.exe'));
  copyResource(ffmpegSource, path.join(resourcesDir, 'ffmpeg.exe'));

  if (optionalConfigSource) {
    copyResource(optionalConfigSource, path.join(resourcesDir, 'config.txt'));
  }

  console.log(`staged CLI: ${cliSource}`);
  console.log(`staged ffmpeg: ${ffmpegSource}`);
  if (optionalConfigSource) {
    console.log(`staged config: ${optionalConfigSource}`);
  } else {
    console.log('config.txt not provided; leaving it absent');
  }
}

function parseArgs(argv) {
  const parsed = {};
  for (let i = 0; i < argv.length; i += 1) {
    const current = argv[i];
    if (!current.startsWith('--')) continue;
    const key = current.slice(2);
    const value = argv[i + 1];
    if (!value || value.startsWith('--')) {
      parsed[key] = true;
      continue;
    }
    parsed[key] = value;
    i += 1;
  }
  return parsed;
}

export function buildFfmpegCandidates({
  args = {},
  env = {},
  resourcesDir,
  workspaceRoot,
  pathCandidates = [],
}) {
  return [
    path.join(resourcesDir, 'ffmpeg.exe'),
    path.join(workspaceRoot, 'ffmpeg.exe'),
    args.ffmpeg,
    env.BUNDLED_FFMPEG_PATH,
    ...pathCandidates,
  ];
}

function resolveRequiredFile(label, candidates) {
  const resolved = resolveOptionalFile(candidates);
  if (!resolved) {
    throw new Error(`Unable to locate required ${label} resource`);
  }
  return resolved;
}

function resolveOptionalFile(candidates) {
  for (const candidate of candidates) {
    if (!candidate || typeof candidate !== 'string') continue;
    const resolved = path.resolve(candidate);
    if (fs.existsSync(resolved) && fs.statSync(resolved).isFile()) {
      return resolved;
    }
  }
  return null;
}

export function resolveRequiredFfmpeg(candidates) {
  for (const candidate of candidates) {
    const resolved = resolveExistingFile(candidate);
    if (!resolved) continue;
    return resolveFfmpegBinary(resolved);
  }

  throw new Error('Unable to locate required ffmpeg resource');
}

function resolveExistingFile(candidate) {
  if (!candidate || typeof candidate !== 'string') return null;
  const resolved = path.resolve(candidate);
  if (fs.existsSync(resolved) && fs.statSync(resolved).isFile()) {
    return resolved;
  }
  return null;
}

function resolveFfmpegBinary(candidate) {
  const chocolateyBinary = resolveChocolateyFfmpegBinary(candidate);
  return chocolateyBinary ?? candidate;
}

function resolveChocolateyFfmpegBinary(candidate) {
  const normalized = candidate.toLowerCase();
  if (!normalized.endsWith(`${path.sep}chocolatey${path.sep}bin${path.sep}ffmpeg.exe`)) {
    return null;
  }

  const actual = path.resolve(
    path.dirname(candidate),
    '..',
    'lib',
    'ffmpeg',
    'tools',
    'ffmpeg',
    'bin',
    'ffmpeg.exe',
  );

  return resolveExistingFile(actual);
}

export function copyResource(source, destination) {
  if (path.resolve(source) === path.resolve(destination)) {
    return;
  }
  fs.copyFileSync(source, destination);
}

function findFfmpegOnPath() {
  if (process.platform !== 'win32') {
    return [];
  }

  try {
    const output = execFileSync('where.exe', ['ffmpeg'], {
      encoding: 'utf8',
      stdio: ['ignore', 'pipe', 'ignore'],
    });
    return output
      .split(/\r?\n/)
      .map((line) => line.trim())
      .filter(Boolean);
  } catch {
    return [];
  }
}

function isMainModule() {
  return process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url);
}
