# Static site for vela.science.
#
# Astro builds dist/ on the host (or in CI); the container ships only
# nginx + the prebuilt files. Tiny image, no node, no symlink resolution
# at container-build time.
#
# Build sequence:
#   cd site && bun install && bun run build
#   flyctl deploy --config site/fly.toml --dockerfile site/Dockerfile site

FROM nginx:alpine
COPY dist /usr/share/nginx/html
COPY nginx.conf /etc/nginx/conf.d/default.conf
EXPOSE 8080
