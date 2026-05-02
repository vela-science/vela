# Launch artifacts (draft, not shipped)

Internal staging document. Three artifacts ready for the day Will pushes the
button: HN launch post, Twitter thread, 60-second screencap script. No outreach
required — single-author publish, single domain.

---

## 1. Show HN post

**Title:**
> Show HN: Vela – an open protocol for scientific findings, signed and replayable

**Body:**

```
Hi HN — I've been building Vela, a content-addressed, signed substrate
for scientific findings, evidence, and corrections. The idea is the
"writeable layer" that science doesn't have: a place where a Phase II
failure, a retraction, or a confidence revision is a first-class event
that propagates through the dependency graph to every downstream claim
or trial that depended on it.

Three things I'd love HN to kick the tires on:

1. The 30-second proof. `curl -sSL https://vela.science/try | bash`
   downloads a real signed proof packet (~68 KB), recomputes SHA-256
   over every file in the manifest with a stdlib-only Python verifier,
   and exits non-zero on any mismatch. No install, no account.

2. Three independent reducer implementations (Rust, Python, TypeScript)
   that agree byte-for-byte on the canonical state for any given event
   log. The Python and TypeScript ones are auditable in one sitting.
   Source at /vela_reducer.py and /vela_reducer.mjs on the domain.

3. A worked example: /proof/pericyte-correction. A real Correction event
   landing on a real finding, propagating through real dependency edges
   to a real downstream object (a Phase II trial's enrollment criteria).
   The substrate doing the only thing the essay says it must do.

What's NOT here yet: external signers. /honest names this directly —
every signature on the live hub today is by a key under my control.
Year-one milestone is recruiting credentialed reviewer-signers; until
then this is technically real and socially unproven.

Why now: AI is producing scientific candidates faster than any prior
generation of tools, and every output that ends in a private log
instead of a shared record is a public good lost. The substrate either
gets built as open protocol in the next two to three years or it gets
built as proprietary layers every model has to license to function.

Essay (16 min): https://borrowedlight.org
Workbench: https://vela.science/workbench
Operational scope: https://vela.science/scope
Protocol spec: https://vela.science/spec
Repo: <REPO_URL>

Happy to answer anything technical about the protocol, the reducer
byte-equality property, the signature scheme, or what would have to be
true for this to be wrong.
```

**When to post:** Tuesday or Wednesday, 9–11am ET. Avoid Monday morning
(too noisy) and Friday (dies on weekend). Do not post during a major news
cycle.

**First-comment plan:** Pre-write three comments to drop in the first
ten minutes:
- One technical detail clarification (probably about the byte-equality
  conformance test or the signing scheme — choose based on which gets
  the first question)
- One "here's the failure mode I'm most worried about" honesty post
- One "if you're a credentialed scientist in [adjacent field] and want
  to try depositing, here's how" CTA

---

## 2. Twitter / X thread

10 posts. Drop one image at posts 1, 3, 5, 7, 9 — screencap of the
constellation, the curl|bash output, the pericyte timeline, the workbench
drill-in, the operational scope page. Use the new sail-glyph + serif
wordmark for the closing card.

**1/**
```
Most projects publish a launch page that reads as if everything in the
architecture diagram already exists.

This one is different. Here's what's actually true on a Tuesday →

vela.science
```

**2/**
```
Vela is a content-addressed, signed substrate for scientific findings,
evidence, and corrections.

A Phase II failure isn't a footnote. It's a first-class event that
propagates through every downstream claim or trial that depended on
the original.
```

**3/**
```
30-second proof:

  $ curl -sSL https://vela.science/try | bash

Downloads a real signed proof packet (~68 KB) and a stdlib-only Python
verifier. Recomputes SHA-256 over every file in the manifest. Exits
non-zero on any mismatch.

No install. No account.
```

**4/**
```
Three independent reducer implementations — Rust, Python, TypeScript —
agree byte-for-byte on the canonical state for any event log.

This is the load-bearing property: same input bytes, same output bytes,
in three languages, on every machine, for as long as the protocol
version holds.
```

**5/**
```
Worked example: a real Correction landing on a real finding.

Pericyte loss → BBB breakdown → APOE4 narrowing (Montagne 2020).

In the substrate, the Correction propagates to three downstream objects:
a target hypothesis, a Phase II inclusion criterion, a review headline.

vela.science/proof/pericyte-correction
```

**6/**
```
The hub is dumb signed transport. Anyone with an Ed25519 key can
publish their own frontier. There's no allowlist, no rate limit, no
gatekeeper.

The signature is the bind.

Any reducer in the world can replay the event log to byte-identical
state.
```

**7/**
```
Five minutes from empty folder to first signed finding on the public hub:

  vela init
  vela add finding
  vela sign
  vela publish

Full walkthrough: vela.science/quickstart
```

**8/**
```
What's NOT true yet: external signers.

vela.science/honest names it directly — every signature on the live hub
today is by a key under my control.

Year-one milestone is credentialed reviewers depositing real findings.
Until then, technically real, socially unproven.
```

**9/**
```
Why now: AI is producing scientific work faster than any prior
generation of tools.

Every output that lands in a private log instead of a shared record
is a public good lost.

The substrate gets built as open protocol in the next 2–3 years or as
proprietary layers every model has to license to function.
```

**10/**
```
Essay (16 min): borrowedlight.org

Operational scope (FRO model): vela.science/scope

Protocol spec: vela.science/spec

Repo: <REPO_URL>

If you're a credentialed scientist in any field that hates relearning
the same map, please email — the corridor opens with you.
```

---

## 3. 60-second screencap script

Record with QuickTime (Cmd+Shift+5, "Record selected portion"). Resolution:
1920×1080. Window: Chrome, vela.science. Cursor: large + visible
(System Settings → Accessibility → Display → Pointer Size).

**Soundtrack:** none. Silence reads more serious than royalty-free music.

**Captions:** burned-in, bottom-third, JetBrains Mono, white on
semi-transparent navy.

### Beat sheet

| Time | Action | Caption |
|------|--------|---------|
| 0:00–0:04 | Open Terminal. Type `curl -sSL https://vela.science/try \| bash` | "30-second proof" |
| 0:04–0:14 | Hit enter. Output streams: download, verify, 35/35 OK | "stdlib-only verifier · no install · no account" |
| 0:14–0:18 | Switch to Chrome, vela.science homepage | "the substrate" |
| 0:18–0:25 | Scroll to constellation viz. Hover a node. Tooltip shows claim | "188 findings on the live frontier" |
| 0:25–0:30 | Click into a finding. Drill down to evidence chain | "every claim is content-addressed and signed" |
| 0:30–0:40 | Switch to /proof/pericyte-correction. Show the timeline | "a real Correction propagating through real dependencies" |
| 0:40–0:48 | Switch to /workbench. Show the live state | "anyone can replay the canonical state in three languages" |
| 0:48–0:55 | Switch to /honest. Show "what isn't true yet" | "operator voice. no promoter voice." |
| 0:55–0:60 | Switch to /scope. Title visible | "vela.science · build the substrate before someone closes it" |

End on the title card for ~2 extra seconds before fade.

### Recording checklist
- [ ] Hide system menu bar (cmd+space → "Hide menu bar")
- [ ] Disable notifications (Focus → Do Not Disturb)
- [ ] Close all other Chrome tabs
- [ ] Pre-load every URL so transitions are instant
- [ ] Test the curl command beforehand to make sure 35/35 verifies
- [ ] Use one clean Terminal window with nothing in scrollback
- [ ] Record at least three takes; the cursor jitter will haunt the first one

### Distribution
- Embed on /try page (top of fold, autoplay muted, loop)
- Embed in HN post via screencap.com or asciinema for terminal portion
- Drop as native MP4 in Twitter thread post 3
- Include in Astera essay submission as supplementary material if format allows

---

## When to push the button

Not yet. Push the button after:

1. /scope, /quickstart, /sdk, /spec, /faq are live (this commit ships them)
2. The pericyte page autoplay timeline is shipped (next commit)
3. You've recorded the 60-second screencap and it embeds cleanly
4. You've re-read the essay one more time fresh

Earliest realistic launch window: this Friday or next Tuesday. Don't
launch on a day when you can't sit at the keyboard for 6 hours straight
to answer comments.

---

*This file is internal. It does not get shipped to vela.science.*
