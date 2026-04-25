#!/usr/bin/env node
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import process from 'node:process';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { execFileSync, spawnSync } from 'node:child_process';

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(scriptDir, '..');
const repoRoot = path.resolve(root, '..');
const workspaceRoot = path.resolve(repoRoot, '..');
const defaultArtifactsDir = path.join(workspaceRoot, 'artifacts');
const args = process.argv.slice(2);
const command = args[0];

if (isMainModule(import.meta.url, process.argv[1])) {
  if (command === 'package-sync') {
    await packageSync(args.slice(1));
    process.exit(0);
  }

  prepareRelease(command);
}

function prepareRelease(version) {
  if (!version || !/^\d+\.\d+\.\d+(?:[-+][0-9A-Za-z.-]+)?$/.test(version)) {
    console.error('Usage: npm run release:prepare -- <semver>');
    console.error('Example: npm run release:prepare -- 0.2.0');
    process.exit(1);
  }

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

  console.log('\nNext steps:');
  console.log(`  git commit -am "chore(release): v${version}"`);
  console.log(`  git tag app-v${version}`);
  console.log(`  git push origin master app-v${version}`);
}

async function packageSync(argv) {
  const options = parsePackageArgs(argv);
  const artifactsDir = options.artifactsDir
    ? resolveAllowedArtifactsDirectory(options.artifactsDir)
    : resolveAllowedArtifactsDirectory(defaultArtifactsDir);

  ensureGhInstalled();

  let runId = options.runId;
  let run = null;

  if (runId != null) {
    run = getRunView(options.repo, runId);
    if (run.status !== 'completed' || run.conclusion !== 'success') {
      throw new Error(`Run ${runId} is not a successful completed run`);
    }
  } else {
    const ref = options.ref ?? getCurrentGitBranch();
    const beforeIds = new Set(getWorkflowRuns(options.repo, options.workflow, ref).map((runItem) => String(runItem.databaseId)));

    runWorkflow(options.repo, options.workflow, ref, !options.skipTests);
    const queuedRun = waitForNewRun(options.repo, options.workflow, ref, beforeIds);
    console.log(`Workflow queued: ${queuedRun.url}`);

    if (options.noWait) {
      return;
    }

    runId = Number(queuedRun.databaseId);
    run = waitForRunCompletion(options.repo, runId, options.pollSeconds, options.timeoutMinutes);
    if (run.conclusion !== 'success') {
      throw new Error(`Workflow failed: ${run.url}`);
    }
  }

  const artifact = getRunArtifact(options.repo, runId);
  const downloadedFiles = downloadArtifactToDirectory(options.repo, runId, artifact.name, artifactsDir);

  console.log(`Run URL: ${run.url}`);
  console.log(`Artifact: ${artifact.name}`);
  console.log(`Artifacts directory: ${artifactsDir}`);
  for (const file of downloadedFiles) {
    console.log(`Downloaded: ${file}`);
  }
}

function parsePackageArgs(argv) {
  const options = {
    repo: 'jaredshuai/N_m3u8DL-CLI_Queue',
    workflow: 'package_gui.yml',
    ref: null,
    skipTests: false,
    artifactsDir: null,
    runId: null,
    pollSeconds: 15,
    timeoutMinutes: 45,
    noWait: false,
  };

  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    switch (arg) {
      case '--repo':
        options.repo = requireValue(argv, ++i, arg);
        break;
      case '--workflow':
        options.workflow = requireValue(argv, ++i, arg);
        break;
      case '--ref':
        options.ref = requireValue(argv, ++i, arg);
        break;
      case '--artifacts-dir':
        options.artifactsDir = requireValue(argv, ++i, arg);
        break;
      case '--run-id':
        options.runId = Number(requireValue(argv, ++i, arg));
        if (!Number.isInteger(options.runId) || options.runId <= 0) {
          throw new Error(`Invalid value for ${arg}: ${argv[i]}`);
        }
        break;
      case '--poll-seconds':
        options.pollSeconds = Number(requireValue(argv, ++i, arg));
        if (!Number.isInteger(options.pollSeconds) || options.pollSeconds <= 0) {
          throw new Error(`Invalid value for ${arg}: ${argv[i]}`);
        }
        break;
      case '--timeout-minutes':
        options.timeoutMinutes = Number(requireValue(argv, ++i, arg));
        if (!Number.isInteger(options.timeoutMinutes) || options.timeoutMinutes <= 0) {
          throw new Error(`Invalid value for ${arg}: ${argv[i]}`);
        }
        break;
      case '--skip-tests':
        options.skipTests = true;
        break;
      case '--no-wait':
        options.noWait = true;
        break;
      default:
        throw new Error(`Unknown argument: ${arg}`);
    }
  }

  return options;
}

function requireValue(argv, index, flag) {
  const value = argv[index];
  if (!value || value.startsWith('--')) {
    throw new Error(`Missing value for ${flag}`);
  }
  return value;
}

function ensureGhInstalled() {
  const result = spawnSync('gh', ['--version'], {
    encoding: 'utf8',
    stdio: 'ignore',
  });
  if (result.error || result.status !== 0) {
    throw new Error("GitHub CLI 'gh' is required but was not found in PATH");
  }
}

function runGh(args) {
  return execFileSync('gh', args, {
    encoding: 'utf8',
    cwd: repoRoot,
    stdio: ['ignore', 'pipe', 'inherit'],
  }).trim();
}

function runGhJson(args) {
  const output = runGh(args);
  return output ? JSON.parse(output) : null;
}

function getCurrentGitBranch() {
  return execFileSync('git', ['-C', repoRoot, 'branch', '--show-current'], {
    encoding: 'utf8',
    stdio: ['ignore', 'pipe', 'inherit'],
  }).trim();
}

function getWorkflowRuns(repo, workflow, branch) {
  return runGhJson([
    'run', 'list',
    '--repo', repo,
    '--workflow', workflow,
    '--branch', branch,
    '--limit', '20',
    '--json', 'databaseId,status,conclusion,url,createdAt,headBranch,displayTitle',
  ]) ?? [];
}

function runWorkflow(repo, workflow, ref, runTests) {
  runGh([
    'workflow', 'run', workflow,
    '--repo', repo,
    '--ref', ref,
    '-f', `run_tests=${runTests ? 'true' : 'false'}`,
  ]);
}

function waitForNewRun(repo, workflow, branch, beforeIds) {
  const deadline = Date.now() + 5 * 60 * 1000;
  while (Date.now() < deadline) {
    const runs = getWorkflowRuns(repo, workflow, branch);
    for (const run of runs) {
      if (!beforeIds.has(String(run.databaseId))) {
        return run;
      }
    }
    sleep(3000);
  }

  throw new Error('Timed out waiting for GitHub Actions run creation');
}

function getRunView(repo, runId) {
  return runGhJson([
    'run', 'view', String(runId),
    '--repo', repo,
    '--json', 'status,conclusion,url,headBranch,headSha',
  ]);
}

function waitForRunCompletion(repo, runId, pollSeconds, timeoutMinutes) {
  const deadline = Date.now() + timeoutMinutes * 60 * 1000;
  while (Date.now() < deadline) {
    const run = getRunView(repo, runId);
    if (run.status === 'completed') {
      return run;
    }
    sleep(pollSeconds * 1000);
  }

  throw new Error(`Timed out waiting for run ${runId} to complete`);
}

function getRunArtifact(repo, runId) {
  const response = runGhJson([
    'api', `repos/${repo}/actions/runs/${runId}/artifacts`,
  ]);
  const artifacts = (response?.artifacts ?? []).filter((artifact) => !artifact.expired);
  const installerArtifact = artifacts.find((artifact) =>
    /m3u8-queue-downloader-windows/i.test(artifact.name),
  );

  if (!installerArtifact) {
    const names = artifacts.map((artifact) => artifact.name).join(', ') || '(none)';
    throw new Error(
      `No installer artifact was found for run ${runId}. Available artifacts: ${names}`,
    );
  }

  return installerArtifact;
}

function downloadArtifactToDirectory(repo, runId, artifactName, destination) {
  clearArtifactsDirectory(destination);

  const tempDir = fs.mkdtempSync(path.join(os.tmpdir(), 'package-gui-'));
  try {
    execFileSync('gh', [
      'run', 'download', String(runId),
      '--repo', repo,
      '--name', artifactName,
      '--dir', tempDir,
    ], {
      cwd: repoRoot,
      stdio: 'inherit',
    });

    const files = listFilesRecursive(tempDir);
    if (files.length === 0) {
      throw new Error('Downloaded artifact did not contain any files');
    }

    for (const file of files) {
      const relativePath = normalizeDownloadedPath(path.relative(tempDir, file));
      const targetPath = path.join(destination, relativePath);
      fs.mkdirSync(path.dirname(targetPath), { recursive: true });
      fs.copyFileSync(file, targetPath);
    }

    return listFilesRecursive(destination);
  } finally {
    fs.rmSync(tempDir, { recursive: true, force: true });
  }
}

function clearArtifactsDirectory(destination) {
  const resolved = resolveAllowedArtifactsDirectory(destination);

  fs.rmSync(resolved, { recursive: true, force: true });
  fs.mkdirSync(resolved, { recursive: true });
}

export function resolveAllowedArtifactsDirectory(destination, context = {}) {
  const {
    cwd = process.cwd(),
    projectRoot = root,
    repoRoot: repositoryRoot = repoRoot,
    defaultArtifactsDir: defaultDirectory = defaultArtifactsDir,
  } = context;

  if (typeof destination !== 'string' || destination.trim() === '') {
    throw new Error('Refusing to clear artifacts directory: empty path');
  }

  const resolved = path.resolve(cwd, destination);
  const normalizedProjectRoot = path.resolve(projectRoot);
  const normalizedRepoRoot = path.resolve(repositoryRoot);
  const normalizedDefaultDirectory = path.resolve(defaultDirectory);

  if (
    isFilesystemRoot(resolved) ||
    isSamePath(resolved, normalizedRepoRoot) ||
    isSamePath(resolved, normalizedProjectRoot)
  ) {
    throw new Error(`Refusing to clear artifacts directory: ${resolved}`);
  }

  if (isSubpathOrSame(normalizedDefaultDirectory, resolved)) {
    return resolved;
  }

  if (
    isSubpathOrSame(normalizedRepoRoot, resolved) &&
    hasArtifactsPathSegment(path.relative(normalizedRepoRoot, resolved))
  ) {
    return resolved;
  }

  throw new Error(`Refusing to clear artifacts directory: ${resolved}`);
}

function isMainModule(moduleUrl, entryPath) {
  return Boolean(entryPath) && moduleUrl === pathToFileURL(path.resolve(entryPath)).href;
}

function isFilesystemRoot(directory) {
  return isSamePath(directory, path.parse(path.resolve(directory)).root);
}

function isSubpathOrSame(parent, child) {
  const relative = path.relative(normalizeForComparison(parent), normalizeForComparison(child));
  return relative === '' || (!relative.startsWith('..') && !path.isAbsolute(relative));
}

function isSamePath(left, right) {
  return normalizeForComparison(left) === normalizeForComparison(right);
}

function normalizeForComparison(directory) {
  const resolved = path.resolve(directory);
  return process.platform === 'win32' ? resolved.toLowerCase() : resolved;
}

function hasArtifactsPathSegment(relativePath) {
  return relativePath
    .split(path.sep)
    .some((segment) => segment === 'artifacts');
}

function normalizeDownloadedPath(relativePath) {
  const normalized = relativePath.split(path.sep).join('/');
  const prefixes = [
    '.portable-dist/',
    'src-tauri/target/release/bundle/nsis/',
  ];

  for (const prefix of prefixes) {
    if (normalized.startsWith(prefix)) {
      return normalized.slice(prefix.length).split('/').join(path.sep);
    }
  }

  return relativePath;
}

function listFilesRecursive(directory) {
  const entries = fs.readdirSync(directory, { withFileTypes: true });
  const files = [];

  for (const entry of entries) {
    const fullPath = path.join(directory, entry.name);
    if (entry.isDirectory()) {
      files.push(...listFilesRecursive(fullPath));
    } else if (entry.isFile()) {
      files.push(fullPath);
    }
  }

  return files.sort();
}

function sleep(ms) {
  Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, ms);
}
