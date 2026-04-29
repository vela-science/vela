# Reading

The Vela canon. Read these looking for **object models, adoption
lessons, trust failures, and missing primitives** — not as
exhaustive coverage of the field. The point is to know the shape of
the problem well enough that the substrate's design choices read as
inevitable.

## Synthesis

After working through this stack, the conclusion is straightforward:

1. GitHub's power came from object modeling, not hosting. The
   relevant primitives were repo, commit, issue, PR, review,
   release.
2. Science's atomic public object is still the paper.
3. The paper is too lossy to support AI-era science. It carries
   narrative, not state.
4. Existing science tools solve fragments. Projects (OSF), protocols
   (protocols.io), compute (Code Ocean), storage (Zenodo), search
   (OpenAlex), discussion (virological.org), data versioning
   (DataLad). None provides the missing layer.
5. The missing object is the **finding**: a structured assertion
   plus evidence, scope, provenance, confidence, contradictions,
   correction history, and downstream dependencies.
6. The missing system is shared scientific state with first-class
   verbs (asserted, reviewed, replicated, contested, revised,
   retracted) acting on first-class objects.
7. Vela is the protocol and registry layer for that state. Not
   another place to upload papers. Not a replacement for GitHub or
   figshare. The layer beneath them.

## Priority order

If you only have an afternoon, read these twelve in order:

1. **Mitchell Hashimoto, "Ghostty Is Leaving GitHub" (2026).** The
   live platform-trust collapse. The lesson: never let canonical
   state live only inside one product UI. https://mitchellh.com/writing/ghostty-leaving-github
2. **Armin Ronacher, "Before GitHub" (2026).** GitHub as social
   infrastructure, archive, trust layer. The post-GitHub problem
   isn't only decentralization; it's preserving project memory.
   https://lucumr.pocoo.org/2026/4/28/before-github/
3. **"Constellations of Borrowed Light" (essay).** The conceptual
   seed. Read for the argument; the prose is itself an iteration.
   https://borrowedlight.org
4. **Trevor Bedford, "Some thoughts on a GitHub of Science."** The
   cleanest early articulation of the GitHub-for-science idea —
   forkable papers, visible contribution, evolving state. https://bedford.io/blog/github-of-science/
5. **Trevor Bedford, "On scientific publishing practices in the
   face of public health emergencies."** The strongest counterread:
   maybe GitHub + virological.org + figshare + blogs is enough. Vela
   has to answer this. https://bedford.io/blog/scientific-publishing-practices/
6. **HN: "ResearchHub: GitHub for Science"** (read the comments). The
   skepticism is exactly right: "GitHub for science" without a real
   object model is just branding. https://news.ycombinator.com/item?id=27271366
7. **SoTA Letters, "AI Scientists Need a Social Network."** Closest
   current articulation of the agent-pressure case for substrate.
   https://sotaletters.substack.com/p/ai-scientists-need-a-social-network
8. **Nielsen & Qiu, "A Vision of Metascience."** Frames science's
   social processes as a design space; defines the metascience
   entrepreneur. https://scienceplusplus.org/metascience/
9. **"The Scientific Paper Is Obsolete" (Atlantic, 2018).** The
   Bret Victor / interactive-paper argument. Useful but
   insufficient — interactive papers are better renderings, not
   state. https://www.theatlantic.com/science/archive/2018/04/the-scientific-paper-is-obsolete/556676/
10. **FAIR Guiding Principles** (Wilkinson et al., 2016). The
    established language for findability, accessibility,
    interoperability, reusability. Vela should inherit and go
    beyond. https://www.nature.com/articles/sdata201618
11. **W3C PROV Overview.** Entities, activities, agents — the
    minimum grammar for trust. Provenance isn't a feature; it's
    part of the core scientific object. https://www.w3.org/TR/prov-overview/
12. **Nanopublications** (Mons et al., arXiv 2018). The closest
    historical cousin. Twenty years of work, real research
    community, never reached the bench scientist. The lesson is
    the adoption tax: structuring at write-time exceeded the
    perceived value. https://arxiv.org/abs/1809.06532

## Thematic shelves

### A. GitHub, post-GitHub, and open-source memory

Why GitHub mattered, what's breaking now, and what comes after.

- Mitchell Hashimoto, "Ghostty Is Leaving GitHub" (2026)
- Armin Ronacher, "Before GitHub" (2026)
- GitHub Blog, "An update on GitHub availability" (2025)
- GitHub Blog, "Bringing more transparency to GitHub's status page" (2026)
- ForgeFed (ActivityPub for software forges) — https://forgefed.org
- Forgejo (forge with federation as a primary goal) — https://forgejo.org
- Radicle (peer-to-peer Git, decentralized identity, gossip
  replication) — https://radicle.dev
- SourceHut (mailing-list-first, no AI features) — https://sourcehut.org
- Jujutsu / jj (Git-compatible local ergonomics) — https://github.com/jj-vcs/jj

**Vela translation:** scientific status, like software status, is
not binary. Findings need verbs: proposed, reviewed, replicated,
contradicted, deprecated, retracted, scope-limited.

### B. "GitHub for science" attempts and debates

Read these to understand why previous attempts feel shallow.

- Trevor Bedford, "Some thoughts on a GitHub of Science"
- Trevor Bedford, "On scientific publishing practices in the face
  of public health emergencies"
- HN: "We need a GitHub of Science" (2011)
- HN: "ResearchHub: GitHub for Science"
- ResearchHub "About"
- OSF (research-project collaboration layer)
- Code Ocean (compute capsules for reproducibility)
- protocols.io (methods versioning and sharing)

**Vela translation:** none of these is a *state layer*. They are
hosting, files, compute, methods, discussion. Vela is the missing
layer beneath.

### C. Scientific state, provenance, and object models

The closest technical shelf to Vela.

- Wilkinson et al., FAIR Guiding Principles (Sci. Data 2016)
- Research Objects (researchobject.org)
- RO-Crate (lightweight research-object packaging with metadata)
- W3C PROV-Overview
- Mons et al., Nanopublications (arXiv 2018)
- Nanopublications for contradictions (Heriot-Watt)
- OpenAlex (open catalog of the global research system)
- DataLad (distributed research data management)

**Vela translation:** Vela's content-address + signed-event model
inherits from PROV and nanopubs; the bet is that AI-era ingestion
makes the structuring tax tractable in a way nanopubs alone
couldn't.

### D. Open science, metascience, and process redesign

The philosophical and institutional shelf.

- Michael Nielsen, "Reinventing Discovery"
- Michael Nielsen, "The Future of Science"
- Nielsen & Qiu, "A Vision of Metascience"
- Collison & Nielsen, "Science Is Getting Less Bang for Its Buck"
  (Atlantic 2018)
- Park, Leahey & Funk, "Papers and patents are becoming less
  disruptive over time" (Nature 2023)
- Marblestone et al., "Unblock research bottlenecks with non-profit
  start-ups" (Nature 2022)
- Issues in Science and Technology, "Field Notes on Focused
  Research Organizations" (2025)

**Vela translation:** Vela is closer in shape to a public-good
utility than a venture-funded product. The substrate is
infrastructure, not a moat.

### E. Scientific publishing and better media

Separate "better paper" from "better state."

- "The Scientific Paper Is Obsolete" (Atlantic 2018)
- Bret Victor, "Media for Thinking the Unthinkable"
- Bret Victor, "Up and Down the Ladder of Abstraction"
- Distill, "The Building Blocks of Interpretability"
- Seemay Chou, "Scientific Publishing: Enough is Enough"
- Retraction Watch on stealth corrections

**Vela translation:** interactive papers are renderings. The
substrate is what's being rendered.

### F. Translation failure, negative results, correction propagation

The moral and empirical shelf behind Vela.

- Morris et al., "The answer is 17 years, what is the question?"
  (J. R. Soc. Med. 2011) — caveats the often-cited 17-year gap
- Franco, Malhotra & Simonovits, "Publication bias in the social
  sciences" (Science 2014) — file-drawer evidence
- Goldacre et al., EU clinical trial reporting (BMJ 2018)
- Raccuglia et al., "Machine-learning-assisted materials discovery
  using failed experiments" (Nature 2016)
- PubPeer (post-publication scrutiny in practice)

**Vela translation:** correction must travel as a signed event,
not a rumor. Negative results enter the substrate as state, not
absence.

### G. AI scientists, agents, and the pressure on scientific state

Why Vela becomes urgent now, not in twenty years.

- SoTA Letters, "AI Scientists Need a Social Network"
- Google Research, "Accelerating scientific breakthroughs with an
  AI co-scientist" (blog + arXiv:2502.18864)
- FutureHouse (AI agents for biology research)
- ToolUniverse (agent ecosystem for scientific tools)
- Dario Amodei, "Machines of Loving Grace" — vision piece, treat
  with appropriate skepticism
- Niko McCarty, "Levers for Biological Progress" (Asimov Press) —
  the grounded counter to Amodei

**Vela translation:** intelligence is not the bottleneck. State is.

### H. Collaboration, collective intelligence, public scientific labor

Not blueprints, but instructive on how scientific work becomes
networked.

- Tim Gowers, "Is massively collaborative mathematics possible?"
- Polymath Project
- Terence Tao on the first Polymath project
- Galaxy Zoo (Zooniverse)
- Foldit (PNAS 2011)

**Vela translation:** public contribution works when the task is
structured, credit is visible, and the object of contribution is
clear. Vela's object is the finding.

### I. Tools for thought and knowledge accumulation

For product and interface taste.

- Vannevar Bush, "As We May Think" (Atlantic, July 1945)
- Andy Matuschak, "Evergreen note-writing as fundamental unit of
  knowledge work"
- Andy Matuschak, "Knowledge work should accrete"
- Ink & Switch, "Local-first software" (2019)

**Vela translation:** scientific state should be portable,
mirrorable, and durable across platform decisions. Local-first by
design.

## What not to overread

There's a lot of DeSci writing around tokens, ResearchCoin,
bounties, decentralized publishing. Skim some of it for market
signal, but don't let it dominate.

The failure mode is obvious:

```
science + tokens + social feed + paper comments = noisy marketplace
```

That isn't Vela.

Vela is closer to:

```
scientific claim state + provenance + correction
+ replication + confidence + frontier map
```

## Why this list exists

The substrate's design choices read as inevitable only if you've
read the field. Otherwise they look arbitrary, or worse, fashion-
driven. This canon is the standing answer to *"why these primitives,
why this protocol, why now"* — for new contributors, future agents,
and anyone evaluating whether to depend on Vela.

When you cite a Vela design decision, cite the precedent or
counter-precedent. The bench scientist asking "why should I trust
this?" deserves the same answer as the metascience entrepreneur
asking "why hasn't this been built yet?"

The stars have always been there. What's missing is the structured
record of which ones we've already named.
