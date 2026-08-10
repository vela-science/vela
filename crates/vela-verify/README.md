# vela-verify

Frozen, independent **exact verifiers** for combinatorial and
coding-theory witnesses.

A discovery proposer (human or agent) is untrusted: it returns an explicit
construction, and this crate re-checks it deterministically before any
claim is recorded. Corrupting a witness must fail the verifier — that is
the property the tests pin. This is the reference registry `vela reproduce`
re-runs stored witnesses through.

The verifiers are pure (no I/O, no randomness) and dependency-light (serde
only), so a third party gets byte-identical verdicts.

## Verifiers

Every `kind` the crate accepts, in `enum Witness` order.

| kind | check |
|------|-------|
| `sidon` | `{0,1}^n`, all pairwise sums distinct |
| `golomb` | integer marks, all pairwise differences distinct |
| `cap` | `F_3^n`, no three points collinear |
| `bh` | `{0,1}^n`, all `h`-fold sums distinct |
| `covering` | `C(v,k,t)`, every `t`-subset covered |
| `constant_weight` | `A(n,d,w)`, weight `w`, pairwise distance `>= d` |
| `costas` | permutation, displacement vectors distinct |
| `gf2_sidon` | `GF(2)^n` (A394031), all pairwise XORs distinct |
| `union_free` | A347025, no member is a union of the others; lower bound |
| `rook_directions` | A321531, distinct rook-pair direction classes; lower bound |
| `linear_code` | `[n,k,d]_q` (q prime), min weight `>= claimed_d` |
| `interval_product` | Erdős #1056, consecutive cuts with interval product `== 1 (mod p)` |
| `balanced_coloring` | Erdős #617, every `(r+1)`-subset of `K_n` sees all `r` colors |
| `crt_partial_cover` | Erdős #203, prime rows pinning orders of 2 and 3 on an affine line |
| `kummer_no_carry` | Erdős #684, zero Kummer carries force `f(m-1) > k` |
| `min_binom_gcd` | Erdős #700, `min_k gcd(n, C(n,k))` rederived through Kummer |
| `binom_deficiency` | Erdős #1093, ELS93 deficiency `delta(N,k)` recomputed |
| `binom_exception_enum` | Erdős #1094, candidate re-enumeration equals the claimed exception set |
| `unsat_cert` | CNF plus an LRAT proof replayed to the empty clause (RUP only) |
| `diff_triangle` | difference triangle set, all within-row differences globally distinct |
| `unit_fraction_decomp` | Erdős #242, `4/n = 1/x + 1/y + 1/z` complete over `[3, n_max]` |
| `distinct_partial_sums` | Erdős #475, distinct partial sums mod `p` for every nonempty subset |
| `powerful_triples_none` | Erdős #364, no three consecutive powerful integers in `[1, n_max]` |
| `two_full_three_full` | Erdős #366, exactly the 2-full `n` with 3-full `n+1` in `[1, n_max]` |
| `brocard_no_square` | Erdős #398, `n! + 1` square only for `n ∈ {4,5,7}` over `[n_min, n_max]` |
| `semiprime_egyptian` | Erdős #306, Egyptian expansions into distinct squarefree semiprimes |

Several kinds certify a **lower bound** or a **finite confirmation** rather
than a universal statement; the per-variant documentation on `enum Witness`
states which, and the verifier's own message repeats it.

## Witness format

```json
{ "kind": "sidon", "n": 8, "points": [[0,1,1,0,0,0,1,1], ...], "claimed_size": 33 }
```

`claimed_size` (where present) is cross-checked: the construction must
pass AND have exactly that many elements, so a record can't claim a larger
set than the witness it ships.

A verified witness can also be bound to an exact claim, through the library
rather than a command:

```rust
vela_verify::claim_witness_faithful(
    "There exists a Sidon subset of {0,1}^8 with at least 33 elements.",
    &witness,
)
```

The claim check fails closed on a wrong kind, dimension, parameter, or lower
bound. A construction witness cannot establish equality or optimality.

There is no `vela-verify` command. The crate built one, but no release ever
shipped it — `scripts/release.sh` stages `vela` alone and `install.sh` installs
`vela` alone — so it was reachable only from a source checkout. `vela reproduce`
is the command surface.
