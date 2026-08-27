# Vela organization: current roadmap

Status: canonical Now, Next, and Later order for VELA-ORG-1
Observation cutoff: 2026-08-27

This roadmap summarizes the evidence gates in
[the migration architecture](VELA_ORG_MIGRATION.md). It grants no permission to
publish, deploy, change provider state, rewrite authority records, or alter
Protocol 1, schemas, release bytes, and product behavior.

## Now

1. Keep Problems source and production frozen at
   `532241ba5db565e9ee35e13cbd7eff76393f6475` through a real WebMCP challenge
   submission and its subsequent 24-hour exact-SHA stability window.
2. Preserve signed, immutable Vela `0.977.5` as historical custody and signed,
   immutable Vela `0.977.6` as the current release. Keep the completed Workbench
   and Math `0.977.6` repins bound to their reviewed digests and exact commits.
3. Leave the current projection, `vela.space`, rollback projects, DNS, legacy
   Observatory data, repository metadata, and rights-sensitive assets unchanged
   while the challenge freeze remains open.

The submission receipt and full stability window are the exit gate from Now.

## Next

1. Make projection reconstruction self-contained at its declared public and
   private boundary. Remove the authenticated `vela-web` adapter dependency
   after custody, exact-input, root, and rollback evidence pass review.
2. Close the asset rights ledger. Resolve the six ITF/Fontshare files, include
   the complete IBM Plex notice where required, and keep licensed private UI
   material outside public extraction.
3. Qualify a new projection release against signed Vela `0.977.6` without
   rewriting the current `0.977.3` projection receipt or changing Standing.

Projection reconstruction and licensing closure are the exit gates from Next.

## Later

1. Extract a tiny rights-safe `vela.space` orientation surface from private
   transitional `vela-web`. Prove route, content, asset, redirect, metadata,
   accessibility, deployment, and rollback parity before any public cutover.
2. Request user approval for each DNS, hook, alias, homepage, rollback-window,
   repository, stash, worktree, local installation, or legacy-data change.
3. Refresh the current architecture and deployment inventory after approved
   changes. Retain Observatory as searchable historical terminology rather than
   a current product or new protocol name.

The organization reaches the target after every completion property in
[the target architecture](VELA_ORG_TARGET.md) has exact evidence.
