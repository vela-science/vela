// @ts-check
import { defineConfig } from 'astro/config';

// https://astro.build/config
export default defineConfig({
  site: 'https://vela.science',
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
