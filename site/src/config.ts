// Single source of truth for site-wide URLs and brand strings.
// Components and pages import from here; nothing else should hardcode
// these values. Changing the hub URL, the repo URL, or the BBB
// frontier identity touches one file.

export const SITE_URL = "https://vela.science";
export const HUB_URL = "https://vela-hub.fly.dev";
export const WORKBENCH_URL = "https://vela-workbench.fly.dev";
export const REPO_URL = "https://github.com/vela-science/vela";
export const ARCHIVE_URL = "https://github.com/willblair0708/vela-archive";

// Borrowed Light is the long-form essay this substrate exists to make
// concrete. It's the recruiting layer; Vela is the working substrate
// the essay argues for. They cross-link aggressively but live at
// separate URLs because they serve different audiences.
export const BORROWED_LIGHT_URL = "https://borrowedlight.org";

// Raw-content base used by `network_locator` fields and CI workflows.
export const REPO_RAW_BASE = "https://raw.githubusercontent.com/vela-science/vela/main";

// Public BBB frontier metadata. The `vfr_id` is intentionally absent
// here — it is content-addressed and changes with content, so we
// resolve it at runtime by listing the registry. See LiveHub.astro.
export const BBB = {
  name: "BBB Flagship",
  description:
    "48 signed findings about blood-brain barrier translation in Alzheimer's research.",
  locator: `${REPO_RAW_BASE}/frontiers/bbb-alzheimer.json`,
};

export const VERSION = "0.14";
