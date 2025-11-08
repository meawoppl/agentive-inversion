.PHONY: help docker-up docker-down docker-rebuild docker-logs docker-clean test fmt clippy

help:
	@echo "Available commands:"
	@echo "  make docker-up       - Start all services with Docker"
	@echo "  make docker-down     - Stop all services"
	@echo "  make docker-rebuild  - Rebuild and restart services"
	@echo "  make docker-logs     - Follow logs from all services"
	@echo "  make docker-clean    - Stop services and remove volumes"
	@echo "  make test            - Run tests"
	@echo "  make fmt             - Format code"
	@echo "  make clippy          - Run clippy linter"

docker-up:
	docker-compose up --build

docker-down:
	docker-compose down

docker-rebuild:
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
	cargo clippy --workspace --all-features
