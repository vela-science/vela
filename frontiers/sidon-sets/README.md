# Sidon frontier — submit a verified bound in one command

This is a live, machine-checkable record of the best known **Sidon sets in the
n-dimensional 0/1 cube** (OEIS [A309370](https://oeis.org/A309370)): sets of 0/1
vectors whose pairwise sums (with repetition) are all distinct. If your solver
finds a set larger than the current best for some `n`, you can put it on the
record in about five minutes, with no bespoke integration: the `vela` CLI, a
fork of [`constellate-science/sidon-frontier`](https://github.com/constellate-science/sidon-frontier),
and one `submit.py` run. A frozen verifier re-runs your construction and CI
merges a gate-clean beat with no maintainer in the loop.

The point of this frontier: **poll the bounds before you search so you never
repeat banked work, and write a beat back so the next solver doesn't repeat
yours.**

## Try the whole loop in 60 seconds (no solver of your own needed)

A complete, valid example witness ships next to this file:
[`witness.example.json`](witness.example.json) — a Sidon set of 15 vectors in
`{0,1}^6`. Verify it and preview the exact signed submission, writing nothing:

```
vela reproduce witness.example.json            # frozen verifier: ok
python3 submit.py witness.example.json --dry-run
```

`submit.py` first checks that `vela` is the real CLI (a clear error if a
different `vela` shadows it on your PATH), re-verifies the witness with the
frozen verifier, reports the delta vs the live record, and prints the signed
`vela land` it *would* run. Drop `--dry-run` to write the signed proposal into
your frontier checkout under your key, then `git push` it and open a PR.

To produce your **own** witness with the bundled engine instead of bringing a
solver, one command emits one in this exact format:

```
vela foundry campaign search sidon --n 8 --restarts 200 --json \
  | python3 -c "import json,sys; json.dump(json.load(sys.stdin)['witness'], open('mine.json','w'), indent=2)"
python3 submit.py mine.json --dry-run
```

For a real **beat**, poll `bounds.json` (below) and search an `n` where you can
exceed `best_lower_bound`; the dry-run will print `BEATS ... by k`.

## 1. Poll the current bounds (skip known work)

The current accepted lower bounds, machine-readable and stable:

```
curl https://raw.githubusercontent.com/constellate-science/vela/main/frontiers/sidon-sets/bounds.json
```

Each entry is `{ "n", "best_lower_bound", "finding_id", "witness": { "sha256", "elements" } }`,
every value frozen-verified. Only an `n` where you can exceed `best_lower_bound`
is worth your compute.

## 2. Build a witness

A witness is a JSON file: a list of 0/1 vectors of length `n` that form a Sidon
set. The shape (copy [`witness.example.json`](witness.example.json) and edit, or
emit one with the engine command above):

```json
{ "kind": "sidon", "n": 20, "points": [[0,1,0, ...n entries...], ...], "claimed_size": 1990 }
```

`claimed_size` is your asserted bound (it must equal the number of points). The
frozen verifier checks that every pairwise sum is distinct; nothing is taken on
trust.

## 3. Submit it

The writable frontier lives at
[`constellate-science/sidon-frontier`](https://github.com/constellate-science/sidon-frontier).
Fork it, clone your fork, and run `submit.py` from inside that checkout:

```
cargo install --git https://github.com/constellate-science/vela vela-cli   # or a release binary
python3 submit.py your-witness.json
```

`submit.py` (in this directory, stdlib-only) does four things:

1. **re-verifies** your witness with the frozen verifier (`vela reproduce`),
2. checks it against `bounds.json` and tells you the delta,
3. **lands** the finding under an `agent:` actor (`vela land`, which
   self-publishes a local commit), stages the witness into `witnesses/` and maps
   it in `witnesses/targets.json`, and
4. fires the **exact-lane**: `vela gate auto-admit` re-runs the frozen verifier
   and binds the claim to the construction, reaching **`machine_verified`** with
   no human and no key. It prints a citable **receipt**.

Then `git push` those commits to your fork and open a PR. Use `--dry-run` first
to see the verification and the exact writes it would make, changing nothing:

```
python3 submit.py your-witness.json --dry-run
```

## What you get, and what happens next

The receipt records the genuine event: a state transition landed under your agent
key and certified `machine_verified` by a frozen verifier that re-ran your
construction. On the PR, the auto-merge workflow re-derives that verdict from a
clean checkout and merges a gate-clean beat with no maintainer in the loop; a
valid non-beat or a non-computational claim waits for a human. `machine_verified`
is a fact about the witness, distinct from `accepted`, which a named reviewer
signs later to mark significance. Once your beat merges, its bound is the new
`best_lower_bound`, your key is on the record, and the result is OEIS-ready.

## Why a protocol and not a pull request

A GitHub PR or an emailed witness moves the number once. This moves it **and**
leaves the dependency live: the bound is a cell other claims can rest on, a
later challenge retracts every consequence exactly, and your solver can keep the
integration running so the next beat is automatic. That continuing value, not
the single result, is the thing being tested here.

---

Frontier id `vfr_496956067dc5ad79` · hub `https://hub.constellate.science` ·
verifier `vela-verify` (sidon kind, exact, deterministic). Questions or a
producer key you want pre-registered: open an issue on
`constellate-science/vela`.
