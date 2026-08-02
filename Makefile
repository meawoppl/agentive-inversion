.PHONY: help frontend build docker-up docker-down docker-rebuild docker-logs docker-clean test fmt clippy audit lint

help:
	@echo "Available commands:"
	@echo "  make frontend        - Build the frontend (required before the backend compiles)"
	@echo "  make build           - Build the release binary the Docker image copies"
	@echo "  make docker-up       - Start all services with Docker"
	@echo "  make docker-down     - Stop all services"
	@echo "  make docker-rebuild  - Rebuild and restart services"
	@echo "  make docker-logs     - Follow logs from all services"
	@echo "  make docker-clean    - Stop services and remove volumes"
	@echo "  make test            - Run tests"
	@echo "  make fmt             - Format code"
	@echo "  make clippy          - Run clippy linter"
	@echo "  make audit           - Check dependency advisories"
	@echo "  make lint            - Validate migration directory names"

# memory-serve embeds crates/frontend/dist into the backend binary at compile
# time, so this has to run before anything builds the backend.
frontend:
	cd crates/frontend && trunk build --release

build: frontend
	cargo build --release --bin backend

# The image copies target/release/backend rather than compiling in Docker.
docker-up: build
	docker-compose up --build

docker-down:
	docker-compose down

docker-rebuild: build
	docker-compose up --build --force-recreate

docker-logs:
	docker-compose logs -f

docker-clean:
	docker-compose down -v

test:
	cargo test --workspace

fmt:
	cargo fmt

clippy:
	cargo clippy --workspace --all-targets

audit:
	cargo audit

lint:
	./scripts/check-migration-names.sh
