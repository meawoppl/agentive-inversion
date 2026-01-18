# Unified Dockerfile for frontend + backend
# Expects pre-built artifacts from CI/CD pipeline
#
# Required environment variables:
#   DATABASE_URL          - PostgreSQL connection string
#   JWT_SECRET            - Secret for signing auth cookies (generate with: openssl rand -hex 32)
#   ALLOWED_EMAILS        - Comma-separated list of authorized user emails
#   GOOGLE_CLIENT_ID      - Google OAuth client ID
#   GOOGLE_CLIENT_SECRET  - Google OAuth client secret
#   AUTH_REDIRECT_URI     - OAuth callback URL (e.g., https://your-domain.com/api/auth/callback)
#   OAUTH_REDIRECT_URI    - Gmail API callback URL (e.g., https://your-domain.com/api/email-accounts/oauth/callback)
#
# Optional environment variables:
#   RUST_LOG              - Log level (default: info)
#   FRONTEND_DIR          - Frontend files path (default: /app/frontend/dist)
#   CORS_ALLOWED_ORIGINS  - Comma-separated allowed CORS origins

FROM debian:trixie-slim

RUN apt-get update && \
    apt-get install -y libpq5 postgresql-client ca-certificates && \
    rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy pre-built backend binary
COPY ./target/release/backend /app/backend

# Copy pre-built frontend dist
COPY ./crates/frontend/dist /app/frontend/dist

# Copy migrations and seed data
COPY migrations ./migrations
COPY diesel.toml ./
COPY seed-data.sql ./

# Copy startup script
COPY docker-entrypoint-backend.sh /app/entrypoint.sh
RUN chmod +x /app/entrypoint.sh

# Set frontend directory for the server
ENV FRONTEND_DIR=/app/frontend/dist

EXPOSE 3000

ENTRYPOINT ["/app/entrypoint.sh"]
