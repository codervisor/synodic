/**
 * prepare-publish.ts — Replace workspace:* dependencies with actual version numbers.
 *
 * Usage:
 *   pnpm tsx scripts/prepare-publish.ts
 */

import { readFileSync, writeFileSync, copyFileSync } from 'fs';
import { join, resolve, dirname } from 'path';
import { fileURLToPath } from 'url';
import { globSync } from 'glob';

const __dirname = dirname(fileURLToPath(import.meta.url));
const ROOT = resolve(__dirname, '..');
const rootPkg = JSON.parse(readFileSync(join(ROOT, 'package.json'), 'utf8'));
const version = rootPkg.version;

const publishableGlobs = ['packages/*/package.json'];

const DEP_TYPES = ['dependencies', 'devDependencies', 'peerDependencies'] as const;

function main() {
  console.log(`📦 Preparing packages for publish (version: ${version})`);
  let replaced = 0;

  for (const pattern of publishableGlobs) {
    for (const pkgPath of globSync(join(ROOT, pattern))) {
      const pkg = JSON.parse(readFileSync(pkgPath, 'utf8'));
      let modified = false;

      for (const depType of DEP_TYPES) {
        const deps = pkg[depType];
        if (!deps) continue;
        for (const [name, ver] of Object.entries(deps)) {
          const verStr = String(ver);
          if (verStr.startsWith('workspace:')) {
            deps[name] = verStr === 'workspace:^' ? `^${version}` :
                         verStr === 'workspace:~' ? `~${version}` : version;
            modified = true;
            replaced++;
          }
        }
      }

      if (modified) {
        copyFileSync(pkgPath, `${pkgPath}.backup`);
        writeFileSync(pkgPath, JSON.stringify(pkg, null, 2) + '\n');
        console.log(`  ✅ ${pkg.name}: updated (backup created)`);
      }
    }
  }

  console.log(`\n📊 Replaced ${replaced} workspace protocol reference(s)`);
}

main();
