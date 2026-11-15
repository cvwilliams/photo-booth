.PHONY: dev build preview test fmt lint shell clean

dev: ; docker compose up dev

build: ; docker compose run --rm builder

preview: ; docker compose run --rm --service-ports preview

test: ; docker compose run --rm --no-deps dev cargo test -p booth-core

fmt: ; docker compose run --rm --no-deps dev cargo fmt --all

lint: ; docker compose run --rm --no-deps dev cargo clippy --workspace --all-targets --target wasm32-unknown-unknown -- -D warnings

shell: ; docker compose run --rm --no-deps dev bash

clean:
	docker compose down --volumes
	rm -rf dist
