#!/usr/bin/env node

const { spawnSync } = require('node:child_process');
const path = require('node:path');

const TARGETS = {
  darwin: {
    arm64: { pkg: '@blprnt/unfuk-darwin-arm64', bin: 'unfuk' },
  },
  linux: {
    x64: { pkg: '@blprnt/unfuk-linux-x64', bin: 'unfuk' },
  },
  win32: {
    x64: { pkg: '@blprnt/unfuk-win32-x64', bin: 'unfuk.exe' },
  },
};

const target = TARGETS[process.platform]?.[process.arch];

if (!target) {
  console.error(
    `Unsupported platform: ${process.platform}-${process.arch}. Supported targets: darwin-arm64, linux-x64, win32-x64.`
  );
  process.exit(1);
}

let binaryPath;

try {
  const packageJsonPath = require.resolve(`${target.pkg}/package.json`);
  binaryPath = path.join(path.dirname(packageJsonPath), target.bin);
} catch (error) {
  console.error(`Missing optional dependency ${target.pkg} for ${process.platform}-${process.arch}.`);
  process.exit(1);
}

const result = spawnSync(binaryPath, process.argv.slice(2), { stdio: 'inherit' });

if (result.error) {
  console.error(result.error.message);
  process.exit(1);
}

if (typeof result.status === 'number') {
  process.exit(result.status);
}

process.exit(1);