/**
 * Publish configuration for Synodic
 */
import type { PublishConfig } from '@codervisor/forge';

export default {
  scope: '@codervisor',

  binaries: [
    { name: 'synodic', scope: 'synodic', cargoPackage: 'syn-cli' },
  ],

  platforms: ['darwin-x64', 'darwin-arm64', 'linux-x64', 'windows-x64'],

  mainPackages: [
    { path: 'packages/cli', name: '@codervisor/synodic' },
  ],

  cargoWorkspace: 'Cargo.toml',

  repositoryUrl: 'https://github.com/codervisor/synodic',
} satisfies PublishConfig;
