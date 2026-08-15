# AGENTS.md — Repo hygiene

This repo follows the branch/release workflow documented in `CONTRIBUTING.md`
— read and follow it for any git, branch, or release work here (the
single-trunk `main` model, `feature/x`/`fix/x` branch naming, RC-tag-driven
beta releases, etc). Don't improvise a different workflow. There is no
`dev` or `beta` branch — that three-branch model was retired.

## Remotes
- `origin` — Forgejo (`git.breadway.dev` via Hestia, SSH) — authoritative.
- `github` — GitHub mirror. Push both when publishing.

## CI
- `check.yml` — clippy + test, triggers on push to `feature/**`/`fix/**`.
- `dev-release.yml` — triggers on push to `main`.
- `rc-release.yml` — triggers on `vX.Y.Z-rc.N` tag push.
- `release.yml` — triggers on any other `v*` tag push.

All four run on a self-hosted runner (`hestia`) inside a pinned Arch
container — not the host's native environment. The Containerfile/build
script are shared across bread-ecosystem products and live in
`bread-ecosystem/ci/`; this repo's `ci/build.sh` clones that repo at the
sha in `ci/bread-ecosystem.rev` (deliberately pinned, not `main`) and
delegates to it. Nothing runs automatically on plain commits or PRs
beyond what's listed.

## Don't
- Don't embed credentials in remote URLs — SSH or a credential helper only.
