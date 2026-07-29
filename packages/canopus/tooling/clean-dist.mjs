import { rmSync } from "node:fs";

// Keep TypeScript output reproducible across local and CI builds.
rmSync(new URL("../dist", import.meta.url), { recursive: true, force: true });
