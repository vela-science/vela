# TypeScript protocol boundary

Internal TypeScript contracts for Vela's cross-language interoperability waist.

The package provides canonical JSON encoding, SHA-256 roots, bounded validation
helpers, and types for current Submission and Verification records. It does not
contain repository authority, Decision, signing-custody, or accepted-state
mutation APIs.

```ts
import { canonicalJcs, protocolDigest, type SubmissionV1 } from "@vela-science/protocol";
```

Rust remains the normative implementation. The package is private until a real
external consumer needs a registry release. It remains the single
authority-free TypeScript contract surface; Vela does not maintain a parallel
handwritten JSON Schema copy. Cross-implementation fixtures and independent
readers live under `conformance/`.
