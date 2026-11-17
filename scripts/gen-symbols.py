#!/usr/bin/env python3
"""Gera crates/obelisk-comp/src/ui/symbols_table.rs: nome -> codepoint da Material Symbols Rounded.
Uso: python3 scripts/gen-symbols.py [caminho-da-fonte]"""
import sys
from fontTools.ttLib import TTFont

FONT = sys.argv[1] if len(sys.argv) > 1 else "/usr/share/fonts/TTF/MaterialSymbolsRounded[FILL,GRAD,opsz,wght].ttf"
BASE = NAMES = """add apps arrow_forward bedtime bluetooth bluetooth_disabled bolt calculate cancel check chevron_left chevron_right close contrast dark_mode dashboard description dock_to_bottom edit fast_forward folder folder_open home info keyboard keyboard_command_key launchpad light_mode lock logout memory music_note nightlight notifications open_in_full pause play_arrow power_settings_new public remove restart_alt screen_record screenshot_monitor screenshot_region search select sep settings settings_ethernet signal_wifi_off skip_next skip_previous space_dashboard sports_esports stop_circle terminal trash travel_explore tune view_carousel volume_down volume_mute volume_off volume_up wayland web_asset widgets wifi wifi_1_bar wifi_2_bar wifi_off zoom_in""".split()
BASE = set(BASE.split()) if isinstance(BASE, str) else set(NAMES)

# também todo nome entre aspas no código que seja um glifo da fonte (ícones das páginas dos Ajustes etc.)
import re, pathlib
for src in pathlib.Path("crates/obelisk-comp/src").rglob("*.rs"):
    if src.name == "symbols_table.rs":
        continue
    NAMES += re.findall(r'"([a-z][a-z0-9_]{2,40})"', src.read_text())

f = TTFont(FONT)
cmap = f.getBestCmap()
rev = {}
for cp, g in cmap.items():
    if cp >= 0xE000:
        rev.setdefault(g, cp)
# os glifos da Material Symbols se chamam como o ícone (ex.: "wifi") e têm um codepoint PUA
by_name = {g: cp for g, cp in sorted(rev.items(), key=lambda kv: kv[1])}
out = ["// Gerado por scripts/gen-symbols.py — não editar à mão.",
       "pub const SYMBOLS: &[(&str, char)] = &["]
missing = []
for n in dict.fromkeys(NAMES):
    cp = by_name.get(n)
    if cp is None:
        if n in NAMES[:200] and n in BASE:
            missing.append(n)
        continue
    out.append(f'    ("{n}", \'\\u{{{cp:x}}}\'),')
out.append("];")
open("crates/obelisk-comp/src/ui/symbols_table.rs", "w").write("\n".join(out) + "\n")
print(f"{len(out) - 3} símbolos; faltando: {missing}")
