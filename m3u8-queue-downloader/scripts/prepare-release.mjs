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

function prepareRelease(version) {
  return new ReleasePrepareCliAdapter().run(version);
}

async function packageSync(argv) {
  return new PackageSyncCliAdapter().run(argv);
}

export async function main(argv = process.argv, moduleUrl = import.meta.url, context = {}) {
  if (!isMainModule(moduleUrl, argv[1])) {
    return;
  }

  const args = argv.slice(2);
  const command = args[0];
  const runPackageSync = context.packageSync ?? packageSync;
  const runPrepareRelease = context.prepareRelease ?? prepareRelease;
  const exit = context.exit ?? process.exit;

  if (command === 'package-sync') {
    await runPackageSync(args.slice(1));
    exit(0);
    return;
  }

  runPrepareRelease(command);
}

export class ReleasePrepareCliAdapter {
  constructor(dependencies = defaultReleasePrepareCliDependencies()) {
    this.dependencies = dependencies;
  }

  run(version) {
    if (!isValidReleaseVersion(version)) {
      this.dependencies.reporter.usage();
      this.dependencies.exit(1);
      return;
    }

    this.dependencies.releasePrepareUseCase.run({ version });
  }
}

export class ReleasePrepareUseCase {
  constructor(dependencies = defaultReleasePrepareUseCaseDependencies()) {
    this.dependencies = dependencies;
  }

  run({ version }) {
    const updatedFiles = this.dependencies.versionFiles.updateVersion(version);
    this.dependencies.reporter.versionFilesUpdated(updatedFiles, version);
    this.dependencies.reporter.nextSteps(version);
  }
}

export class JsonVersionFiles {
  constructor(options = {}) {
    this.rootDir = options.rootDir ?? root;
    this.files = options.files ?? defaultReleaseVersionFiles();
  }

  updateVersion(version) {
    const updatedFiles = [];
    for (const file of this.files) {
      if (file.endsWith('.toml')) {
        updateTomlVersion(file, version);
      } else {
        updateJsonVersion(file, version);
      }
      updatedFiles.push(path.relative(this.rootDir, file));
    }
    return updatedFiles;
  }
}

export class ReleasePrepareReporter {
  constructor(output = {}) {
    this.log = output.log ?? console.log;
    this.error = output.error ?? console.error;
  }

  usage() {
    this.error('Usage: npm run release:prepare -- <semver>');
    this.error('Example: npm run release:prepare -- 0.2.0');
  }

  versionFilesUpdated(files, version) {
    for (const file of files) {
      this.log(`updated ${file} -> ${version}`);
    }
  }

  nextSteps(version) {
    this.log('\nNext steps:');
    this.log(`  git commit -am "chore(release): v${version}"`);
    this.log(`  git tag app-v${version}`);
    this.log(`  git push origin master app-v${version}`);
  }
}

export class PackageSyncCliAdapter {
  constructor(dependencies = defaultPackageSyncCliDependencies()) {
    this.dependencies = dependencies;
  }

  async run(argv) {
    const options = this.dependencies.parsePackageArgs(argv);
    const artifactsDir = options.artifactsDir
      ? this.dependencies.resolveAllowedArtifactsDirectory(options.artifactsDir)
      : this.dependencies.resolveAllowedArtifactsDirectory(this.dependencies.defaultArtifactsDir);

    return this.dependencies.packageSyncUseCase.run({
      options,
      artifactsDir,
    });
  }
}

export class PackageSyncUseCase {
  constructor(dependencies = defaultPackageSyncDependencies()) {
    this.dependencies = dependencies;
  }

  async run(request) {
    const { options, artifactsDir } = request;
    const workflowRunGateway = this.dependencies.workflowRunGateway;
    const artifactDownloader = this.dependencies.artifactDownloader;
    const releaseReporter = this.dependencies.releaseReporter;
    const runValidator = this.dependencies.runValidator;

    workflowRunGateway.ensureAvailable();

    let runId = options.runId;
    let run = null;
    const expectedRef = options.ref ?? workflowRunGateway.currentBranch();
    const expectedSha = options.sha ?? workflowRunGateway.currentHeadSha();

    if (runId != null) {
      run = workflowRunGateway.getRunView(options.repo, runId);
      runValidator.validate(run, {
        workflow: options.workflow,
        ref: expectedRef,
        sha: expectedSha,
        runId,
      });
    } else {
      const ref = expectedRef;
      const beforeIds = new Set(
        workflowRunGateway
          .getWorkflowRuns(options.repo, options.workflow, ref)
          .map((runItem) => String(runItem.databaseId)),
      );

      workflowRunGateway.runWorkflow(options.repo, options.workflow, ref, !options.skipTests);
      const queuedRun = workflowRunGateway.waitForNewRun(
        options.repo,
        options.workflow,
        ref,
        beforeIds,
      );
      releaseReporter.workflowQueued(queuedRun);

      if (options.noWait) {
        return;
      }

      runId = Number(queuedRun.databaseId);
      run = workflowRunGateway.waitForRunCompletion(
        options.repo,
        runId,
        options.pollSeconds,
        options.timeoutMinutes,
      );
      runValidator.validate(run, {
        workflow: options.workflow,
        ref,
        sha: expectedSha,
        runId,
      });
    }

    const { artifact, downloadedFiles } = artifactDownloader.downloadRunArtifact(
      options.repo,
      runId,
      artifactsDir,
    );

    releaseReporter.artifactSynced({
      run,
      artifact,
      artifactsDir,
      downloadedFiles,
    });
  }
}

function defaultReleasePrepareCliDependencies() {
  const reporter = new ReleasePrepareReporter();
  return {
    reporter,
    releasePrepareUseCase: new ReleasePrepareUseCase({
      reporter,
      versionFiles: new JsonVersionFiles(),
    }),
    exit: process.exit,
  };
}

function defaultReleasePrepareUseCaseDependencies() {
  return {
    reporter: new ReleasePrepareReporter(),
    versionFiles: new JsonVersionFiles(),
  };
}

function defaultReleaseVersionFiles() {
  return [
    path.join(root, 'package.json'),
    path.join(root, 'src-tauri', 'tauri.conf.json'),
    path.join(root, 'src-tauri', 'Cargo.toml'),
  ];
}

function updateJsonVersion(file, version) {
  const json = JSON.parse(fs.readFileSync(file, 'utf8'));
  json.version = version;
  fs.writeFileSync(file, `${JSON.stringify(json, null, 2)}\n`, 'utf8');
}

function updateTomlVersion(file, version) {
  const content = fs.readFileSync(file, 'utf8');
  const updated = content.replace(/^version = ".*"/m, `version = "${version}"`);
  fs.writeFileSync(file, updated, 'utf8');
}

function defaultPackageSyncCliDependencies() {
  return {
    defaultArtifactsDir,
    parsePackageArgs,
    resolveAllowedArtifactsDirectory,
    packageSyncUseCase: new PackageSyncUseCase(),
  };
}

function defaultPackageSyncDependencies() {
  const artifactValidator = new ArtifactValidator();
  const artifactReplacementTransaction = new ArtifactReplacementTransaction({
    artifactValidator,
  });

  return {
    runValidator: new PackageRunValidator(),
    workflowRunGateway: new GitHubWorkflowRunGateway(),
    artifactDownloader: new GitHubArtifactDownloader({
      artifactReplacementTransaction,
    }),
    releaseReporter: new ReleaseReporter(console.log),
  };
}

export class PackageRunValidator {
  validate(run, expected) {
    validatePackageRun(run, expected);
  }
}

export class GitHubWorkflowRunGateway {
  ensureAvailable() {
    ensureGhInstalled();
  }

  currentBranch() {
    return getCurrentGitBranch();
  }

  currentHeadSha() {
    return getCurrentGitHeadSha();
  }

  getWorkflowRuns(repo, workflow, branch) {
    return getWorkflowRuns(repo, workflow, branch);
  }

  runWorkflow(repo, workflow, ref, runTests) {
    runWorkflow(repo, workflow, ref, runTests);
  }

  waitForNewRun(repo, workflow, branch, beforeIds) {
    return waitForNewRun(repo, workflow, branch, beforeIds);
  }

  getRunView(repo, runId) {
    return getRunView(repo, runId);
  }

  waitForRunCompletion(repo, runId, pollSeconds, timeoutMinutes) {
    return waitForRunCompletion(repo, runId, pollSeconds, timeoutMinutes);
  }
}

export class GitHubArtifactDownloader {
  constructor(dependencies = {}) {
    this.dependencies = {
      artifactReplacementTransaction:
        dependencies.artifactReplacementTransaction ?? new ArtifactReplacementTransaction(),
      getRunArtifact: dependencies.getRunArtifact ?? getRunArtifact,
      downloadArtifact: dependencies.downloadArtifact ?? downloadArtifactToDirectory,
    };
  }

  downloadRunArtifact(repo, runId, destination) {
    const artifact = this.dependencies.getRunArtifact(repo, runId);
    const downloadedFiles = this.dependencies.downloadArtifact(
      repo,
      runId,
      artifact.name,
      destination,
      this.dependencies.artifactReplacementTransaction,
    );
    return { artifact, downloadedFiles };
  }
}

export class ArtifactValidator {
  validate(directory) {
    validateDownloadedArtifactContents(directory);
  }
}

export class ArtifactReplacementTransaction {
  constructor(context = {}) {
    this.context = context;
  }

  replaceFromDirectory(source, destination) {
    return replaceArtifactsDirectoryFromDownloadedFiles(source, destination, this.context);
  }
}

export class ReleaseReporter {
  constructor(log = console.log) {
    this.log = log;
  }

  workflowQueued(run) {
    this.log(`Workflow queued: ${run.url}`);
  }

  artifactSynced({ run, artifact, artifactsDir, downloadedFiles }) {
    this.log(`Run URL: ${run.url}`);
    this.log(`Artifact: ${artifact.name}`);
    this.log(`Artifacts directory: ${artifactsDir}`);
    for (const file of downloadedFiles) {
      this.log(`Downloaded: ${file}`);
    }
  }
}

function isValidReleaseVersion(version) {
  return Boolean(version) && /^\d+\.\d+\.\d+(?:[-+][0-9A-Za-z.-]+)?$/.test(version);
}

function parsePackageArgs(argv) {
  const options = {
    repo: 'jaredshuai/N_m3u8DL-CLI_Queue',
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
      case '--sha':
        options.sha = requireValue(argv, ++i, arg);
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

function getCurrentGitHeadSha() {
  return execFileSync('git', ['-C', repoRoot, 'rev-parse', 'HEAD'], {
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
  const run = runGhJson(['api', `repos/${repo}/actions/runs/${runId}`]);
  return {
    status: run.status,
    conclusion: run.conclusion,
    url: run.html_url,
    headBranch: run.head_branch,
    headSha: run.head_sha,
    workflowName: run.name,
    workflowPath: run.path,
  };
}

export function validatePackageRun(run, expected) {
  const runLabel = expected.runId ? `Run ${expected.runId}` : 'Run';
  if (run.status !== 'completed' || run.conclusion !== 'success') {
    throw new Error(`${runLabel} is not a successful completed run`);
  }

  if (!workflowMatches(run, expected.workflow)) {
    throw new Error(
      `${runLabel} does not belong to workflow ${expected.workflow}: ${run.workflowPath ?? run.workflowName ?? '(unknown)'}`,
    );
  }

  if (expected.ref && run.headBranch !== expected.ref) {
    throw new Error(
      `${runLabel} does not match expected ref ${expected.ref}: ${run.headBranch ?? '(unknown)'}`,
    );
  }

  if (
    expected.sha &&
    normalizeSha(run.headSha) !== normalizeSha(expected.sha)
  ) {
    throw new Error(
      `${runLabel} does not match expected sha ${expected.sha}: ${run.headSha ?? '(unknown)'}`,
    );
  }
}

function workflowMatches(run, expectedWorkflow) {
  if (!expectedWorkflow) return true;

  if (run.workflowPath) {
    return normalizeWorkflowIdentifier(run.workflowPath) === normalizeWorkflowIdentifier(expectedWorkflow);
  }

  return run.workflowName === expectedWorkflow;
}

function normalizeWorkflowIdentifier(value) {
  const normalized = String(value ?? '').replaceAll('\\', '/').replace(/^\/+/, '');
  if (normalized.includes('/')) {
    return normalized;
  }
  return `.github/workflows/${normalized}`;
}

function normalizeSha(value) {
  return String(value ?? '').trim().toLowerCase();
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

function downloadArtifactToDirectory(
  repo,
  runId,
  artifactName,
  destination,
  artifactReplacementTransaction = new ArtifactReplacementTransaction(),
) {
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

    return artifactReplacementTransaction.replaceFromDirectory(tempDir, destination);
  } finally {
    fs.rmSync(tempDir, { recursive: true, force: true });
  }
}

export function replaceArtifactsDirectoryFromDownloadedFiles(source, destination, context = {}) {
  const resolved = resolveAllowedArtifactsDirectory(destination, context);
  const renameSync = context.renameSync ?? fs.renameSync;
  const replaceInPlace = context.replaceDirectoryContentsInPlace ?? replaceDirectoryContentsInPlace;
  const artifactValidator = context.artifactValidator ?? {
    validate: context.validateDownloadedArtifactContents ?? validateDownloadedArtifactContents,
  };
  const validateArtifacts = (directory) => artifactValidator.validate(directory);
  const files = listFilesRecursive(source);
  if (files.length === 0) {
    throw new Error('Downloaded artifact did not contain any files');
  }

  const parent = path.dirname(resolved);
  fs.mkdirSync(parent, { recursive: true });
  const stagingDir = fs.mkdtempSync(path.join(parent, `.${path.basename(resolved)}-staging-`));
  const backupDir = path.join(parent, `.${path.basename(resolved)}-backup-${process.pid}-${Date.now()}`);
  let backupCreated = false;
  let replacedInPlace = false;
  let replacementSucceeded = false;

  try {
    for (const file of files) {
      const relativePath = normalizeDownloadedPath(path.relative(source, file));
      const targetPath = path.join(stagingDir, relativePath);
      fs.mkdirSync(path.dirname(targetPath), { recursive: true });
      fs.copyFileSync(file, targetPath);
    }

    validateArtifacts(stagingDir);

    if (fs.existsSync(resolved)) {
      try {
        renameSync(resolved, backupDir);
        backupCreated = true;
      } catch (err) {
        if (!isRecoverableDirectoryRenameError(err)) {
          throw err;
        }
        replaceArtifactsContentsInPlaceWithRollback(
          stagingDir,
          resolved,
          parent,
          replaceInPlace,
          validateArtifacts,
        );
        replacedInPlace = true;
      }
    }

    if (!replacedInPlace) {
      try {
        renameSync(stagingDir, resolved);
      } catch (err) {
        if (backupCreated && !fs.existsSync(resolved)) {
          try {
            renameSync(backupDir, resolved);
            backupCreated = false;
          } catch (rollbackErr) {
            backupCreated = false;
            throw new Error(
              `Failed to replace artifacts (${err.message}) and failed to restore backup at ${backupDir}: ${rollbackErr.message}`,
            );
          }
        }
        if (backupCreated) {
          throw new Error(
            `Failed to replace artifacts (${err.message}). Backup was preserved at ${backupDir}`,
          );
        }
        throw err;
      }
    }

    const replacedFiles = listFilesRecursive(resolved);
    replacementSucceeded = true;

    if (backupCreated) {
      fs.rmSync(backupDir, { recursive: true, force: true });
      backupCreated = false;
    }

    return replacedFiles;
  } finally {
    fs.rmSync(stagingDir, { recursive: true, force: true });
    if (backupCreated && replacementSucceeded) {
      fs.rmSync(backupDir, { recursive: true, force: true });
    }
  }
}

function replaceArtifactsContentsInPlaceWithRollback(
  source,
  destination,
  parent,
  replaceInPlace,
  validateArtifacts,
) {
  const backupDir = fs.mkdtempSync(path.join(parent, `.${path.basename(destination)}-inplace-backup-`));
  let backupCreated = false;
  let preserveBackup = false;

  try {
    if (fs.existsSync(destination)) {
      replaceInPlace(destination, backupDir);
      backupCreated = true;
    }

    replaceInPlace(source, destination);
    validateArtifacts(destination);
  } catch (err) {
    if (backupCreated) {
      try {
        replaceInPlace(backupDir, destination);
      } catch (rollbackErr) {
        preserveBackup = true;
        throw new Error(
          `In-place artifact replacement failed (${err.message}) and failed to restore backup at ${backupDir}: ${rollbackErr.message}`,
        );
      }
    }
    throw err;
  } finally {
    if (!preserveBackup) {
      fs.rmSync(backupDir, { recursive: true, force: true });
    }
  }
}

export function replaceDirectoryContentsInPlace(source, destination) {
  const sourceFiles = listFilesRecursive(source);
  const sourceRelativeFiles = new Set();
  const sourceRelativeDirs = new Set(['']);

  fs.mkdirSync(destination, { recursive: true });

  for (const file of sourceFiles) {
    const relativePath = normalizePortableRelativePath(path.relative(source, file));
    sourceRelativeFiles.add(relativePath);
    for (const directory of parentRelativeDirectories(relativePath)) {
      sourceRelativeDirs.add(directory);
    }

    const targetPath = path.join(destination, ...relativePath.split('/'));
    fs.mkdirSync(path.dirname(targetPath), { recursive: true });
    fs.copyFileSync(file, targetPath);
  }

  for (const file of listFilesRecursive(destination)) {
    const relativePath = normalizePortableRelativePath(path.relative(destination, file));
    if (!sourceRelativeFiles.has(relativePath)) {
      fs.rmSync(file, { force: true });
    }
  }

  for (const directory of listDirectoriesRecursive(destination)) {
    const relativePath = normalizePortableRelativePath(path.relative(destination, directory));
    if (!sourceRelativeDirs.has(relativePath)) {
      fs.rmdirSync(directory);
    }
  }
}

export function validateDownloadedArtifactContents(directory) {
  const fileEntries = listFilesRecursive(directory).map((file) => ({
    path: file,
    relativePath: normalizeDownloadedPath(path.relative(directory, file))
      .split(path.sep)
      .join('/'),
  }));
  const fileByRelativePath = new Map(
    fileEntries.map((file) => [file.relativePath, file.path]),
  );
  const installer = fileEntries.find((file) =>
    /^m3u8-queue-downloader_.*_x64-setup\.exe$/i.test(file.relativePath),
  );
  const missing = [];
  const empty = [];

  if (!installer) {
    missing.push('m3u8-queue-downloader_*_x64-setup.exe');
  } else if (fs.statSync(installer.path).size <= 0) {
    empty.push(installer.relativePath);
  }

  for (const required of [
    'm3u8-queue-downloader-portable/m3u8-queue-downloader.exe',
    'm3u8-queue-downloader-portable/resources/N_m3u8DL-CLI_v3.0.2.exe',
    'm3u8-queue-downloader-portable/resources/ffmpeg.exe',
    'm3u8-queue-downloader-portable/lib/ffmpeg/tools/ffmpeg/bin/ffmpeg.exe',
  ]) {
    const file = fileByRelativePath.get(required);
    if (!file) {
      missing.push(required);
    } else if (fs.statSync(file).size <= 0) {
      empty.push(required);
    }
  }

  if (missing.length > 0) {
    throw new Error(
      `Downloaded artifact is missing required files: ${missing.join(', ')}`,
    );
  }

  if (empty.length > 0) {
    throw new Error(
      `Downloaded artifact has empty required files: ${empty.join(', ')}`,
    );
  }
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

export function normalizeDownloadedPath(relativePath) {
  const normalized = normalizePortableRelativePath(relativePath);
  let stripped = normalized;
  const prefixes = [
    '.portable-dist/',
    'src-tauri/target/release/bundle/nsis/',
  ];

  for (const prefix of prefixes) {
    if (normalized.startsWith(prefix)) {
      stripped = normalized.slice(prefix.length);
      break;
    }
  }

  const portablePath = normalizePortableRelativePath(stripped);
  if (isUnsafeRelativePath(portablePath)) {
    throw new Error(`Refusing unsafe artifact path: ${relativePath}`);
  }

  return portablePath.split('/').join(path.sep);
}

function normalizePortableRelativePath(relativePath) {
  return String(relativePath ?? '').split('\\').join('/');
}

function isUnsafeRelativePath(relativePath) {
  if (!relativePath || path.isAbsolute(relativePath) || path.win32.isAbsolute(relativePath)) {
    return true;
  }

  return relativePath
    .split('/')
    .some((segment) => segment === '..' || segment === '');
}

function parentRelativeDirectories(relativePath) {
  const directories = [];
  let directory = path.posix.dirname(relativePath);
  while (directory && directory !== '.') {
    directories.push(directory);
    directory = path.posix.dirname(directory);
  }
  return directories;
}

function isRecoverableDirectoryRenameError(err) {
  return ['EPERM', 'EACCES', 'EBUSY'].includes(err?.code);
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

function listDirectoriesRecursive(directory) {
  const entries = fs.readdirSync(directory, { withFileTypes: true });
  const directories = [];

  for (const entry of entries) {
    const fullPath = path.join(directory, entry.name);
    if (entry.isDirectory()) {
      directories.push(...listDirectoriesRecursive(fullPath));
      directories.push(fullPath);
    }
  }

  return directories.sort((left, right) => right.length - left.length);
}

function sleep(ms) {
  Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, ms);
}

if (isMainModule(import.meta.url, process.argv[1])) {
  await main(process.argv, import.meta.url);
}
