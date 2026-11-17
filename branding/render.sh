#!/usr/bin/env bash
# Gera PNGs a partir dos SVGs originais do Obelisk. Requer: python3, rsvg-convert (librsvg).
set -euo pipefail
cd "$(dirname "$0")"

python3 gen.py
mkdir -p png/icons png/sizes

# Marca / ícone / wordmark
for f in obelisk-mark obelisk-mark-color obelisk-app-icon; do
  rsvg-convert -w 1024 -h 1024 "svg/$f.svg" -o "png/$f.png"
done
for f in obelisk-wordmark-light obelisk-wordmark-dark; do
  rsvg-convert -w 2200 "svg/$f.svg" -o "png/$f.png"
done

# Tamanhos de ícone para temas (hicolor)
for s in 16 24 32 48 64 128 256 512; do
  rsvg-convert -w "$s" -h "$s" svg/obelisk-app-icon.svg -o "png/sizes/obelisk-$s.png"
done

# Ícones de dock / apps
for f in svg/icons/*.svg; do
  n=$(basename "$f" .svg)
  rsvg-convert -w 512 -h 512 "$f" -o "png/icons/$n.png"
done

# Wallpapers 4K (e versão 1080p para live/preview)
for f in wallpapers/*.svg; do
  n=$(basename "$f" .svg)
  rsvg-convert -w 3840 -h 2160 "$f" -o "wallpapers/$n.png"
  rsvg-convert -w 1920 -h 1080 "$f" -o "wallpapers/$n-1080p.png"
done

echo "PNGs gerados em branding/png e branding/wallpapers"
