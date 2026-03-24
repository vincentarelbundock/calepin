#!/usr/bin/env bash
# ubuntu_bootstrap.sh
# Bootstrap a fresh Ubuntu install with my usual packages.
# Tested with Ubuntu 24.04 (Noble) but should work on other supported R2u releases (focal/jammy/noble).

set -euo pipefail

prompt_block() {
  local message="$1"
  read -r -p "$message [y/N]: " reply
  case "$reply" in
    [yY]|[yY][eE][sS])
      return 0
      ;;
    *)
      return 1
      ;;
  esac
}

if ! command -v lsb_release >/dev/null 2>&1; then
  echo "lsb_release not found; this script is intended for Ubuntu."
  exit 1
fi

DISTRO_ID="$(lsb_release -is)"
CODENAME="$(lsb_release -cs)"

if [[ "$DISTRO_ID" != "Ubuntu" ]]; then
  echo "This script is intended for Ubuntu; detected: $DISTRO_ID"
  exit 1
fi

echo "==> Detected Ubuntu $CODENAME"

if prompt_block "Update system"; then
  echo "==> Updating system..."
  sudo apt update
  sudo apt upgrade -y
fi

if prompt_block "Base tools"; then
  echo "==> Installing base tools for extra repositories..."
  sudo apt install -y --no-install-recommends \
    ca-certificates \
    wget \
    gnupg \
    software-properties-common \
    dirmngr
fi

###############################################################################
# Enable multiverse + i386 (needed for Steam and some legacy libs)
###############################################################################
if prompt_block "Multiverse/i386"; then
  echo "==> Enabling multiverse..."
  # 1. Enable multiverse (where Steam lives)
  sudo add-apt-repository multiverse
  echo "==> Enabling 32-bit (i386) architecture..."
  # 2. Enable 32-bit (i386) architecture – Steam needs this
  sudo dpkg --add-architecture i386
fi

###############################################################################
# APT packages
###############################################################################
APT_PKGS=(
  # browsers / desktop basics
  firefox

  # office / viewers / media
  libreoffice
  vlc
  inkscape
  obs-studio

  # terminals / editors / tools
  byobu
  vifm
  stow
  git
  ripgrep
  zsh

  # R from CRAN + R packages from r2u
  r-base-core
  r-base-dev
  r-cran-tidyverse
  r-cran-marginaleffects
  r-cran-modelsummary

  # system / CLI niceties
  zoxide
  fd-find           # binary is `fdfind`
  eza
  btop

  # meta tools
  myrepos

  # pdf / docs
  zathura

  # GitHub CLI
  gh

  # fonts
  fonts-noto-core
  fonts-noto-color-emoji
  fonts-dejavu-core
  fonts-liberation

  # languages
  php-cli
  luarocks
  composer

  # build tools
  build-essential
)

if prompt_block "APT packages"; then
  echo "==> Installing APT packages..."
  sudo apt install -y --no-install-recommends "${APT_PKGS[@]}" || {
    echo "Some APT packages failed to install. You can rerun or adjust the list above."
  }
fi

###############################################################################
# R packages (post-APT)
###############################################################################
if prompt_block "cmdstanr/CmdStan"; then
  echo "==> Installing cmdstanr (R package)..."
  if command -v Rscript >/dev/null 2>&1; then
    Rscript -e "install.packages('cmdstanr', repos = c('https://stan-dev.r-universe.dev', getOption('repos')))"

    echo "==> Installing CmdStan (this may take a while)..."
    Rscript -e "cmdstanr::install_cmdstan()"
  else
    echo "Rscript not found; install R first (see APT packages above)."
  fi
fi

if prompt_block "Alacritty"; then
  echo "==> Installing Alacritty..."
  sudo apt install -y alacritty || echo "Alacritty install failed; you can retry later."
  if command -v alacritty >/dev/null 2>&1; then
    echo "==> Setting Alacritty as the default terminal..."
    sudo update-alternatives --install /usr/bin/x-terminal-emulator x-terminal-emulator "$(command -v alacritty)" 50
    sudo update-alternatives --set x-terminal-emulator "$(command -v alacritty)" || \
      echo "Could not set default terminal automatically; run update-alternatives --config x-terminal-emulator."
  fi
fi

if prompt_block "Steam"; then
  echo "==> Installing Steam..."
  sudo apt install -y steam || echo "Steam install failed; you can retry later."
fi

###############################################################################
# Snap packages (GUI apps)
###############################################################################
if prompt_block "Snap apps"; then
  echo "==> Ensuring snapd is installed..."
  sudo apt install -y snapd

  echo "==> Installing desktop apps via snap..."
  SNAP_APPS=(
    spotify                  # Spotify
    slack                    # Slack
    localsend                # LocalSend file transfer
    dropbox                  # Dropbox client
    inkscape                 # Vector graphics editor (snap)
    nvim                     # Neovim (snap, classic confinement)
    pinta                    # image editor (exists as APT too; snap is fine)
  )

  for app in "${SNAP_APPS[@]}"; do
    if snap list | awk 'NR>1 {print $1}' | grep -qx "$app"; then
      echo "snap '$app' already installed."
    else
      echo "Installing snap '$app'..."
      if [[ "$app" == "nvim" ]]; then
        sudo snap install "$app" --classic || echo "Could not install snap '$app' with classic confinement; retrying without classic."
        sudo snap install "$app" || echo "Could not install snap '$app'; skipping."
      else
        # Try with --classic first, fall back without if that fails
        if ! sudo snap install "$app" --classic 2>/dev/null; then
          sudo snap install "$app" || echo "Could not install snap '$app'; skipping."
        fi
      fi
    fi
  done
fi

###############################################################################
# uv (Astral)
###############################################################################
if prompt_block "uv"; then
  echo "==> Installing uv..."
  curl -LsSf https://astral.sh/uv/install.sh | sh
  uv python install --default
  uv tool install ruff
  uv tool install pytest
  cd $HOME
  uv venv
fi

###############################################################################
# Rustup
###############################################################################
if prompt_block "uv"; then
  echo "==> Installing uv..."
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
fi

###############################################################################
# Starship prompt
###############################################################################
if prompt_block "Starship"; then
  echo "==> Installing Starship..."
  curl -sS https://starship.rs/install.sh | sh
fi

###############################################################################
# Lazygit (latest release)
###############################################################################
if prompt_block "Lazygit"; then
  echo "==> Installing lazygit..."
  LAZYGIT_VERSION=$(curl -s "https://api.github.com/repos/jesseduffield/lazygit/releases/latest" | \grep -Po '"tag_name": *"v\K[^"]*')
  curl -Lo lazygit.tar.gz "https://github.com/jesseduffield/lazygit/releases/download/v${LAZYGIT_VERSION}/lazygit_${LAZYGIT_VERSION}_Linux_x86_64.tar.gz"
  tar xf lazygit.tar.gz lazygit
  sudo install lazygit -D -t /usr/local/bin/
fi

###############################################################################
# Default shell: zsh
###############################################################################
if prompt_block "Default zsh"; then
  echo "==> Setting zsh as the default shell..."
  if command -v zsh >/dev/null 2>&1; then
    ZSH_PATH="$(command -v zsh)"
    if [ "${SHELL:-}" != "$ZSH_PATH" ]; then
      if chsh -s "$ZSH_PATH"; then
        echo "Default shell changed to zsh ($ZSH_PATH). Log out and back in to apply."
      else
        echo "Could not change default shell automatically. Run: chsh -s \"$ZSH_PATH\""
      fi
    else
      echo "zsh is already the default shell."
    fi
  else
    echo "zsh binary not found; cannot change default shell."
  fi
fi

###############################################################################
# SSH key
###############################################################################
if prompt_block "SSH key"; then
  echo "==> Creating SSH key..."
  SSH_DIR="$HOME/.ssh"
  KEY_PATH="$SSH_DIR/id_ed25519"
  mkdir -p "$SSH_DIR"
  chmod 700 "$SSH_DIR"
  if [ -f "$KEY_PATH" ]; then
    echo "SSH key $KEY_PATH already exists; skipping."
  else
    DEFAULT_COMMENT="$(whoami)@$(hostname)"
    read -r -p "Email/comment for SSH key [$DEFAULT_COMMENT]: " SSH_EMAIL
    SSH_EMAIL="${SSH_EMAIL:-$DEFAULT_COMMENT}"
    ssh-keygen -t ed25519 -C "$SSH_EMAIL" -f "$KEY_PATH"
    echo "Public key:"
    cat "${KEY_PATH}.pub"
  fi
fi

echo "==> Done."


###############################################################################
# GPT Codex CLI
###############################################################################
if prompt_block "Codex CLI"; then
  echo "==> Installing GPT Codex CLI..."

  if command -v codex >/dev/null 2>&1; then
    echo "Codex CLI already installed; skipping."
  else
    # Ensure Node.js + npm
    if ! command -v node >/dev/null 2>&1 || ! command -v npm >/dev/null 2>&1; then
      echo "Node.js or npm not found."

      if command -v apt >/dev/null 2>&1; then
        echo "==> Installing Node.js and npm with apt..."
        sudo apt install -y nodejs npm
      else
        echo "No supported package manager detected for Node.js."
        echo "Please install Node.js (>=18) and npm manually, then re-run this script."
      fi
    fi

    if command -v npm >/dev/null 2>&1; then
      # Decide whether we need sudo for global install
      NPM_PREFIX="$(npm config get prefix 2>/dev/null || echo "")"
      NPM_CMD="npm"

      case "$NPM_PREFIX" in
        /usr|/usr/local|"")
          # System-wide prefix -> likely need sudo
          NPM_CMD="sudo npm"
          ;;
      esac

      echo "==> Installing Codex CLI with $NPM_CMD..."
      $NPM_CMD install -g @openai/codex

      if command -v codex >/dev/null 2>&1; then
        echo "Codex CLI installed successfully:"
        codex --version
      else
        echo "Codex CLI installation finished, but 'codex' is not on your PATH."
        echo "You may need to adjust your PATH or restart your shell."
      fi
    else
      echo "npm still not available; cannot install Codex CLI."
    fi
  fi
fi
