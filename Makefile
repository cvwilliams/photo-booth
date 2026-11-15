.PHONY: dev build preview hosting-preview test fmt lint shell clean deploy deploy-setup

dev: ; docker compose up dev

build: ; docker compose run --rm builder

preview: ; docker compose run --rm --service-ports preview

# Serve dist/ with Firebase's hosting emulator: production headers and rewrites, no sign-in.
hosting-preview: build
	docker compose run --rm --no-deps -p 127.0.0.1:5000:5000 firebase emulators:start --only hosting --project demo-photo-booth

test: ; docker compose run --rm --no-deps dev cargo test -p booth-core

fmt: ; docker compose run --rm --no-deps dev cargo fmt --all

lint: ; docker compose run --rm --no-deps dev cargo clippy --workspace --all-targets --target wasm32-unknown-unknown -- -D warnings

shell: ; docker compose run --rm --no-deps dev bash

# Manual deploy of a fresh release build (CI normally does this on merge to main).
deploy: build
	docker compose run --rm firebase deploy --only hosting

# One-time: sign in and let Firebase add its deploy secret to the GitHub repo.
deploy-setup:
	docker compose run --rm firebase login --no-localhost
	docker compose run --rm firebase init hosting:github

clean:
	docker compose down --volumes
	rm -rf dist
