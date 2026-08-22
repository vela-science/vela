# Independent corrective re-review: maintained evidence qualification

## Verdict

**BLOCKED** for producer commit
`5e01d3e07951d9d231dd28f31bad1af785da2837`, tree
`f6db284ac94777d88774e52e1a306b750da4da14`, over parent
`27d368e6fbb111c1c65a51850e6da43596eabd50`.

The correction closes the prior environment, malformed-record, and lifecycle
findings represented by its 37-test suite. The exact documented test command
passes, and fresh isolated locked Python 3.11, 3.12, 3.13, and 3.14
environments with user-site packages disabled each pass all 37 tests. The
three-file scope is exact, formatting and diff checks pass, and Protocol 1 is
byte-identical at root
`sha256:6ca327d6ec6f56c051e236be9eee42629e1f67dce393c1518e051ff8c14b279e`.

The maintained custody boundary is still not fail closed in two cases,
however. A referenced bundle file with a hardlink alias outside the bundle is
accepted as `qualified_hold`, contrary to the documented rejection of every
hardlink alias. Separately, `consume_permit` can validate one inode and then
atomically link and return replacement bytes installed at the source name
before `os.link`. These are exact byte-custody failures in the surface this
tool claims to own.

This review modified no producer byte, called no provider or external service,
handled no credential, released no permit, and performed no merge, release,
authority, Decision, Standing, or scientific-record action.

## Exact binding and scope

- Producer ref: `origin/codex/evidence-harness-consolidation`
- Producer commit/tree:
  `5e01d3e07951d9d231dd28f31bad1af785da2837` /
  `f6db284ac94777d88774e52e1a306b750da4da14`
- Parent commit:
  `27d368e6fbb111c1c65a51850e6da43596eabd50`
- Prior BLOCKED review commit:
  `c1ecf70b7c0b2c52850697e0f2a8cc83c69e1fc1`
- Reviewed at: `2026-08-22T04:02:43Z`

The refreshed remote ref equals the immutable producer commit and advertised
tree. The producer is the direct child of the stated parent. The corrective
range changes exactly three files:

| Path | SHA-256 at producer |
| --- | --- |
| `tools/evidence_qualification/README.md` | `2a82a17183beeee3b8ebcebf178643b5248a3958bb4cdef20f03faba9184d6b6` |
| `tools/evidence_qualification/qualification.py` | `96f1ba1e994b0f17766c0eab591f1a006b4fed7da92dc49ab61fbfdacb9157e1` |
| `tools/evidence_qualification/test_qualification.py` | `38e684d631516bc4b41b1daae652803690147c9a0640d7a0da471481d498b69d` |

The clean detached producer checkout remained clean. No protocol schema,
conformance input, Rust crate, scientific record, or historical evidence
artifact changed.

## Corrective findings that pass

### Locked interpreter and documented fixture

The self-verification path now retains `sys.executable` without resolving the
virtual-environment symlink and requires all of these exact bindings:

- `sys.prefix` differs from `sys.base_prefix`;
- the executable is lexically inside the environment prefix;
- the imported `jsonschema` module is inside the same prefix;
- the command names that exact executable, current qualifier bytes, and exact
  canonical bundle path.

Fresh environments were created outside the repository from the locked
`conformance` project. Each was invoked directly with `PYTHONNOUSERSITE=1` and
`-s`:

| Python | Result |
| --- | --- |
| 3.11.15 | 37/37 PASS |
| 3.12.10 | 37/37 PASS |
| 3.13.3 | 37/37 PASS |
| 3.14.4 | 37/37 PASS |

The documented `BundleFixture` executed through the CLI and returned
`qualified_hold`, qualification root
`sha256:1035af2e60f15b1af25bf9a91d24ee866c01cf2770bb0ce0430e08fc70d2ae64`
for that exact temporary path, all eleven gates true, zero provider calls, zero
scientific sessions, and zero consumed participant permits. Repeated
qualification at one exact path is byte-identical; the receipt intentionally
binds its environment and canonical bundle path.

### Malformed records and lifecycle consistency

The regression suite now rejects the prior accepted adversaries:

- false, missing, or stale runner versions and stale configuration/runtime /
  Dockerfile/image roots;
- same-path, same-day, same-byte, malformed, wrong-account, or metadata-drifted
  account fixtures;
- comment-only Dockerfile controls and network package-manager instructions;
- missing, duplicate, extra, substituted, or reordered OCI graph members and
  invalid manifest descriptors;
- internal symlink traversal, in-bundle hardlink role aliases, and unsafe
  relative paths;
- forged, open-shape, boolean-attempt, cross-run, and cross-assignment permits;
- replayed/pre-consumed permits;
- forged launch/terminal/teardown bindings, stale launch roots, reversed event
  lifecycle, boolean usage, negative/reversed timing, and mismatched terminal,
  teardown, response, and capture roots.

The implementation uses closed field sets, exact schema constants, exact
identity/root bindings, monotone RFC 3339 UTC timestamps, nonnegative Decimal
durations, a four-event lifecycle, complete OCI member equality, and
descriptor-relative no-follow reads. Those checks are appropriate and the
registered valid fixture passes them.

## Blocking findings

### EQ-R1: out-of-bundle hardlink aliases are accepted

`validate_bundle_tree` records duplicate `(st_dev, st_ino)` identities only
among paths found while walking the bundle. It does not reject a regular file
whose `st_nlink` is greater than one when the other directory entry is outside
the bundle.

Independent reproduction created a valid `BundleFixture`, hardlinked
`schemas/registered.json` to a path in a separate temporary directory, and ran
the qualifier without changing any declared digest. The referenced file had
`st_nlink == 2`; qualification nevertheless returned:

```json
{
  "status": "qualified_hold",
  "qualification_root": "sha256:55b386a693f073d22840d098d4f4a78970042320e94cbb26370b60bbcbaece96"
}
```

This contradicts the README's exact claim that the bundle tree rejects every
hardlink alias and leaves a mutable alias to trusted role bytes outside the
validated tree.

Minimal correction: reject every regular bundle file with `st_nlink != 1`, in
addition to the existing duplicate-inode map, and add an adversary whose only
second link is outside the bundle.

### EQ-R2: permit validation and consumption are not one inode-bound action

`consume_permit` opens, reads, and validates the held permit, then closes its
descriptor. It subsequently calls descriptor-relative `os.link` by source
name. A replacement regular file installed at that name in between is the file
linked to the consumed name and returned as successfully consumed. The code
does not compare the consumed inode or bytes with the descriptor it validated.

A deterministic adversary replaced the source name immediately inside the
`os.link` call. `consume_permit` returned success; the consumed file contained
only `{"forged":true}`, while the valid bytes it had checked remained under a
different name. No overwrite or symlink was required.

This violates the claimed immutable, assignment-bound, fail-closed permit
consumption boundary. The concurrent one-winner regression does not exercise
source replacement between validation and linking.

Minimal correction: retain the validated source descriptor and its inode
through the link, open the new consumed name with no-follow semantics, require
matching device/inode and exact validated bytes before unlinking the source,
and fail closed with cleanup on any mismatch. Add a deterministic replacement
race regression.

## Checks completed

- PASS: refreshed remote commit/tree/parent and exact three-file scope.
- PASS: documented locked Python 3.13 suite, 37 tests.
- PASS: isolated locked Python 3.11–3.14, 37 tests on each minor, user site
  disabled.
- PASS: documented executable valid fixture and canonical receipt.
- PASS: locked Ruff check and Ruff format check.
- PASS: producer-range `git diff --check` and clean detached status.
- PASS: Protocol 1 verifier, 77 normative and 39 informative files, root
  `sha256:6ca327d6ec6f56c051e236be9eee42629e1f67dce393c1518e051ff8c14b279e`.
- PASS: no diff in `docs/PROTOCOL.md`, `schemas/`, or `conformance/` across the
  corrective range.
- BLOCKED: out-of-bundle hardlink alias received `qualified_hold`.
- BLOCKED: replacement bytes were returned as successfully consumed after
  different permit bytes were validated.

## Claim ceiling

The corrected commit supports a substantially stronger neutral qualifier: the
former locked-environment escape and the reviewed malformed record/lifecycle
acceptances are fixed, and the current fixture is deterministic and portable
across Python 3.11–3.14. It does not yet support the README's complete
hardlink-alias rejection or immutable one-shot permit-consumption claims.

Protocol 1 remains byte-identical. Qualification remains non-authoritative
tooling and creates no provider result, scientific acceptance, Repository
authority, Decision, Event, or Standing. Builder independence remains receipt
attestation rather than a build performed by this qualifier.
