#!/usr/bin/env bash
# Gera a ISO do Obelisk.
#   1. gera o repositório pacman do Obelisk (scripts/build-repo.sh): obelisk-desktop + obelisk-themes
#   2. monta uma cópia do perfil com o repositório apontado para repo/x86_64 desta árvore e
#      uma cópia dele dentro da ISO (/opt/obelisk-repo, usada pelo instalador)
#   3. roda o mkarchiso (precisa de root e do pacote `archiso`)
#
# Uso: scripts/build-iso.sh [--no-iso] [--skip-repo]
#   OBELISK_REPO_URL=https://.../x86_64  → endereço público do repositório (atualizações do
#                                           sistema instalado). Sem ele, o sistema instalado usa
#                                           a cópia local e só recebe atualizações do Arch.
set -euo pipefail
cd "$(dirname "$0")/.."
ROOT=$PWD

[[ " $* " == *" --skip-repo "* ]] || scripts/build-repo.sh

echo "==> preparando o perfil"
P="$ROOT/work-profile"
rm -rf "$P"
cp -a profile "$P"
sed -i "s|@OBELISK_REPO@|$ROOT/repo/x86_64|" "$P/pacman.conf"
mkdir -p "$P/airootfs/opt/obelisk-repo"
cp -a repo/x86_64/. "$P/airootfs/opt/obelisk-repo/"
if [[ -n "${OBELISK_REPO_URL:-}" ]]; then
  echo "$OBELISK_REPO_URL" > "$P/airootfs/usr/share/obelisk/repo-url"
fi

[[ " $* " == *" --no-iso "* ]] && { echo "perfil pronto em $P"; exit 0; }

command -v mkarchiso >/dev/null || { echo "instale o archiso:  sudo pacman -S archiso"; exit 1; }
echo "==> mkarchiso (pede sudo)"
sudo rm -rf work
sudo mkarchiso -v -w work -o out "$P"
echo "ISO em: $(ls -1t out/*.iso | head -1)"
