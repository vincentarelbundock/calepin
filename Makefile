.PHONY: help docs plugins site

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

install: ## Build release binary, install to ~/.cargo/bin, and set up shell completions
	cargo install --path calepin
	@mkdir -p ~/.config/calepin
	@if [ -n "$$ZSH_VERSION" ] || [ "$$SHELL" = "/bin/zsh" ]; then \
		calepin completions zsh > ~/.config/calepin/_calepin 2>/dev/null && \
		echo "Zsh completions written to ~/.config/calepin/_calepin"; \
		echo "Add to .zshrc: fpath=(~/.config/calepin \$$fpath); compinit"; \
	elif [ -n "$$BASH_VERSION" ] || [ "$$SHELL" = "/bin/bash" ]; then \
		calepin completions bash > ~/.config/calepin/calepin.bash 2>/dev/null && \
		echo "Bash completions written to ~/.config/calepin/calepin.bash"; \
		echo "Add to .bashrc: source ~/.config/calepin/calepin.bash"; \
	elif [ -n "$$FISH_VERSION" ] || [ "$$SHELL" = "/usr/bin/fish" ]; then \
		calepin completions fish > ~/.config/fish/completions/calepin.fish 2>/dev/null && \
		echo "Fish completions installed."; \
	fi

clean:  ## Remove build artifacts
	cargo clean --manifest-path calepin/Cargo.toml

flush:  ## Delete all _cache directories recursively
	find . -type d -name '_cache' -exec rm -rf {} +

# ==============================================================================
# Test targets
# ==============================================================================

test:  ## Run unit tests
	cargo test --manifest-path calepin/Cargo.toml

check:  ## Run cargo check (fast compile check)
	cargo check --manifest-path calepin/Cargo.toml

site: build ## Build and serve static site from website/
	@cd website && ../calepin/target/debug/calepin site preview

# ==============================================================================
# Render targets
# ==============================================================================

docs:  build ## Render all .qmd files in website/ to all formats
	@cd website && for f in *.qmd; do \
		base=$${f%.qmd}; \
		for fmt in html latex typst markdown; do \
			case $$fmt in \
				html)     ext=html ;; \
				latex)    ext=tex  ;; \
				typst)    ext=typ  ;; \
				markdown) ext=md   ;; \
			esac; \
			../calepin/target/debug/calepin "$$f" --format $$fmt -o "$${base}.$${ext}"; \
		done; \
	done

# ==============================================================================
# Plugins
# ==============================================================================

WASM_TARGET = wasm32-unknown-unknown

plugins:  ## Build WASM plugins and install to plugins/ and website/_calepin/plugins/
	@mkdir -p website/_calepin/plugins
	@for plugin in plugins/*/; do \
		name=$$(basename $$plugin); \
		if [ -f "$$plugin/Cargo.toml" ]; then \
			echo "Building plugin: $$name"; \
			cargo build --release --target $(WASM_TARGET) --manifest-path "$$plugin/Cargo.toml" && \
			cp "$$plugin/target/$(WASM_TARGET)/release/$${name}.wasm" "$$plugin/$${name}.wasm" && \
			cp "$$plugin/$${name}.wasm" website/_calepin/plugins/ && \
			echo "  → plugins/$${name}/$${name}.wasm, website/_calepin/plugins/$${name}.wasm"; \
		fi; \
	done

# ==============================================================================
# Benchmarks
# ==============================================================================

bench:  release ## Benchmark calepin vs Quarto on bench/*.qmd (requires hyperfine)
	@cd bench && for f in *.qmd; do \
		base=$${f%.qmd}; \
		echo "\n=== Benchmarking $$base ===\n"; \
		hyperfine --warmup 1 \
			-n "calepin $$base → HTML"  '../calepin/target/release/calepin '"$$f"' -o '"$$base"'.html -q' \
			-n "calepin $$base → LaTeX" '../calepin/target/release/calepin '"$$f"' -o '"$$base"'.tex -q' \
			-n "Quarto $$base → HTML"   'quarto render '"$$f"' --to html --quiet' \
			-n "Quarto $$base → LaTeX"  'quarto render '"$$f"' --to latex --quiet' \
			--ignore-failure; \
		rm -f "$$base".html "$$base".tex "$$base".pdf; \
		rm -rf "$${base}_files"; \
	done
