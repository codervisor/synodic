#!/usr/bin/env tsx
/**
 * Validates a factory manifest.json against the JSON Schema.
 *
 * Usage: pnpm tsx scripts/validate-manifest.ts <path-to-manifest.json>
 *
 * Exit codes:
 *   0 - manifest is valid
 *   1 - manifest is invalid (validation errors printed)
 *   2 - usage error (missing argument, file not found, etc.)
 */

import { readFileSync } from "node:fs";
import { resolve, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import Ajv from "ajv";

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);
const projectRoot = resolve(__dirname, "..");

// --- CLI argument handling ---
const manifestPath = process.argv[2];

if (!manifestPath) {
  console.error("Usage: pnpm tsx scripts/validate-manifest.ts <path-to-manifest.json>");
  process.exit(2);
}

// --- Read files ---
const schemaPath = resolve(projectRoot, "skills/factory/references/manifest.schema.json");

let schemaText: string;
try {
  schemaText = readFileSync(schemaPath, "utf-8");
} catch {
  console.error(`Error: could not read schema at ${schemaPath}`);
  process.exit(2);
}

const resolvedManifestPath = resolve(manifestPath);
let manifestText: string;
try {
  manifestText = readFileSync(resolvedManifestPath, "utf-8");
} catch {
  console.error(`Error: could not read manifest at ${resolvedManifestPath}`);
  process.exit(2);
}

let schema: unknown;
try {
  schema = JSON.parse(schemaText);
} catch {
  console.error("Error: schema file is not valid JSON");
  process.exit(2);
}

let manifest: unknown;
try {
  manifest = JSON.parse(manifestText);
} catch {
  console.error("Error: manifest file is not valid JSON");
  process.exit(2);
}

// --- Validate ---
const ajv = new Ajv({ allErrors: true, validateSchema: false });
const validate = ajv.compile(schema as Record<string, unknown>);
const valid = validate(manifest);

if (valid) {
  console.log(`Valid: ${resolvedManifestPath} conforms to the factory manifest schema.`);
  process.exit(0);
} else {
  console.error(`Invalid: ${resolvedManifestPath} has validation errors:\n`);
  for (const err of validate.errors ?? []) {
    const path = err.instancePath || "(root)";
    console.error(`  - ${path}: ${err.message}`);
  }
  process.exit(1);
}
