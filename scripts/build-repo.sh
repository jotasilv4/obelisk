#!/usr/bin/env bash
# Gera o repositório pacman do Obelisk em repo/x86_64 (obelisk-desktop e obelisk-themes).
# Uso: scripts/build-repo.sh        Publicar: suba repo/x86_64/* para um servidor estático
#      (GitHub Releases/Pages) e gere a ISO com OBELISK_REPO_URL=https://.../x86_64
set -euo pipefail
cd "$(dirname "$0")/.."
ROOT=$PWD
OUT="$ROOT/repo/x86_64"
mkdir -p "$OUT"
for p in obelisk-themes obelisk-desktop; do
  echo "==> empacotando $p"
  (cd "packaging/$p" && PKGDEST="$OUT" BUILDDIR="$ROOT/work-pkg" makepkg -f --nodeps --noconfirm --cleanbuild >/dev/null)
done
# só a versão mais nova de cada pacote
for p in obelisk-themes obelisk-desktop; do
  ls -t "$OUT"/$p-[0-9]*.pkg.tar.zst 2>/dev/null | tail -n +2 | xargs -r rm -f
done
rm -f "$OUT"/obelisk.db* "$OUT"/obelisk.files*
repo-add -q "$OUT/obelisk.db.tar.zst" "$OUT"/*.pkg.tar.zst
echo "Repositório pronto: $OUT"
ls -1 "$OUT"
