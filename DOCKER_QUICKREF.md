# Docker Quick Reference

First complete the authentication setup and Linux artifact build in
[DOCKER.md](DOCKER.md). The image requires both `backend` and `agent-cli` in
`target/release`; native macOS binaries are not compatible.

```bash
# In a compatible Linux build environment
(cd crates/frontend && trunk build --release)
cargo build --release --locked --bin backend --bin agent-cli

# From the repository root, with .env configured
docker compose up --build
```

Visit **http://localhost:3000**. The app serves both UI and API; there is no
separate frontend container. Google login is required, and sample data is not
loaded automatically.

```bash
# Logs
docker compose logs -f app

# Database shell
docker compose exec postgres psql -U testuser -d agentive_inversion

# Stop, preserving database data
docker compose down

# Deliberately reset database data
docker compose down -v
```

For source changes, rebuild the assets and both Linux binaries before rebuilding
the image. See [QUICKSTART.md](QUICKSTART.md) for native development with Trunk.
