#!/usr/bin/env bash
# arch_bootstrap.sh
# Bootstrap a fresh Arch install with my usual packages.

set -euo pipefail

# Core system update
sudo pacman -Syu

# Official repo packages (pacman)
PACMAN_PKGS=(
  # browsers / desktop basics
  firefox
  bitwarden

  # WM / DE
  hyprland
  waybar

  # office / viewers / media
  libreoffice-fresh
  vlc
  inkscape
  obs-studio
  retroarch

  # audio (PulseAudio stack)
  pulseaudio
  pulseaudio-alsa
  pavucontrol
  pamixer

  # terminals / editors / tools
  vifm
  neovim
  r
  lazygit
  stow
  git
  ripgrep
  uv
  zsh
  wget

  # fonts
  noto-fonts
  noto-fonts-emoji
  ttf-dejavu
  ttf-liberation
  ttf-nerd-fonts-symbols-mono

  # system / cli niceties
  zoxide
  fd
  eza
  btop
  fastfetch

  # gaming
  steam

  # pdf / docs
  zathura
  zathura-pdf-mupdf

  # github cli (gh)
  github-cli

  # build tools for AUR stuff etc.
  base-devel
)

echo "==> Installing official repo packages with pacman..."
sudo pacman -S --needed --noconfirm "${PACMAN_PKGS[@]}"

echo "==> Switching audio stack to PulseAudio (removing pipewire-pulse if present)..."
sudo pacman -Rns --noconfirm pipewire-pulse 2>/dev/null || echo "pipewire-pulse not installed or already removed."

echo "==> Ensuring yay (AUR helper) is installed..."
if ! command -v yay >/dev/null 2>&1; then
  sudo pacman -S --needed --noconfirm base-devel git

  tmpdir="$(mktemp -d)"
  git clone https://aur.archlinux.org/yay.git "$tmpdir/yay"
  (
    cd "$tmpdir/yay"
    makepkg -si --noconfirm
  )
  rm -rf "$tmpdir"
fi

AUR_HELPER="yay"

# AUR packages
AUR_PKGS=(
  # chat / cloud / misc GUI apps
  whatsapp-for-linux
  chatgpt-desktop-bin
  spotify
  slack-desktop
  localsend-bin
  pinta

  # extras you mentioned, likely in AUR
  impala
  caligular

  # AI coding tools (package names may need adjustment)
  claude-code
  openai-codex-bin
)

if ((${#AUR_PKGS[@]})); then
  echo "==> Installing AUR packages with $AUR_HELPER..."
  "$AUR_HELPER" -S --needed "${AUR_PKGS[@]}"
fi

echo "==> Installing R packages (tidyverse, marginaleffects, modelsummary)..."
if command -v Rscript >/dev/null 2>&1; then
  Rscript -e 'install.packages(
                c("tidyverse", "marginaleffects", "modelsummary"),
                repos = "https://cloud.r-project.org"
              )' \
    || echo "Warning: R package installation failed; you can rerun this step manually."
else
  echo "Rscript not found; skipping R package installation."
fi

echo "==> Setting zsh as the default shell (for the current user)..."
if command -v zsh >/dev/null 2>&1; then
  ZSH_PATH="$(command -v zsh)"
  if [ "${SHELL:-}" != "$ZSH_PATH" ]; then
    chsh -s "$ZSH_PATH" || echo "Could not change default shell automatically. Run: chsh -s \"$ZSH_PATH\""
  else
    echo "zsh is already the default shell."
  fi
else
  echo "zsh binary not found; cannot change default shell."
fi

echo "==> Done."
