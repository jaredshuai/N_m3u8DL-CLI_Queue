#!/usr/bin/env node
import fs from 'node:fs';
import path from 'node:path';

const version = process.argv[2];

if (!version || !/^\d+\.\d+\.\d+(?:[-+][0-9A-Za-z.-]+)?$/.test(version)) {
  console.error('Usage: npm run release:prepare -- <semver>');
  console.error('Example: npm run release:prepare -- 0.2.0');
  process.exit(1);
}

const root = process.cwd();
const files = [
  path.join(root, 'package.json'),
  path.join(root, 'src-tauri', 'tauri.conf.json'),
];

for (const file of files) {
  const json = JSON.parse(fs.readFileSync(file, 'utf8'));
  json.version = version;
  fs.writeFileSync(file, `${JSON.stringify(json, null, 2)}\n`, 'utf8');
  console.log(`updated ${path.relative(root, file)} -> ${version}`);
}

console.log(`\nNext steps:`);
console.log(`  git commit -am "chore(release): v${version}"`);
console.log(`  git tag app-v${version}`);
console.log(`  git push origin master app-v${version}`);
