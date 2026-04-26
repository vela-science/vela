// @ts-check
import { defineConfig } from 'astro/config';

// https://astro.build/config
export default defineConfig({
  // vela.science is owned by a third party; live site is at vela-site.fly.dev.
  // Keep in sync with site/src/config.ts SITE_URL.
  site: 'https://vela-site.fly.dev',
  // Single static site. Pages render at build time; islands hydrate
  // client-side only where explicitly opted into.
  output: 'static',
  trailingSlash: 'never',
  build: {
    format: 'directory',
  },
  prefetch: {
    prefetchAll: true,
    defaultStrategy: 'viewport',
  },
});
