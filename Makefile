.PHONY: help build release install clean test check

help:  ## Display this help screen
	@echo -e "\033[1mAvailable commands:\033[0m\n"
	@grep -E '^[a-z.A-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | awk 'BEGIN {FS = ":.*?## "}; {printf "  \033[36m%-22s\033[0m %s\n", $$1, $$2}' | sort

# ==============================================================================
# Build targets
# ==============================================================================

build:  ## Build debug binary
	cargo build --manifest-path calepin/Cargo.toml

release:  ## Build optimized release binary
	cargo build --manifest-path calepin/Cargo.toml --release

install: ## Build release binary and install to ~/.cargo/bin
	cargo install --path calepin

clean:  ## Remove build artifacts
	cargo clean --manifest-path calepin/Cargo.toml

# ==============================================================================
# Test targets
# ==============================================================================

test:  ## Run unit tests
	cargo test --manifest-path calepin/Cargo.toml

check:  ## Run cargo check (fast compile check)
	cargo check --manifest-path calepin/Cargo.toml
