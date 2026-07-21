#!/usr/bin/env node

import { readFileSync, renameSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const SCRIPT_DIR = dirname(fileURLToPath(import.meta.url));
const ROOT_DIR = resolve(SCRIPT_DIR, '..');
const CARGO_TOML = join(ROOT_DIR, 'Cargo.toml');
const CARGO_LOCK = join(ROOT_DIR, 'Cargo.lock');
const TAURI_CONFIG = join(
  ROOT_DIR,
  'hen-local-translator-shell',
  'tauri.conf.json',
);
const UI_PACKAGE = join(
  ROOT_DIR,
  'hen-local-translator-shell',
  'ui',
  'package.json',
);
const UI_PACKAGE_LOCK = join(
  ROOT_DIR,
  'hen-local-translator-shell',
  'ui',
  'package-lock.json',
);

const SEMVER_PATTERN =
  /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-((?:0|[1-9]\d*|\d*[A-Za-z-][0-9A-Za-z-]*)(?:\.(?:0|[1-9]\d*|\d*[A-Za-z-][0-9A-Za-z-]*))*))?(?:\+([0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*))?$/;
const WORKSPACE_PACKAGES = [
  'dora-qwen3-asr',
  'dora-qwen35-translator',
  'hen-local-init',
  'hen-local-translator-shell',
  'moxin-dora-bridge',
];

function usage() {
  console.log(`Usage:
  node scripts/version.mjs check
  node scripts/version.mjs current
  node scripts/version.mjs check-tag <vMAJOR.MINOR.PATCH[-prerelease]>
  node scripts/version.mjs sync
  node scripts/version.mjs set <major.minor.patch[-prerelease]>

Commands:
  check  Verify every application version matches Cargo.toml.
  current  Print the current workspace version for scripts and CI.
  check-tag  Verify a release tag exactly matches the workspace version.
  sync   Copy the Cargo.toml workspace version to Tauri and npm metadata.
  set    Change the Cargo.toml workspace version, then synchronize metadata.`);
}

function readWorkspaceVersion() {
  const contents = readFileSync(CARGO_TOML, 'utf8');
  const sectionMatch = /^\[workspace\.package\]\s*$/m.exec(contents);
  if (!sectionMatch) {
    throw new Error('Cargo.toml has no [workspace.package] section');
  }

  const sectionStart = sectionMatch.index + sectionMatch[0].length;
  const remaining = contents.slice(sectionStart);
  const nextSection = /^\[/m.exec(remaining);
  const sectionEnd = nextSection
    ? sectionStart + nextSection.index
    : contents.length;
  const section = contents.slice(sectionStart, sectionEnd);
  const versionMatch = /^version\s*=\s*"([^"]+)"\s*$/m.exec(section);

  if (!versionMatch) {
    throw new Error(
      'Cargo.toml [workspace.package] section has no string version',
    );
  }

  validateVersion(versionMatch[1]);
  return versionMatch[1];
}

function validateVersion(version) {
  if (!SEMVER_PATTERN.test(version)) {
    throw new Error(`Invalid semantic version: ${version}`);
  }
}

function readJson(path) {
  return JSON.parse(readFileSync(path, 'utf8'));
}

function cargoLockVersions() {
  const contents = readFileSync(CARGO_LOCK, 'utf8');
  const blocks = contents.split('[[package]]').slice(1);

  return WORKSPACE_PACKAGES.map((packageName) => {
    const block = blocks.find((candidate) => {
      const nameMatch = /^name\s*=\s*"([^"]+)"\s*$/m.exec(candidate);
      return nameMatch?.[1] === packageName;
    });
    if (!block) {
      throw new Error(`Cargo.lock has no package entry for ${packageName}`);
    }

    const versionMatch = /^version\s*=\s*"([^"]+)"\s*$/m.exec(block);
    if (!versionMatch) {
      throw new Error(`Cargo.lock package ${packageName} has no version`);
    }
    return [`Cargo.lock package ${packageName}`, versionMatch[1]];
  });
}

function writeFileAtomic(path, contents) {
  const temporaryPath = `${path}.version-tmp`;
  writeFileSync(temporaryPath, contents);
  renameSync(temporaryPath, path);
}

function writeJson(path, value) {
  writeFileAtomic(path, `${JSON.stringify(value, null, 2)}\n`);
}

function versionTargets() {
  const tauriConfig = readJson(TAURI_CONFIG);
  const uiPackage = readJson(UI_PACKAGE);
  const uiPackageLock = readJson(UI_PACKAGE_LOCK);

  if (!uiPackageLock.packages?.['']) {
    throw new Error('package-lock.json has no root package entry');
  }

  return {
    tauriConfig,
    uiPackage,
    uiPackageLock,
    values: [
      ['hen-local-translator-shell/tauri.conf.json', tauriConfig.version],
      ['hen-local-translator-shell/ui/package.json', uiPackage.version],
      ['hen-local-translator-shell/ui/package-lock.json', uiPackageLock.version],
      [
        'hen-local-translator-shell/ui/package-lock.json packages[""]',
        uiPackageLock.packages[''].version,
      ],
      ...cargoLockVersions(),
    ],
  };
}

function refreshCargoLock() {
  const result = spawnSync(
    'cargo',
    ['update', '--workspace'],
    {
      cwd: ROOT_DIR,
      encoding: 'utf8',
    },
  );
  if (result.error) {
    throw new Error(`Failed to run cargo metadata: ${result.error.message}`);
  }
  if (result.status !== 0) {
    throw new Error(
      `Failed to refresh Cargo.lock:\n${result.stderr.trim() || result.stdout.trim()}`,
    );
  }
}

function checkVersions() {
  const workspaceVersion = readWorkspaceVersion();
  const { values, tauriConfig } = versionTargets();
  const mismatches = values.filter(([, version]) => version !== workspaceVersion);
  const macBundleVersion = workspaceVersion.split(/[-+]/, 1)[0];
  if (tauriConfig.bundle?.macOS?.bundleVersion !== macBundleVersion) {
    mismatches.push([
      'hen-local-translator-shell/tauri.conf.json bundle.macOS.bundleVersion',
      tauriConfig.bundle?.macOS?.bundleVersion,
    ]);
  }

  if (mismatches.length > 0) {
    console.error(`Version mismatch: Cargo.toml is ${workspaceVersion}`);
    for (const [label, version] of mismatches) {
      console.error(`  ${label}: ${String(version)}`);
    }
    console.error('\nRun: node scripts/version.mjs sync');
    process.exitCode = 1;
    return false;
  }

  console.log(`Version check passed: ${workspaceVersion}`);
  return true;
}

function syncVersions() {
  const workspaceVersion = readWorkspaceVersion();
  refreshCargoLock();
  const { tauriConfig, uiPackage, uiPackageLock } = versionTargets();

  tauriConfig.version = workspaceVersion;
  tauriConfig.bundle.macOS.bundleVersion = workspaceVersion.split(/[-+]/, 1)[0];
  uiPackage.version = workspaceVersion;
  uiPackageLock.version = workspaceVersion;
  uiPackageLock.packages[''].version = workspaceVersion;

  writeJson(TAURI_CONFIG, tauriConfig);
  writeJson(UI_PACKAGE, uiPackage);
  writeJson(UI_PACKAGE_LOCK, uiPackageLock);

  console.log(`Synchronized application version ${workspaceVersion}`);
}

function setWorkspaceVersion(version) {
  validateVersion(version);

  const contents = readFileSync(CARGO_TOML, 'utf8');
  const sectionMatch = /^\[workspace\.package\]\s*$/m.exec(contents);
  if (!sectionMatch) {
    throw new Error('Cargo.toml has no [workspace.package] section');
  }

  const sectionStart = sectionMatch.index + sectionMatch[0].length;
  const remaining = contents.slice(sectionStart);
  const nextSection = /^\[/m.exec(remaining);
  const sectionEnd = nextSection
    ? sectionStart + nextSection.index
    : contents.length;
  const section = contents.slice(sectionStart, sectionEnd);
  const updatedSection = section.replace(
    /^(version\s*=\s*")[^"]+("\s*)$/m,
    `$1${version}$2`,
  );

  if (updatedSection === section && readWorkspaceVersion() !== version) {
    throw new Error('Failed to update the workspace package version');
  }

  try {
    writeFileAtomic(
      CARGO_TOML,
      `${contents.slice(0, sectionStart)}${updatedSection}${contents.slice(sectionEnd)}`,
    );
    syncVersions();
  } catch (error) {
    writeFileAtomic(CARGO_TOML, contents);
    throw error;
  }
}

function checkReleaseTag(tag) {
  const workspaceVersion = readWorkspaceVersion();
  const expectedTag = `v${workspaceVersion}`;
  if (tag !== expectedTag) {
    throw new Error(
      `Release tag ${tag} does not match workspace version ${workspaceVersion}; expected ${expectedTag}`,
    );
  }
  console.log(`Release tag check passed: ${tag}`);
}

function main() {
  const [command, value, ...extra] = process.argv.slice(2);
  if (extra.length > 0) {
    usage();
    process.exitCode = 2;
    return;
  }

  switch (command) {
    case 'check':
      if (value !== undefined) {
        usage();
        process.exitCode = 2;
        return;
      }
      checkVersions();
      break;
    case 'current':
      if (value !== undefined) {
        usage();
        process.exitCode = 2;
        return;
      }
      console.log(readWorkspaceVersion());
      break;
    case 'check-tag':
      if (value === undefined) {
        usage();
        process.exitCode = 2;
        return;
      }
      checkReleaseTag(value);
      break;
    case 'sync':
      if (value !== undefined) {
        usage();
        process.exitCode = 2;
        return;
      }
      syncVersions();
      checkVersions();
      break;
    case 'set':
      if (value === undefined) {
        usage();
        process.exitCode = 2;
        return;
      }
      setWorkspaceVersion(value);
      checkVersions();
      break;
    case '--help':
    case '-h':
      usage();
      break;
    default:
      usage();
      process.exitCode = 2;
  }
}

try {
  main();
} catch (error) {
  console.error(`Version command failed: ${error.message}`);
  process.exitCode = 1;
}
