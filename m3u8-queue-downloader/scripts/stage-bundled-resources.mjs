#!/usr/bin/env node
import fs from 'node:fs';
import path from 'node:path';
import process from 'node:process';
import { execFileSync } from 'node:child_process';

const root = process.cwd();
const resourcesDir = path.join(root, 'src-tauri', 'resources');
const repoRoot = path.resolve(root, '..');

const args = parseArgs(process.argv.slice(2));

const cliSource = resolveRequiredFile('cli', [
  args.cli,
  process.env.BUNDLED_CLI_PATH,
  path.join(repoRoot, 'N_m3u8DL-CLI', 'bin', 'Release', 'N_m3u8DL-CLI.exe'),
  path.join(resourcesDir, 'N_m3u8DL-CLI_v3.0.2.exe'),
]);

const ffmpegSource = resolveRequiredFile('ffmpeg', [
  args.ffmpeg,
  process.env.BUNDLED_FFMPEG_PATH,
  path.join(resourcesDir, 'ffmpeg.exe'),
  ...findFfmpegOnPath(),
]);

const optionalConfigSource = resolveOptionalFile([
  args.config,
  process.env.BUNDLED_CONFIG_PATH,
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

function copyResource(source, destination) {
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
