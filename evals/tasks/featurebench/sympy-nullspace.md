# Eval: FeatureBench — SymPy Matrix Operations (Fractal Decomposition)

## Source

- **Benchmark**: [FeatureBench](https://huggingface.co/datasets/LiberCoders/FeatureBench) (ICLR 2026)
- **Instance**: `sympy__sympy.c1097516.test_nullspace.f14fc970.lv1`
- **Repo**: sympy/sympy
- **Difficulty**: 18 files changed, 20k char problem statement
- **SOTA resolve rate**: 11% (Claude Opus 4.5 via Claude Code)

## Why this task tests fractal decomposition

Matrix representation systems have **natural algebraic orthogonality**: format conversion, element access, linear algebra ops, and domain handling are independent concerns with shared type contracts. The 18-file change span makes monolithic approaches brittle. Fractal decomposition maps cleanly:

- **Orthogonal sub-problems**: Each operation type is independently implementable
- **Shared interfaces**: All operations share the matrix representation types (DDM, SDM, DFM)
- **Depth potential**: Linear algebra ops can further split into nullspace, factorization, etc.

## Prompt

```
You have the fractal decomposition skill loaded.

/fractal decompose "Implement matrix representation conversion and manipulation
interfaces for a domain-based matrix system in SymPy:

1. **Format Conversion**: Seamless conversion between dense (DDM), sparse (SDM),
   and flat (DFM) matrix representations preserving mathematical properties
   and domain constraints.

2. **Element Access**: Efficient indexing and slicing for scalar retrieval and
   submatrix extraction with bounds checking across all representations.

3. **Linear Algebra — Nullspace**: Nullspace computation using reduced row echelon
   form, supporting both field and ring domains with normalization options.

4. **Linear Algebra — Factorization**: Primitive form extraction by factoring out
   GCDs from matrix elements while maintaining domain integrity.

5. **Domain Compatibility**: Ensure all operations respect domain-specific constraints
   and support domain conversion when mathematically valid.

Key constraints:
- Mathematical correctness across DDM, SDM, DFM representations
- Domain-specific arithmetic rules (division in non-fields)
- Memory efficiency for large sparse matrices
- Numerical stability in factorization
- Consistent interfaces regardless of underlying representation

Config: output_mode=design, split_strategy=orthogonal, max_depth=3"

Follow the full orchestration protocol from SKILL.md.
```

## Expected decomposition tree

```
root: Matrix Representation System
├── 1-format-conversion/
│   ├── 1-ddm-sdm/              (dense ↔ sparse conversion)
│   └── 2-dfm-adapters/         (flat format adapters)
├── 2-element-access/            (LEAF — indexing + slicing)
├── 3-nullspace/
│   ├── 1-rref/                  (reduced row echelon form)
│   └── 2-kernel-extraction/     (nullspace from RREF)
├── 4-factorization/             (LEAF — GCD extraction + primitive form)
└── 5-domain-layer/              (LEAF — domain constraints + conversion rules)
```

## Grading markers

```json
{
  "benchmark": "featurebench",
  "instance_id": "sympy__sympy.c1097516.test_nullspace.f14fc970.lv1",
  "markers": {
    "representation_types_shared": {
      "check": "DDM, SDM, DFM types are defined once and referenced across children, not duplicated",
      "required": true
    },
    "domain_layer_isolated": {
      "check": "Domain constraints are a separate sub-spec, not mixed into every operation",
      "required": true
    },
    "nullspace_decomposed": {
      "check": "Nullspace computation split into RREF + kernel extraction (depth >= 2)",
      "required": false
    },
    "format_conversion_bidirectional": {
      "check": "Conversion sub-spec handles both directions (dense→sparse AND sparse→dense)",
      "required": true
    },
    "scope_boundaries_clear": {
      "check": "Each child explicitly states what it does NOT handle",
      "required": true
    },
    "reunification_type_alignment": {
      "check": "Reunify step verifies that all children use the same matrix type interfaces",
      "required": true
    }
  },
  "pass_threshold": "all required markers present"
}
```

## Anti-signal

- Decomposition by file path instead of by mathematical concern
- Domain constraints duplicated in every child
- No shared type contract for matrix representations
- Nullspace and factorization lumped together (not orthogonal)
