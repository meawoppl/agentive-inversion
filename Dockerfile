# Single-binary image: the Yew frontend is embedded into the backend binary by
# memory-serve at build time, and migrations are embedded by diesel_migrations
# and applied by the server on startup. CI compiles the binary, so there is no
# Rust build stage here.
#
# Required environment variables:
#   DATABASE_URL          - PostgreSQL connection string
#   JWT_SECRET            - Secret for signing auth cookies (generate with: openssl rand -hex 32)
#   ALLOWED_EMAILS        - Comma-separated list of authorized user emails
#   GOOGLE_CLIENT_ID      - Google OAuth client ID
#   GOOGLE_CLIENT_SECRET  - Google OAuth client secret
#   AUTH_REDIRECT_URI     - OAuth callback URL (e.g., https://your-domain.com/api/auth/callback)
#
# Optional environment variables:
#   HOST                  - Bind address (default: 0.0.0.0)
#   PORT                  - Bind port (default: 3000)
#   RUST_LOG              - Log level (default: info)
#   ANTHROPIC_API_KEY     - Enables the agentic triage pipeline (else keyword fallback)
#   CORS_ALLOWED_ORIGINS  - Comma-separated allowed CORS origins

FROM debian:trixie-slim

RUN apt-get update && \
    apt-get install -y ca-certificates libpq5 libssl3 curl && \
    rm -rf /var/lib/apt/lists/*

# Node.js + Claude Code CLI for the agentic triage pipeline. Without these
# (or without ANTHROPIC_API_KEY) the backend runs in keyword-fallback mode.
RUN curl -fsSL https://deb.nodesource.com/setup_22.x | bash - && \
    apt-get install -y nodejs && \
    npm install -g @anthropic-ai/claude-code && \
    rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy pre-built backend binary (frontend assets and migrations are embedded)
COPY ./target/release/backend /app/backend

# Agent CLI used by triage agent sessions (must be on PATH)
COPY ./target/release/agent-cli /usr/local/bin/agent-cli

# Create non-root user. The triage pipeline shells out to the claude CLI, which
# needs a writable HOME for ~/.claude.
RUN useradd -m -u 1001 -s /bin/bash appuser && \
    chown -R appuser:appuser /app

USER appuser
ENV HOME=/home/appuser

EXPOSE 3000

# Surface crash-loops (e.g. a failed migration) as an unhealthy container
# instead of a silent restart cycle
HEALTHCHECK --interval=30s --timeout=5s --start-period=300s --retries=3 \
  CMD curl -fsS http://localhost:3000/health || exit 1

CMD ["/app/backend"]
