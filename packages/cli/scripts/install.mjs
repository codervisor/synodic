#!/usr/bin/env node
// Postinstall script: resolves the platform-specific binary from optional deps.
import { existsSync } from 'fs';
import { join, dirname } from 'path';
import { fileURLToPath } from 'url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const binDir = join(__dirname, '..', 'bin');

const PLATFORM_MAP = {
  'darwin-arm64': '@synodic/cli-darwin-arm64',
  'darwin-x64': '@synodic/cli-darwin-x64',
  'linux-x64': '@synodic/cli-linux-x64',
  'linux-arm64': '@synodic/cli-linux-arm64',
};

const key = `${process.platform}-${process.arch}`;
const pkg = PLATFORM_MAP[key];

if (!pkg) {
  console.warn(`synodic: no prebuilt binary for ${key}. Build from source: cd rust && cargo build --release`);
  process.exit(0);
}

try {
  const resolved = import.meta.resolve(pkg);
  console.log(`synodic: using ${pkg}`);
} catch {
  console.warn(`synodic: platform package ${pkg} not installed. Build from source.`);
}
