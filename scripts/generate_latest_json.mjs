#!/usr/bin/env node

import { readFileSync, writeFileSync } from 'node:fs';
import { basename, resolve } from 'node:path';

const options = new Map();
for (let index = 2; index < process.argv.length; index += 2) {
  options.set(process.argv[index], process.argv[index + 1]);
}

for (const required of ['--version', '--archive', '--signature', '--tag', '--output']) {
  if (!options.get(required)) throw new Error(`Missing ${required}`);
}

const version = options.get('--version');
const archive = resolve(options.get('--archive'));
const signature = readFileSync(resolve(options.get('--signature')), 'utf8').trim();
if (!signature) throw new Error('Updater signature is empty');

const changelog = readFileSync(resolve('CHANGELOG.md'), 'utf8');
const unreleased = changelog.match(/## Unreleased\s*([\s\S]*?)(?=\n## |$)/)?.[1]?.trim();
const notes = unreleased || `Hen Local Translator ${version}`;
const repository = process.env.GITHUB_REPOSITORY || 'Hen-Local/Hen-Local-Translator';
const encodedArchive = encodeURIComponent(basename(archive)).replaceAll('%2F', '/');
const url = `https://github.com/${repository}/releases/download/${options.get('--tag')}/${encodedArchive}`;

const payload = {
  version,
  notes,
  pub_date: new Date().toISOString(),
  platforms: {
    'darwin-aarch64': { url, signature }
  }
};

writeFileSync(resolve(options.get('--output')), `${JSON.stringify(payload, null, 2)}\n`);
