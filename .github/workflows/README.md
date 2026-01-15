# GitHub Actions Workflows

This directory contains automated workflows for Vixy.

## Workflows

### CI (`ci.yml`)

**Triggers:** Push to main, Pull Requests to main

Runs continuous integration checks:
- Format checking (`cargo fmt --check`)
- Linting (`cargo clippy`)
- Unit tests (`cargo test`)
- BDD tests (`cargo test --test cucumber`)
- Release build (`cargo build --release`)

### Docker Release (`docker-release.yml`)

**Triggers:** Git tags matching `v*.*.*` (e.g., `v0.1.2`, `v1.0.0`)

Builds and publishes multi-platform Docker images to GitHub Container Registry (GHCR).

**Platforms:** linux/amd64, linux/arm64

**Image tags generated:**
- `ghcr.io/chainbound/vixy:v0.1.2` (exact version)
- `ghcr.io/chainbound/vixy:0.1` (major.minor)
- `ghcr.io/chainbound/vixy:0` (major)
- `ghcr.io/chainbound/vixy:latest` (on main branch)

**How to trigger:**

1. Update version in `Cargo.toml`
2. Commit and push to main
3. Create and push a git tag:
   ```bash
   git tag -a v0.1.2 -m "Release v0.1.2"
   git push origin v0.1.2
   ```
4. The workflow automatically builds and pushes the Docker image

**Features:**
- Multi-platform builds (amd64, arm64)
- Layer caching for faster builds
- Automatic version extraction from git tags
- Published to GHCR with proper permissions

## Secrets

The workflows use built-in GitHub secrets:
- `GITHUB_TOKEN` - Automatically provided by GitHub Actions for GHCR authentication

No additional secrets need to be configured.

## Permissions

Workflows require these permissions:
- `contents: read` - Read repository contents
- `packages: write` - Push to GHCR

These are configured in each workflow file.
