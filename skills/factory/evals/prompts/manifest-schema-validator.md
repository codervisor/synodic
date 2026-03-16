# Eval: Add manifest.json schema validator script

Run the factory skill on the validator spec:

```
/factory run skills/factory/fixtures/manifest-schema-validator/README.md
```

## Why this is hard

- Cross-language: TypeScript script validating a JSON Schema for data produced by Rust structs
- Must install a new npm dependency (`ajv`) and wire it correctly
- Must create both valid and invalid test fixtures that conform to the actual schema
- The existing schema at `skills/factory/references/manifest.schema.json` must be read and understood
- Exit codes must be precise (0, 1, 2) — not just "pass/fail"
- `pnpm tsx` must work — the script must be valid TypeScript

## Verify

1. `scripts/validate-manifest.ts` exists and is valid TypeScript.
2. `ajv` is in `devDependencies` in root `package.json`.
3. `valid-manifest.json` fixture passes validation (exit 0).
4. `invalid-manifest.json` fixture fails validation (exit 1) with printed errors.
5. Running with no args exits 2 with a usage message.
6. The script reads the schema from `skills/factory/references/manifest.schema.json`.
