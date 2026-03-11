#!/usr/bin/env node

/**
 * bin.js — CLI entry point that resolves and executes the Rust binary.
 *
 * This is the thin JS wrapper that makes `npx synodic` work.
 * It finds the correct platform-specific binary package (installed via
 * optionalDependencies) and spawns it with all CLI arguments forwarded.
 */

const { execFileSync } = require('child_process');
const { join } = require('path');

// Map Node.js platform+arch → npm platform package name
const PLATFORMS = {
  'darwin-arm64': '@codervisor/synodic-darwin-arm64',
  'darwin-x64':   '@codervisor/synodic-darwin-x64',
  'linux-x64':    '@codervisor/synodic-linux-x64',
  'win32-x64':    '@codervisor/synodic-windows-x64',
};

function getBinaryPath() {
  const platformKey = `${process.platform}-${process.arch}`;
  const packageName = PLATFORMS[platformKey];

  if (!packageName) {
    const supported = Object.keys(PLATFORMS)
      .map((k) => `  - ${k}`)
      .join('\n');
    console.error(
      `Unsupported platform: ${platformKey}\n\nSupported platforms:\n${supported}`
    );
    process.exit(1);
  }

  try {
    const pkgDir = join(
      require.resolve(`${packageName}/package.json`),
      '..'
    );
    const pkgMeta = require(`${packageName}/package.json`);
    return join(pkgDir, pkgMeta.main);
  } catch {
    console.error(
      `Failed to find package "${packageName}" for platform ${platformKey}.\n` +
      'This usually means the optional dependency was not installed.\n\n' +
      'Try reinstalling with:\n  npm install\n\n' +
      'If the problem persists, install the platform package directly:\n' +
      `  npm install ${packageName}`
    );
    process.exit(1);
  }
}

const binary = getBinaryPath();

try {
  execFileSync(binary, process.argv.slice(2), { stdio: 'inherit' });
} catch (error) {
  if (error.status !== null) {
    process.exit(error.status);
  }
  throw error;
}
