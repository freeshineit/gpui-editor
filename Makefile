.PHONY: build demo dev fmt lint test doc

build:
	@echo "==> Building gpui-editor..."
	cargo build $(ARGS)
	@echo "==> Build succeeded."

demo:
	@echo "==> Running demo..."
	cargo run --example demo $(ARGS)

dev:
	@echo "==> Watching demo (auto-restart on changes)..."
	cargo watch -x 'run --example demo' $(ARGS)

fmt:
	@echo "==> Formatting code..."
	cargo fmt $(ARGS)
	@echo "==> Format complete."

lint:
	@echo "==> Checking code (lint)..."
	cargo clippy --all-targets --all-features -- -D warnings $(ARGS)
	@echo "==> Lint passed."

test:
	@echo "==> Running tests..."
	cargo test $(ARGS)
	@echo "==> All tests passed."

doc:
	@echo "==> Generating documentation..."
	cargo doc --no-deps --open $(ARGS)
	@echo "==> Docs generated."
