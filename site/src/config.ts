// Single source of truth for site-wide URLs and brand strings.
// Components and pages import from here; nothing else should hardcode
// these values. Changing the hub URL, the repo URL, or the BBB
// frontier identity touches one file.

// vela.science is registered to a third party (since 2014); the live
// site is at vela-site.fly.dev until/unless we acquire a real domain.
// Update both this and astro.config.mjs `site:` together if that lands.
export const SITE_URL = "https://vela-site.fly.dev";
export const HUB_URL = "https://vela-hub.fly.dev";
export const WORKBENCH_URL = "https://vela-workbench.fly.dev";
// v0.34.1: substrate source moved private. The public mirror at
// `vela-science/vela-frontiers` carries the locator-bound published
// artifacts (frontier JSONs, VelaBench gold + scored results) so
// `vela registry pull` continues to verify against a public URL.
// `REPO_URL` points at the public artifact mirror; the substrate
// source is private until it reaches a public-ready state.
export const REPO_URL = "https://github.com/vela-science/vela-frontiers";

// Borrowed Light is the long-form essay this substrate exists to make
// concrete. It's the recruiting layer; Vela is the working substrate
// the essay argues for. They cross-link aggressively but live at
// separate URLs because they serve different audiences.
export const BORROWED_LIGHT_URL = "https://borrowedlight.org";

// Raw-content base used by `network_locator` fields and CI workflows.
// v0.34.1: pinned to the public mirror.
export const REPO_RAW_BASE = "https://raw.githubusercontent.com/vela-science/vela-frontiers/main";

// Public BBB frontier metadata. The `vfr_id` is intentionally absent
// here — it is content-addressed and changes with content, so we
// resolve it at runtime by listing the registry. See LiveHub.astro.
export const BBB = {
  name: "BBB Flagship",
  description:
    "48 signed findings about blood-brain barrier translation in Alzheimer's research.",
  locator: `${REPO_RAW_BASE}/frontiers/bbb-alzheimer.json`,
};

// Topic-first display identity. The protocol-side directory stays
// `bbb-flagship` for stable id; the site renders it under its
// scientific subject. When the underlying frontier is re-published
// under a renamed canonical name + new vfr_id, this constant is what
// readers see, and a single line moves with it.
export const FRONTIER = {
  name: "Alzheimer's Therapeutics",
  slug: "alzheimers-therapeutics",
  description:
    "The live state of Alzheimer's therapeutics — drug targets, mechanisms, clinical readouts. Signed, content-addressed, agent-augmented.",
  // Path on disk (relative to repo root) where the canonical frontier lives.
  // Used at build time by the site loader; not exposed at runtime.
  repoPath: "projects/bbb-flagship",
  // Public manifest filename (under `frontiers/` in the repo).
  // The hub locator is `${REPO_RAW_BASE}/frontiers/${manifestFile}`.
  manifestFile: "alzheimers-therapeutics.json",
};

export const VERSION = "0.35.1";
