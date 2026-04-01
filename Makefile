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
		calepin extra completions zsh > ~/.config/calepin/_calepin 2>/dev/null && \
		echo "Zsh completions written to ~/.config/calepin/_calepin"; \
		echo "Add to .zshrc: fpath=(~/.config/calepin \$$fpath); compinit"; \
	elif [ -n "$$BASH_VERSION" ] || [ "$$SHELL" = "/bin/bash" ]; then \
		calepin extra completions bash > ~/.config/calepin/calepin.bash 2>/dev/null && \
		echo "Bash completions written to ~/.config/calepin/calepin.bash"; \
		echo "Add to .bashrc: source ~/.config/calepin/calepin.bash"; \
	elif [ -n "$$FISH_VERSION" ] || [ "$$SHELL" = "/usr/bin/fish" ]; then \
		calepin extra completions fish > ~/.config/fish/completions/calepin.fish 2>/dev/null && \
		echo "Fish completions installed."; \
	fi

clean:  ## Remove build artifacts
	cargo clean --manifest-path calepin/Cargo.toml

flush:  ## Delete build artifacts (.calepin/ and *_output/ directories)
	rm -rf .calepin *_output website/.calepin website/*_output

# ==============================================================================
# Test targets
# ==============================================================================

test:  ## Run unit tests
	cargo test --manifest-path calepin/Cargo.toml

check:  ## Run cargo check (fast compile check)
	cargo check --manifest-path calepin/Cargo.toml

CLP = target/debug/calepin

site: build ## Build and serve static site from website/
	rm -rf website/.calepin website/*_output website/index_calepin/templates/
	$(CLP) preview website

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
			../target/debug/calepin "$$f" -t $$fmt -o "$${base}.$${ext}"; \
		done; \
	done

# ==============================================================================
# Profiling
# ==============================================================================

PROF_FILE ?= bench/text.qmd

prof-build:  ## Build with profiling profile (release + debug symbols)
	cargo build --manifest-path calepin/Cargo.toml --profile profiling

prof: prof-build  ## Profile single file (set PROF_FILE=bench/text.qmd)
	cd $$(dirname $(PROF_FILE)) && samply record --save-only --unstable-presymbolicate -o profile.json -- ../target/profiling/calepin $$(basename $(PROF_FILE)) -o /dev/null -q
	@echo "Profile saved to $$(dirname $(PROF_FILE))/profile.json"
	bench/profile_summary.py $$(dirname $(PROF_FILE))/profile.json

prof-batch: prof-build  ## Profile 1000 parallel files (gibberish complexity 2)
	bench/gibberish.sh --samply

prof-website: prof-build  ## Profile rendering the website/ collection
	cd website && samply record --save-only --unstable-presymbolicate -o profile.json -- ../target/profiling/calepin *.qmd -q
	@echo "Profile saved to website/profile.json"
	bench/profile_summary.py website/profile.json

# ==============================================================================
# Benchmarks
# ==============================================================================

bench: release  ## Time single file render (bench/text.qmd)
	@cd bench && hyperfine --warmup 3 \
		-n "calepin text → HTML"  '../target/release/calepin text.qmd -o /dev/null -q' \
		-n "calepin text → LaTeX" '../target/release/calepin text.qmd -t latex -o /dev/null -q' \
		--ignore-failure

bench-batch: release  ## Time 1000 parallel files (gibberish complexity 2)
	bench/gibberish.sh
