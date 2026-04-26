#!/usr/bin/env node
import fs from 'node:fs';
import path from 'node:path';
import process from 'node:process';
import { execFileSync } from 'node:child_process';

const root = process.cwd();
const resourcesDir = path.join(root, 'src-tauri', 'resources');
const requiredFiles = [
  path.join('resources', 'N_m3u8DL-CLI_v3.0.2.exe'),
  path.join('resources', 'ffmpeg.exe'),
  path.join('lib', 'ffmpeg', 'tools', 'ffmpeg', 'bin', 'ffmpeg.exe'),
];

const missing = requiredFiles.filter((file) => {
  const fullPath = path.join(root, 'src-tauri', file);
  return !fs.existsSync(fullPath) || !fs.statSync(fullPath).isFile();
});

if (missing.length > 0) {
  console.error(`Missing bundled resources in ${path.join(root, 'src-tauri')}: ${missing.join(', ')}`);
  process.exit(1);
}

for (const file of requiredFiles) {
  const fullPath = path.join(root, 'src-tauri', file);
  const stats = fs.statSync(fullPath);
  console.log(`${file}: ${stats.size} bytes`);
}

try {
  const defaultFfmpegPath = path.join(
    root,
    'src-tauri',
    'lib',
    'ffmpeg',
    'tools',
    'ffmpeg',
    'bin',
    'ffmpeg.exe',
  );
  const output = execFileSync(defaultFfmpegPath, ['-version'], {
    cwd: path.dirname(defaultFfmpegPath),
    encoding: 'utf8',
    stdio: ['ignore', 'pipe', 'pipe'],
  });
  console.log(output.split(/\r?\n/)[0]);
} catch (err) {
  console.error('Bundled ffmpeg.exe is not executable from the CLI default ffmpeg path.');
  if (err.stderr) {
    console.error(String(err.stderr).trim());
  }
  process.exit(1);
}
