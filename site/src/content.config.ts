import { defineCollection, z } from "astro:content";
import { glob } from "astro/loaders";

// Essays — long-form prose about the Vela substrate. The .md files live
// in essays/ at the repo root and are symlinked into src/content/essays.
const essays = defineCollection({
  loader: glob({ pattern: "**/*.md", base: "./src/content/essays" }),
  schema: z.object({
    title: z.string().optional(),
    description: z.string().optional(),
    pubDate: z.coerce.date().optional(),
    draft: z.boolean().optional().default(false),
  }),
});

// Docs — technical reference markdown shared with the rest of the repo.
// Only the slugs explicitly listed in src/pages/docs/ get rendered as
// public pages; the rest are ignored at routing time.
const docs = defineCollection({
  loader: glob({ pattern: "**/*.md", base: "./src/content/docs" }),
  schema: z.object({
    title: z.string().optional(),
    description: z.string().optional(),
  }),
});

export const collections = { essays, docs };
