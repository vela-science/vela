---
description: The triage room — walk the sign queue, record the human's verdicts, hand off one ceremony
---

# /vela:review

You are running the triage room for the human's sign queue. Two invariants
override everything else in this command:

- **You never run `vela sign`, never touch a key, and never suggest, default,
  or pre-select a verdict.** The verdict is the human's. You are the clerk who
  presents the evidence and records what they say — nothing more.
- **The terminal ceremony is the authority.** `vela sign` re-renders every item
  independently from frontier state; it does not trust this chat. Answers you
  record here only pre-fill the session file it reads.

Steps:

1. Fetch the queue: `vela sign --frontier . --json`. Shape:
   `{ok, frontiers: [{frontier, items: [{lane, id, title, why_here, signable,
   pack, preview}]}], signable_total}`.
2. If `signable_total` is 0, say the queue is clear and stop.
3. Read `.vela/sign-session.json` if it exists — the resume file, shaped
   `{"answers": {"<id>": "accept" | "reject:<reason>" | "yes"}}`. Items whose
   id already has an answer are done; report the resume state ("3 of 5 already
   answered") and skip them unless the user asks to revisit one.
4. Walk the remaining signable items **one at a time**. For each:
   - Present the claim headline (`title`), the `preview` lines verbatim
     (evidence, prior, caveats), `why_here`, and the lane. The content decides,
     so show the content — do not compress the preview away.
   - Ask for the verdict. AskUserQuestion is natural here. Decision-lane items
     take accept / reject / skip (a reject needs a reason — ask for it);
     hygiene and judgment lanes take yes / skip. Present the options neutrally,
     in the same order every time, with no recommendation attached.
5. Record each verdict as it is given (not batched at the end — the walk must
   survive an interruption). Merge into `.vela/sign-session.json` without
   clobbering answers already there. Use Bash with python3, passing the id and
   answer as arguments so reasons with quotes survive:

   ```bash
   python3 - "<id>" "<answer>" <<'EOF'
   import json, os, sys
   path = ".vela/sign-session.json"
   state = {"answers": {}}
   if os.path.exists(path):
       with open(path) as f:
           state = json.load(f)
   state.setdefault("answers", {})[sys.argv[1]] = sys.argv[2]
   with open(path, "w") as f:
       json.dump(state, f, indent=2)
       f.write("\n")
   EOF
   ```

   Answer strings, exactly: `accept`, `reject:<reason>`, `yes`. A skip records
   nothing.
6. When the walk ends — all items answered or the user stops — close by handing
   exactly one command to run in the terminal:

   ```
   vela sign
   ```

   And state the wallet invariant in plain words: the ceremony re-renders every
   item independently from state, takes the one confirm and the one key read,
   and nothing is signed until the key speaks. What happened here only saved
   the human the retyping.
