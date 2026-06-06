.PHONY: help build build-release release install clean test check version bump editors vscode vsx positron vscode-package vscode-sync-version vscode-stage-binary vscode-stage-pdfjs vscode-package-target vscode-package-universal vscode-publish vscode-compile cli-reference website serve

# Package version, parsed from the CLI crate manifest.
VERSION := $(shell awk -F'"' '/^version/ { print $$2; exit }' calepin/Cargo.toml)
VSCODE_DIR := editors/vscode
VSCODE_OUT := $(VSCODE_DIR)/dist
VSCODE_BIN_PATH ?= $(if $(BIN_PATH),$(BIN_PATH),target/release/calepin)
VSCODE_CLI := code
CALEPIN := uv run calepin
UNAME_S := $(shell uname -s)
ifeq ($(UNAME_S),Darwin)
  POSITRON_CLI := /Applications/Positron.app/Contents/Resources/app/bin/code
else
  POSITRON_CLI := positron
endif

help:  ## Display this help screen
	@echo -e "\033[1mAvailable commands:\033[0m\n"
	@grep -E '^[a-z.A-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | awk 'BEGIN {FS = ":.*?## "}; {printf "  \033[36m%-22s\033[0m %s\n", $$1, $$2}' | sort

# ==============================================================================
# Build targets
# ==============================================================================

build:  ## Build debug binary
	cargo build --manifest-path calepin/Cargo.toml

build-release:  ## Build optimized release binary
	cargo build --manifest-path calepin/Cargo.toml --release

install: ## Build release binary and install to ~/.cargo/bin
	cargo install --path calepin

version: ## Print the current package version
	@echo $(VERSION)

# Bump the Rust package version and refresh Cargo.lock. Usage:
# `make bump VERSION=0.0.2`.
bump: ## Bump package version (usage: make bump VERSION=x.y.z)
	@if [ -z "$(VERSION)" ] || [ "$(VERSION)" = "$(shell awk -F'"' '/^version/ { print $$2; exit }' calepin/Cargo.toml)" ]; then \
	    echo "usage: make bump VERSION=x.y.z  (must differ from current $(shell awk -F'"' '/^version/ { print $$2; exit }' calepin/Cargo.toml))"; \
	    exit 1; \
	fi
	@sed -i.bak -E 's/^version = "[^"]*"/version = "$(VERSION)"/' calepin/Cargo.toml && rm calepin/Cargo.toml.bak
	@cargo update -w >/dev/null
	@echo "Bumped calepin to $(VERSION)."
	@git diff --stat calepin/Cargo.toml Cargo.lock
	@echo ""
	@echo "Next: update docs if needed, commit calepin/Cargo.toml + Cargo.lock, then 'make release'."

# Tag the current commit and push the tag. This triggers:
#   - .github/workflows/release.yml        cargo-dist binaries and installers
#   - .github/workflows/publish-crates.yml cargo publish to crates.io
# Refuses to run on a dirty tree so the tag reflects committed code.
release: ## Tag and push v$(VERSION); fires cargo-dist + crates.io workflows
	@test -z "$$(git status --porcelain)" || { echo "working tree is dirty; commit or stash first"; exit 1; }
	@echo "Tagging v$(VERSION) at $$(git rev-parse --short HEAD) and pushing..."
	git tag -a v$(VERSION) -m "Release v$(VERSION)"
	git push origin v$(VERSION)

clean:  ## Remove build artifacts
	cargo clean --manifest-path calepin/Cargo.toml

# ==============================================================================
# Test targets
# ==============================================================================

test:  ## Run unit tests
	cargo test --manifest-path calepin/Cargo.toml

check:  ## Run cargo check (fast compile check)
	cargo check --manifest-path calepin/Cargo.toml

# ==============================================================================
# Editor extension targets
# ==============================================================================

editors: vscode vsx positron  ## Build editor packages and install in VS Code and Positron

vscode-sync-version:
	@node -e "const fs=require('fs'); \
	  const p='$(VSCODE_DIR)/package.json'; const j=JSON.parse(fs.readFileSync(p)); \
	  j.version='$(VERSION)'; fs.writeFileSync(p, JSON.stringify(j, null, 2)+'\n');"

vscode-stage-binary:
	@rm -rf $(VSCODE_DIR)/bin
	@mkdir -p $(VSCODE_DIR)/bin
	@cp "$(VSCODE_BIN_PATH)" $(VSCODE_DIR)/bin/
	@chmod +x $(VSCODE_DIR)/bin/$$(basename "$(VSCODE_BIN_PATH)") 2>/dev/null || true

vscode-stage-pdfjs:
	@rm -rf $(VSCODE_DIR)/media/pdfjs
	@mkdir -p $(VSCODE_DIR)/media/pdfjs/build $(VSCODE_DIR)/media/pdfjs/web
	@cp $(VSCODE_DIR)/node_modules/pdfjs-dist/build/pdf.min.mjs $(VSCODE_DIR)/media/pdfjs/build/
	@cp $(VSCODE_DIR)/node_modules/pdfjs-dist/build/pdf.worker.min.mjs $(VSCODE_DIR)/media/pdfjs/build/
	@cp $(VSCODE_DIR)/node_modules/pdfjs-dist/web/pdf_viewer.mjs $(VSCODE_DIR)/media/pdfjs/web/
	@cp $(VSCODE_DIR)/node_modules/pdfjs-dist/web/pdf_viewer.css $(VSCODE_DIR)/media/pdfjs/web/
	@cp -R $(VSCODE_DIR)/node_modules/pdfjs-dist/web/images $(VSCODE_DIR)/media/pdfjs/web/

vscode-compile: vscode-sync-version
	cd $(VSCODE_DIR) && npm install --no-audit --no-fund --silent
	cd $(VSCODE_DIR) && npx tsc -p ./

vscode-package: build-release vscode-stage-binary vscode-compile vscode-stage-pdfjs
	@mkdir -p $(VSCODE_OUT)
	cd $(VSCODE_DIR) && npx vsce package --no-dependencies -o dist/calepin-$(VERSION).vsix

vscode-package-target: vscode-sync-version vscode-stage-binary vscode-compile vscode-stage-pdfjs
	@test -n "$(TARGET)" || { echo "TARGET must be set (e.g. darwin-arm64)"; exit 1; }
	@mkdir -p $(VSCODE_OUT)
	@cd $(VSCODE_DIR) && npx tsc -p ./
	@cd $(VSCODE_DIR) && npx vsce package --no-dependencies \
	  --target $(TARGET) -o dist/calepin-$(TARGET)-$(VERSION).vsix

vscode-package-universal: vscode-sync-version vscode-compile vscode-stage-pdfjs
	@mkdir -p $(VSCODE_OUT)
	@cd $(VSCODE_DIR) && npx tsc -p ./
	@cd $(VSCODE_DIR) && npx vsce package --no-dependencies \
	  -o dist/calepin-universal-$(VERSION).vsix

vscode-publish:
	@for v in $(VSCODE_OUT)/*.vsix; do \
	  echo "Publishing $$v"; \
	  npx --prefix $(VSCODE_DIR) vsce publish --no-dependencies --packagePath $$v -p $$VSCE_PAT; \
	  npx --prefix $(VSCODE_DIR) ovsx publish $$v -p $$OVSX_PAT; \
	done

vscode: vscode-package  ## Install Calepin for Typst in VS Code
	$(VSCODE_CLI) --install-extension $(VSCODE_OUT)/calepin-$(VERSION).vsix --force

vsx: vscode-package  ## Build Calepin for Typst Open VSX VSIX
	@mkdir -p $(VSCODE_OUT)
	@cp $(VSCODE_OUT)/calepin-$(VERSION).vsix $(VSCODE_OUT)/calepin-open-vsx-$(VERSION).vsix

positron: vscode-package  ## Install Calepin for Typst in Positron
	$(POSITRON_CLI) --install-extension $(VSCODE_OUT)/calepin-$(VERSION).vsix --force

# ==============================================================================
# Documentation targets
# ==============================================================================

cli-reference:  ## Generate docs-src/cli.md from clap help output
	@set -eu; { \
		printf '%s\n' '---' 'title: CLI reference' '---' ''; \
		printf '# CLI reference\n\n'; \
		printf '## `calepin`\n\n```text\n'; \
		$(CALEPIN) --help; \
		printf '```\n\n'; \
		printf '## `calepin compile`\n\n```text\n'; \
		$(CALEPIN) compile --help; \
		printf '```\n\n'; \
		printf '## `calepin watch`\n\n```text\n'; \
		$(CALEPIN) watch --help; \
		printf '```\n\n'; \
		printf '## `calepin stop`\n\n```text\n'; \
		$(CALEPIN) stop --help; \
		printf '```\n'; \
	} > docs-src/cli.md

website:  ## Render docs/ via build_website.py
	@set -eu; \
	uv run python build_website.py

serve:  ## Build and serve the website at http://localhost:8000
	$(MAKE) website
	uv run python -m http.server 8000 --directory docs
