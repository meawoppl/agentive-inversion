# Unified Dockerfile for frontend + backend
# Expects pre-built artifacts from CI/CD pipeline

FROM debian:trixie-slim

RUN apt-get update && \
    apt-get install -y libpq5 postgresql-client ca-certificates curl && \
    rm -rf /var/lib/apt/lists/*

# Install diesel_cli for running migrations
RUN curl -sSL https://github.com/diesel-rs/diesel/releases/download/v2.1.6/diesel_cli-2.1.6-x86_64-unknown-linux-gnu.tar.gz | tar -xz -C /usr/local/bin

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
