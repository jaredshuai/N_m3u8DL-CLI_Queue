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
const cargoLockPackagePattern = /(\[\[package\]\]\r?\nname = "m3u8-queue-downloader"\r?\nversion = ")([^"]+)(")/;
const releaseVersionPattern = /^(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)(?:-(?:rc|beta|alpha)(?:\.(?:0|[1-9][0-9]*|[0-9A-Za-z-]*[A-Za-z-][0-9A-Za-z-]*))*)?(?![\s\S])/;
const releaseRepository = 'jaredshuai/N_m3u8DL-CLI_Queue';
const releaseTagRulesetName = 'Protect app-v release tags';
const releaseGitHubApiVersion = '2026-03-10';
const expectedPreTagVersionFiles = [
  'package.json',
  'package-lock.json',
  'src-tauri/tauri.conf.json',
  'src-tauri/Cargo.toml',
  'src-tauri/Cargo.lock',
];

function prepareRelease(version) {
  return new ReleasePrepareCliAdapter().run(version);
}

function preTag(version) {
  return new PreTagCliAdapter().run(version);
}

function checkReleaseVersions() {
  const version = assertConsistentReleaseVersions(new JsonVersionFiles().readVersions());
  console.log(`release versions consistent: ${version}`);
  return version;
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
  const runPreTag = context.preTag ?? preTag;
  const runVersionCheck = context.versionCheck ?? checkReleaseVersions;
  const exit = context.exit ?? process.exit;

  if (command === 'package-sync') {
    await runPackageSync(args.slice(1));
    exit(0);
    return;
  }

  if (command === 'version-check') {
    runVersionCheck();
    exit(0);
    return;
  }

  if (command === 'pre-tag') {
    await runPreTag(args[1]);
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

export class PreTagCliAdapter {
  constructor(dependencies = defaultPreTagCliDependencies()) {
    this.dependencies = dependencies;
  }

  run(version) {
    if (!isValidReleaseVersion(version)) {
      this.dependencies.reporter.preTagUsage();
      this.dependencies.exit(1);
      return;
    }

    try {
      const result = this.dependencies.preTagUseCase.run({ version });
      this.dependencies.reporter.preTagVerified(result);
      this.dependencies.exit(0);
    } catch (error) {
      this.dependencies.reporter.preTagFailed(error);
      this.dependencies.exit(1);
    }
  }
}

export class PreTagUseCase {
  constructor(dependencies = defaultPreTagUseCaseDependencies()) {
    this.dependencies = dependencies;
  }

  run({ version }) {
    const gateway = this.dependencies.gateway;
    const branch = runPreTagStep(
      'Failed to read current git branch',
      () => gateway.currentBranch(),
    );
    if (branch !== 'master') {
      throw new Error(`Pre-tag gate requires branch master; current branch is ${branch || '(detached)'}.`);
    }

    const status = runPreTagStep(
      'Failed to inspect the git working tree',
      () => gateway.workingTreeStatus(),
    );
    if (status !== '') {
      throw new Error(`Pre-tag gate requires a clean working tree; git status --porcelain returned: ${status}`);
    }

    runPreTagStep('Failed to fetch origin master', () => gateway.fetchOriginMaster());
    const headSha = runPreTagStep('Failed to resolve HEAD', () => gateway.currentHeadSha());
    const originMasterSha = runPreTagStep(
      'Failed to resolve origin/master',
      () => gateway.originMasterHeadSha(),
    );
    if (headSha !== originMasterSha) {
      throw new Error(`HEAD ${headSha} does not match origin/master ${originMasterSha}.`);
    }

    const versionFiles = runPreTagStep(
      'Failed to read release version files',
      () => this.dependencies.versionFiles.readVersions(),
    );
    assertExpectedPreTagVersionFiles(versionFiles);
    const fileVersion = assertConsistentReleaseVersions(versionFiles);
    if (fileVersion !== version) {
      throw new Error(
        `Requested release version ${version} does not match version files ${fileVersion}.`,
      );
    }

    const tag = `app-v${version}`;
    const localTagExists = runPreTagStep(
      `Failed to inspect local release tag ${tag}`,
      () => gateway.localTagExists(tag),
    );
    if (localTagExists) {
      throw new Error(`Local release tag ${tag} already exists.`);
    }

    const remoteTagExists = runPreTagStep(
      `Failed to inspect remote release tag ${tag}`,
      () => gateway.remoteTagExists(tag),
    );
    if (remoteTagExists) {
      throw new Error(`Remote release tag ${tag} already exists.`);
    }

    const rulesets = runPreTagStep(
      'Failed to query repository tag rulesets',
      () => gateway.listRepositoryTagRulesets(),
    );
    if (!Array.isArray(rulesets)) {
      throw new Error('Repository tag rulesets response must be an array.');
    }
    const matchingRulesets = rulesets.filter(({ name }) => name === releaseTagRulesetName);
    if (matchingRulesets.length !== 1) {
      throw new Error(
        `Expected exactly one repository tag ruleset named ${releaseTagRulesetName}; found ${matchingRulesets.length}.`,
      );
    }

    const rulesetSummary = matchingRulesets[0];
    const ruleset = runPreTagStep(
      `Failed to read release tag ruleset ${rulesetSummary.id}`,
      () => gateway.getRepositoryRuleset(rulesetSummary.id),
    );
    assertReleaseTagRuleset(ruleset);

    const immutableReleases = runPreTagStep(
      'Failed to query repository Immutable Releases',
      () => gateway.getImmutableReleases(),
    );
    if (immutableReleases?.enabled !== true) {
      throw new Error('Repository Immutable Releases must be enabled before creating the tag.');
    }

    return {
      version,
      tag,
      branch,
      headSha,
      rulesetId: ruleset.id,
      rulesetName: ruleset.name,
      immutableReleasesEnabled: true,
    };
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
      if (path.basename(file) === 'Cargo.lock') {
        updateCargoLockVersion(file, version);
      } else if (file.endsWith('.toml')) {
        updateTomlVersion(file, version);
      } else {
        updateJsonVersion(file, version);
      }
      updatedFiles.push(path.relative(this.rootDir, file));
    }
    return updatedFiles;
  }

  readVersions() {
    return this.files.map((file) => ({
      file: path.relative(this.rootDir, file),
      version: readReleaseVersion(file),
    }));
  }
}

export class PreTagVersionFiles {
  constructor(options = {}) {
    this.rootDir = options.rootDir ?? root;
    this.files = options.files ?? defaultPreTagVersionFiles();
  }

  readVersions() {
    return this.files.map((file) => ({
      file: path.relative(this.rootDir, file),
      version: readPreTagReleaseVersion(file),
    }));
  }
}

export function assertConsistentReleaseVersions(versionFiles) {
  if (versionFiles.length === 0) {
    throw new Error('No release version files configured');
  }

  const versions = new Set(versionFiles.map(({ version }) => version));
  if (versions.size !== 1) {
    const details = versionFiles.map(({ file, version }) => `  ${file}: ${version}`).join('\n');
    throw new Error(`Release version mismatch:\n${details}`);
  }

  return versionFiles[0].version;
}

export class ReleasePrepareReporter {
  constructor(output = {}) {
    this.log = output.log ?? console.log;
    this.error = output.error ?? console.error;
  }

  usage() {
    this.error('Usage: npm run release:prepare -- <version>');
    this.error('Allowed: X.Y.Z or X.Y.Z-(rc|beta|alpha)[.<SemVer prerelease identifier>...]');
    this.error('ASCII numeric identifiers must not have leading zeroes; build metadata is not supported.');
    this.error('Example: npm run release:prepare -- 0.2.0');
  }

  preTagUsage() {
    this.error('Usage: node scripts/prepare-release.mjs pre-tag <version>');
    this.error('Allowed: X.Y.Z or X.Y.Z-(rc|beta|alpha)[.<SemVer prerelease identifier>...]');
    this.error('ASCII numeric identifiers must not have leading zeroes; build metadata is not supported.');
  }

  preTagFailed(error) {
    this.error(`Pre-tag gate failed: ${errorMessage(error)}`);
  }

  preTagVerified(result) {
    this.log(`Pre-tag gate passed for ${result.tag}:`);
    this.log(`  branch: ${result.branch}`);
    this.log(`  HEAD == origin/master: ${result.headSha}`);
    this.log(`  version files: ${result.version}`);
    this.log('  local and remote tag: absent');
    this.log(`  ruleset: ${result.rulesetName} (${result.rulesetId})`);
    this.log('  repository Immutable Releases: enabled');
  }

  versionFilesUpdated(files, version) {
    for (const file of files) {
      this.log(`updated ${file} -> ${version}`);
    }
  }

  nextSteps(version) {
    this.log('\nNext steps:');
    this.log('  npm install --package-lock-only --ignore-scripts');
    this.log('  npm run check:versions');
    this.log('  git add package.json package-lock.json src-tauri/tauri.conf.json src-tauri/Cargo.toml src-tauri/Cargo.lock');
    this.log(`  git commit -m "chore(release): v${version}"`);
    this.log('  git push origin master');
    this.log('');
    this.log('Run the mechanical pre-tag gate:');
    this.log(`  node scripts/prepare-release.mjs pre-tag ${version}`);
    this.log('');
    this.log('Only after the pre-tag gate passes:');
    this.log(`  git tag app-v${version}`);
    this.log(`  git push origin app-v${version}`);
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

function defaultPreTagCliDependencies() {
  const reporter = new ReleasePrepareReporter();
  return {
    reporter,
    preTagUseCase: new PreTagUseCase(),
    exit: process.exit,
  };
}

function defaultPreTagUseCaseDependencies() {
  return {
    gateway: new LocalPreTagGateway(),
    versionFiles: new PreTagVersionFiles(),
  };
}

function defaultReleaseVersionFiles() {
  return [
    path.join(root, 'package.json'),
    path.join(root, 'src-tauri', 'tauri.conf.json'),
    path.join(root, 'src-tauri', 'Cargo.toml'),
    path.join(root, 'src-tauri', 'Cargo.lock'),
  ];
}

function defaultPreTagVersionFiles() {
  return [
    path.join(root, 'package.json'),
    path.join(root, 'package-lock.json'),
    path.join(root, 'src-tauri', 'tauri.conf.json'),
    path.join(root, 'src-tauri', 'Cargo.toml'),
    path.join(root, 'src-tauri', 'Cargo.lock'),
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

function readReleaseVersion(file) {
  if (path.basename(file) === 'Cargo.lock') {
    return readCargoLockVersion(file);
  }
  if (file.endsWith('.toml')) {
    return readTomlVersion(file);
  }
  return JSON.parse(fs.readFileSync(file, 'utf8')).version;
}

function readPreTagReleaseVersion(file) {
  if (path.basename(file) === 'package-lock.json') {
    return readPackageLockVersion(file);
  }
  return readReleaseVersion(file);
}

function readPackageLockVersion(file) {
  const packageLock = JSON.parse(fs.readFileSync(file, 'utf8'));
  const rootVersion = packageLock.version;
  const packageEntryVersion = packageLock.packages?.['']?.version;
  if (typeof rootVersion !== 'string' || typeof packageEntryVersion !== 'string') {
    throw new Error(`${path.basename(file)} must contain string versions at root and packages[""]`);
  }
  if (rootVersion !== packageEntryVersion) {
    throw new Error(
      `${path.basename(file)} root version ${rootVersion} does not match packages[""] version ${packageEntryVersion}`,
    );
  }
  return rootVersion;
}

function readTomlVersion(file) {
  const content = fs.readFileSync(file, 'utf8');
  const match = content.match(/^version = "([^"]+)"/m);
  if (!match) {
    throw new Error(`TOML version not found: ${file}`);
  }
  return match[1];
}

function updateCargoLockVersion(file, version) {
  const content = fs.readFileSync(file, 'utf8');
  if (!cargoLockPackagePattern.test(content)) {
    throw new Error(`Cargo.lock package entry not found: ${file}`);
  }
  const updated = content.replace(
    cargoLockPackagePattern,
    (_match, prefix, _currentVersion, suffix) => `${prefix}${version}${suffix}`,
  );
  fs.writeFileSync(file, updated, 'utf8');
}

function readCargoLockVersion(file) {
  const content = fs.readFileSync(file, 'utf8');
  const match = content.match(cargoLockPackagePattern);
  if (!match) {
    throw new Error(`Cargo.lock package entry not found: ${file}`);
  }
  return match[2];
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

export class LocalPreTagGateway {
  constructor(options = {}) {
    this.repo = options.repo ?? releaseRepository;
    this.apiVersion = options.apiVersion ?? releaseGitHubApiVersion;
    this.gitOutput = options.gitOutput ?? runGitOutput;
    this.gitStatus = options.gitStatus ?? runGitStatus;
    this.ghJson = options.ghJson ?? runGhJson;
  }

  currentBranch() {
    return this.gitOutput(['branch', '--show-current']);
  }

  workingTreeStatus() {
    return this.gitOutput(['status', '--porcelain']);
  }

  fetchOriginMaster() {
    this.gitOutput(['fetch', 'origin', 'master']);
  }

  currentHeadSha() {
    return this.gitOutput(['rev-parse', 'HEAD']);
  }

  originMasterHeadSha() {
    return this.gitOutput(['rev-parse', 'origin/master']);
  }

  localTagExists(tag) {
    return referenceExists(
      this.gitStatus(['show-ref', '--verify', '--quiet', `refs/tags/${tag}`]),
      1,
      `local tag ${tag}`,
    );
  }

  remoteTagExists(tag) {
    return referenceExists(
      this.gitStatus([
        'ls-remote', '--exit-code', '--tags', 'origin', `refs/tags/${tag}`,
      ]),
      2,
      `remote tag ${tag}`,
    );
  }

  listRepositoryTagRulesets() {
    return this.ghJson([
      'api', '--method', 'GET',
      '-H', `X-GitHub-Api-Version: ${this.apiVersion}`,
      `repos/${this.repo}/rulesets`,
      '-f', 'targets=tag',
      '-F', 'includes_parents=false',
    ]);
  }

  getRepositoryRuleset(id) {
    return this.ghJson([
      'api', '--method', 'GET',
      '-H', `X-GitHub-Api-Version: ${this.apiVersion}`,
      `repos/${this.repo}/rulesets/${id}`,
    ]);
  }

  getImmutableReleases() {
    return this.ghJson([
      'api', '--method', 'GET',
      '-H', `X-GitHub-Api-Version: ${this.apiVersion}`,
      `repos/${this.repo}/immutable-releases`,
    ]);
  }
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
  return typeof version === 'string' && releaseVersionPattern.test(version);
}

function runPreTagStep(label, operation) {
  try {
    return operation();
  } catch (error) {
    throw new Error(`${label}: ${errorMessage(error)}`);
  }
}

function errorMessage(error) {
  return error instanceof Error ? error.message : String(error);
}

function assertExpectedPreTagVersionFiles(versionFiles) {
  if (!Array.isArray(versionFiles)) {
    throw new Error('Pre-tag version files response must be an array.');
  }
  const actualFiles = versionFiles.map(({ file }) => String(file).replaceAll('\\', '/'));
  if (JSON.stringify(actualFiles) !== JSON.stringify(expectedPreTagVersionFiles)) {
    throw new Error(
      `Pre-tag gate must read exactly these five version files: ${expectedPreTagVersionFiles.join(', ')}. Found: ${actualFiles.join(', ')}`,
    );
  }
}

function assertReleaseTagRuleset(ruleset) {
  if (!ruleset || typeof ruleset !== 'object') {
    throw new Error('Release tag ruleset response must be an object.');
  }
  if (ruleset.name !== releaseTagRulesetName) {
    throw new Error(
      `Release tag ruleset name must be ${releaseTagRulesetName}; got ${String(ruleset.name)}.`,
    );
  }
  if (ruleset.enforcement !== 'active') {
    throw new Error(
      `Release tag ruleset enforcement must be active; got ${String(ruleset.enforcement)}.`,
    );
  }
  if (ruleset.target !== 'tag') {
    throw new Error(`Release tag ruleset target must be tag; got ${String(ruleset.target)}.`);
  }

  const include = ruleset.conditions?.ref_name?.include;
  if (JSON.stringify(include) !== JSON.stringify(['refs/tags/app-v*'])) {
    throw new Error(
      `Release tag ruleset include must be exactly refs/tags/app-v*; got ${JSON.stringify(include)}.`,
    );
  }
  const exclude = ruleset.conditions?.ref_name?.exclude;
  if (JSON.stringify(exclude) !== JSON.stringify([])) {
    throw new Error(
      `Release tag ruleset exclude must be empty; got ${JSON.stringify(exclude)}.`,
    );
  }

  const ruleTypes = Array.isArray(ruleset.rules)
    ? ruleset.rules.map(({ type }) => type).sort()
    : null;
  if (JSON.stringify(ruleTypes) !== JSON.stringify(['deletion', 'update'])) {
    throw new Error(
      `Release tag ruleset rule types must be exactly deletion and update; got ${JSON.stringify(ruleTypes)}.`,
    );
  }
  if (JSON.stringify(ruleset.bypass_actors) !== JSON.stringify([])) {
    throw new Error(
      `Release tag ruleset bypass actors must be empty; got ${JSON.stringify(ruleset.bypass_actors)}.`,
    );
  }
  if (ruleset.current_user_can_bypass !== 'never') {
    throw new Error(
      `Release tag ruleset current_user_can_bypass must be never; got ${String(ruleset.current_user_can_bypass)}.`,
    );
  }
}

function referenceExists(result, absentStatus, label) {
  if (result.error) {
    throw new Error(`${label} check failed: ${errorMessage(result.error)}`);
  }
  if (result.status === 0) {
    return true;
  }
  if (result.status === absentStatus) {
    return false;
  }
  throw new Error(
    `${label} check failed with exit ${String(result.status)}: ${String(result.stderr ?? '').trim()}`,
  );
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

function runGitOutput(args) {
  return execFileSync('git', ['-C', repoRoot, ...args], {
    encoding: 'utf8',
    stdio: ['ignore', 'pipe', 'pipe'],
  }).trim();
}

function runGitStatus(args) {
  return spawnSync('git', ['-C', repoRoot, ...args], {
    encoding: 'utf8',
    stdio: ['ignore', 'pipe', 'pipe'],
  });
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
