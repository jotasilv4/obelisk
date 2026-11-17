# kyro shell — visual + behavior spec (for a tiny-skia re-implementation)

Source: `config/quickshell` do kyro (repositório moonlast) (read-only analysis). All sizes are **logical px** (Qt `pixelSize`/item units; multiply by output scale). Theme tokens already known are only resolved here, not re-explained.

Conventions used below:
- `fg(a)` = `Theme.foreground(item, a)` = **white@a in dark mode, black@a in light mode** — *except* inside any ancestor with `darkSurface: true` (Notch, Launchpad content, Mission Control spaces row, LockScreen login column), where it is **always white@a**.
- `overlay(a)` = same as `fg(a)`.
- Durations in ms. Qt `NumberAnimation` default easing = **Linear**. `OutCubic` = `1-(1-t)^3`; `OutQuad` = `1-(1-t)^2`; `OutBack` = Qt overshoot 1.70158 (`c1=1.70158,c3=c1+1: 1+c3(t-1)^3+c1(t-1)^2`); `OutBounce` = Penner bounce.
- "Shadow r=X" means Qt `MultiEffect` with `shadowBlur*blurMax = X` px blur radius (MultiEffect approximates a Gaussian whose full extent ≈ that radius → use σ ≈ X/2 in a Gaussian/3×box blur). Shadow colour black, no x-offset unless noted.
- tiny-skia has **no blur filter and no text**: you need your own blur (3-pass box blur on an A8 mask) and a shaper/rasterizer (e.g. rustybuzz + swash/cosmic-text) that supports **OpenType ligatures + variable axes** (needed for Material Symbols and Inter weights).

---

## 0. Resolved colour tokens

| Token | Dark | Light |
|---|---|---|
| label (fg .85) | `rgba(255,255,255,.85)` | `rgba(0,0,0,.85)` |
| secondaryLabel | white .55 | black .55 |
| tertiaryLabel | white **.25** | black **.40** |
| quaternaryLabel / separator | white .10 | black .10 |
| windowBg | `#1e1e20` | `#f5f5f7` |
| raisedBg | `#2a2a2d` | `#ffffff` |
| sidebarBg | `rgba(33,33,38,.85)` | `rgba(240,240,245,.90)` |
| dockBg | `rgba(28,28,33,.42)` | `rgba(255,255,255,.65)` |
| menuBg | `rgba(33,33,36,.72)` | `rgba(250,250,252,.90)` |
| menuBorder | white .14 | black .14 |
| ccModule / surface | white .08 | black .08 |
| surfaceHover | white .14 | black .14 |
| notch | `#000000` | `#000000` |
| accent (default "blue") | `#0091ff` | same |
| textOnAccent | white for blue/purple/pink/red/graphite; `rgba(0,0,0,.85)` for yellow/orange/green (rule: `1.05/(L+0.05) >= 3` with WCAG relative luminance) | same |

Accent palette (Config.accents): blue `#0091ff`, purple `#db34f2`, pink `#ff375f`, red `#ff4245`, orange `#ff9230`, yellow `#ffd600`, green `#30d158`, graphite `#8e8e93`. Extra system colours: indigo `#6d7cff`, teal `#00d2e0`, gray `#8e8e93`.

Dark/light: `Config.s.appearance` default **"dark"**; "auto" = light 07:00–18:59.

Bar text colours (depend on wallpaper, not on dark mode):
`barOnLight = barStyle != "black" && Wallpaper.luminance > 0.62`

| | barOnLight | else |
|---|---|---|
| barText | `rgba(0,0,0,.85)` | `#ffffff` |
| barTextDim | `rgba(0,0,0,.35)` | `rgba(255,255,255,.45)` |
| barHighlight | `rgba(0,0,0,.12)` | `rgba(255,255,255,.22)` |

barBackground: `"transparent"` (default) · `"blur"` → dark `rgba(20,20,23,.12)` / light `rgba(235,235,240,.12)` · `"black"` → `#000`.

---

## 1. Compositor-side material (Hyprland config the look depends on)

From `config/hypr/hyprland.lua` — the "vibrancy" is *compositor blur behind translucent layer pixels*, not drawn by QML.

| Setting | Value |
|---|---|
| blur | size 16, passes 4 (dual-Kawase), noise 0.015, contrast 1.0, brightness 1.0, vibrancy 0, popups on |
| window rounding | 12, `rounding_power 3` (superellipse-ish corners, not circular) |
| window shadow | range 18, render_power 3, offset (0,6), colour `0x55000000` (black @ 0.333) |
| gaps | in 4, out 10, border 0 |
| layer blur rules (`ignore_alpha` = only pixels with alpha > threshold get blur) | `kyro-bar` 0.05 (+blur_popups) · `kyro-panel` 0.5 · `kyro-dock` 0.3 · `kyro-overlay` 0.05; all `no_anim` |
| curves | `macos` bezier (0.25,0.1,0.25,1) [= CSS ease]; `snappy` (0.05,0.9,0.1,1.05); `easy` spring mass 1, stiffness 260, damping 25 |
| animations (speed unit = 100 ms) | global 600 ms macos; windows spring easy (speed 4); windowsIn popin 90% (speed 3.5, easy); windowsOut 200 ms macos popin 90%; fade 300 ms; layers 300 ms fade (but kyro layers no_anim); workspaces 350 ms snappy slide |
| cursor | `MacTahoe-cursors`, size 24 |

Namespace → surface: Bar `kyro-bar`; menus/CC/NC/BarPopup/Spotlight/AppSwitcher/ScreenshotBar `kyro-panel`; Dock `kyro-dock`; Launchpad/Mission Control `kyro-overlay`; Background `kyro-wallpaper` (Background layer). LockScreen = ext-session-lock (blur done in QML).

---

## 2. Primitives

### Spring.qml (SwiftUI `.spring(response, dampingFraction)`)
- mass 1, `k = (2π/response)²`, `c = 4π·ζ/response`.
- Semi-implicit Euler, fixed sub-steps `dt = 1/240 s`, frame time clamped to ≤ 1/30 s: `v += (-k(x-target) - c·v)·dt; x += v·dt`.
- Settles when `|x-target| < ε && |v| < 10ε` (ε default 0.05 px; 0.002 for opacity/scale) → snap to target, v=0.
- Velocity is **preserved when target changes mid-flight** (key to the "liquid" notch). First value = target (no initial animation).

| Use | response | ζ | k | c |
|---|---|---|---|---|
| Notch open (w,h) | 0.42 | 0.8 | 223.8 | 23.94 |
| Notch close (w,h) | 0.45 | 1.0 | 194.96 | 27.93 |
| Notch radii (ear, bottom) | resp as above | 1.0 | — | — |
| Notch content in | 0.40 | 1.0 | 246.7 | 31.42 |
| Notch content out | 0.22 | 1.0 | 815.6 | 57.12 |
| Dock slide (autohide) | 0.35 | 1.0 | 322.3 | 35.90 |

### Txt.qml (default text)
Inter, 13 px, **Medium (500)**, colour fg(.85), elide right, vcenter. Grayscale AA (fontconfig: antialias, rgba none, hintslight, lcdfilter none; `QSG_DISTANCEFIELD_ANTIALIASING=gray`). Tabular figures (`tnum`) used for clocks/percentages.
Qt weights: Normal 400, Medium 500, DemiBold 600, Bold 700, Light 300.

### Symbol.qml (icons)
- Font **"Material Symbols Rounded"** variable (`MaterialSymbolsRounded[FILL,GRAD,opsz,wght].ttf`, Arch pkg `ttf-material-symbols-variable`, Google, Apache-2.0).
- Text = ligature name (e.g. `"wifi"`); axes: `FILL` 0/1 (prop `fill`, default 0), `wght` default 500, `opsz = clamp(size,20,48)`, `GRAD 0`. Pixel size = `size` (default 16). Colour default fg(.85). Centered H+V. Rendered as curves (vector).
- Codepoints (extracted from the installed Rounded font's GSUB; use these to avoid ligature shaping):

| name | cp | name | cp | name | cp |
|---|---|---|---|---|---|
| add | E145 | apps | E5C3 | arrow_forward | E5C8 |
| bedtime | E1F9 | bluetooth | E1A7 | bluetooth_connected | E1A8 |
| bluetooth_disabled | E1A9 | bolt | EA0B | calculate | EA5F |
| cancel | E5C9 | check | E5CA | close | E14C |
| content_copy | E14D | content_paste | E14F | dark_mode | E51C |
| event_available | E614 | expand_more | E5CF | fast_forward | E01F |
| folder | E2C7 | folder_open | E2C8 | home | E88A |
| image | E251 | inbox | E156 | language | E894 |
| light_mode | E518 | lock | E88D | logout | E9BA |
| memory | E322 | memory_alt | F7A3 | nightlight | EF5E |
| notes | E26C | notifications | E7F4 | open_in_full | F1CE |
| pause | E034 | play_arrow | E037 | power_settings_new | E8AC |
| remove | E15B | restart_alt | F053 | screen_record | F679 |
| screenshot_monitor | EC08 | screenshot_region | F7D2 | search | E8B6 |
| select | F74D | settings_ethernet | E8BE | signal_wifi_off | E1DA |
| sports_esports | EA28 | stadia_controller | F135 | stop_circle | EF71 |
| travel_explore | E2DB | view_quilt | E8F1 | volume_down | E04D |
| volume_mute | E04E | volume_off | E04F | volume_up | E050 |
| warning | E002 | web_asset | E069 | wifi | E63E |
| wifi_1_bar | E4CA | wifi_2_bar | E4D9 | wifi_tethering | E1E2 |

(Net icon: hotspot→wifi_tethering; wifi signal >66 wifi, >33 wifi_2_bar, else wifi_1_bar; wired→settings_ethernet; offline→signal_wifi_off. Audio.symbol: muted/≤0.001 volume_off, <0.34 volume_mute, <0.67 volume_down, else volume_up.)
- `Theme.iconFont` "JetBrainsMono Nerd Font" exists but the shell UI uses Material Symbols (Nerd glyphs only in legacy `Audio.icon`).

### SymbolButton.qml
Size `size + 2·pad` (defaults 18 + 2·6 = 30). Hover: full circle `overlay(.14)`, opacity fade 120 ms. Press: glyph scale 0.85 (110 ms). Disabled opacity 0.3. `fill` default **true**. Pointing-hand cursor.

### KyroLogo.qml
Vector, in a 120×120 viewBox scaled to item size (default 18×18; bar uses 17×17). Two filled paths:
- Crescent: `M61.46 12.02 A48 48 0 1 0 107.82 64.17 A36 36 0 1 1 61.46 12.02 Z`
- "New moon" disc: if size ≤ 32: `M88.26 21.2 A48 48 0 0 1 101.84 36.48 A25 25 0 1 1 88.26 21.2 Z`; else `M79.25 16.03 A48 48 0 0 1 105.93 46.04 A28 28 0 1 1 79.25 16.03 Z`.
Colour default barText (both paths; `accent` overridable — Avatar uses `#eef1ff` / `#6c7dff`).

### Background.qml
Fullscreen Background layer, black base, wallpaper `PreserveAspectCrop` (cover, centred), decoded at screen size. Path: `Config.s.wallpaper` or `~/.local/share/kyro/wallpaper.jpg`.

### Wallpaper luminance (services/Wallpaper.qml)
`ColorQuantizer { depth: 0; rescaleSize: 32 }` → image downscaled to 32 px, 2^0 = **1 bucket = mean colour**. `luminance = 0.2126·r + 0.7152·g + 0.0722·b` on **gamma-encoded sRGB 0..1 (no linearisation)**. Bar switches to dark text when > **0.62**.

### TrafficLights.qml
Row spacing 8; three 12×12 circles (r 6). Colours on hover: close `#ff5f57`, minimise `#febc2e`, zoom `#28c840`. **Not hovered: all three `overlay(.2)` (grey)**; hovering anywhere over the group colours all three and shows glyphs `close` / `remove` / `open_in_full` at size 9, wght 700, colour `rgba(0,0,0,.55)`. No animation. Used in in-shell apps (e.g. Settings at x 6, y 14 of its title area).

---

## 3. Bar.qml (menu bar)

Layer-shell top, anchored top/left/right, window height 260 (room for notch), **exclusive zone 32**, input mask = bar rect ∪ notch. Background `barBackground` (transparent by default → only compositor blur if style "blur").

### Left cluster (x starts at 10)
| Item | Geometry | Text |
|---|---|---|
| System menu title | width 34, KyroLogo 17×17 centred | — |
| App-name title | width = text + 2·10 | 13 px **Bold**, barText; name = .desktop name, else title/appId tail, capitalised, max 32 chars ("…") ; "Finder" when no window |
| "Janela" (only with an active window), "Ir" | width = text + 20 | 13 px **Medium** |
| Workspace dots | container width = dots + 20; dots spacing 5; each 6×6 r 3; **current = 18×6** pill | current barText, others barTextDim; width anim 260 ms OutBack; click → workspace (hit area +5 px) |

Title highlight (when its menu is open): capsule width = item width, height 24 (=32-8), radius 12, `barHighlight`, opacity anim 80 ms linear. **No hover highlight.** Pressing a title toggles its menu; while any bar menu is open, **hovering another title switches** to it.

### Right cluster (right margin 4, spacing 0, each status item 26 wide, height 32)
Order left→right: recording pill (if recording) · system tray items · game mode (`sports_esports` 18 filled green) · DND (`dark_mode` 16 filled, when DND & !game) · Bluetooth (17, outline) · Sound (`Audio.symbol`, 17 filled; wheel = ±5 % volume) · Battery (laptops) · Network (`Net.icon` 17 filled) · Control Center glyph · Clock.
- Recording: `stop_circle` 17 filled (red while recording) + elapsed 12 px tnum, spacing 5, width = row + 14.
- Tray: 16×16 app icon; left click activate, right/menu-only → native menu at (x, 32).
- Battery item: width = row + 12; row spacing 6: `NN%` 12 px + battery glyph.
  - Battery glyph 27×13: body 24×13 r 4 stroke 1 barTextDim; fill at (2,2) h 9 w `max(2, 20·level)` r 2, colour charging green / <20 % red / low-power yellow / else barText; nub 1.5×4 r 1, 1 px right of body, barTextDim; charging: `bolt` 12 filled white with black outline, centred.
- Control Center glyph (15×15.5): two capsules 15×6.5 r 3.25, stroke 1.3 barText, gap 2.5; knob 4.5 Ø filled barText — top knob right (x = 15-4.5-1), bottom knob left (x = 1).
- Clock: width = text + 12, 13 px Medium tnum, barText. Format `"Seg 16 de nov  14:05"` (pt_BR `ddd d 'de' MMM`, first letter capitalised, **two spaces**, then `HH:mm` or `h:mm AP`). Click → Notification Center.
- Status highlight: capsule (width-2)×24 r 12 barHighlight, shown when its panel is open **or while pressed**, 80 ms.
- Panels (CC, Wi-Fi, Sound, BT, Battery, NC) all open at the same spot: **top = 35 (32+3), right edge 6 px from screen edge**. Only one popup open at a time (toggle).

Text shadow on both clusters (only when **not** barOnLight): black, opacity .45, blur r≈2.8 (0.35·8), y-offset 1.

Services: ToplevelManager (active window), Apps, Hyprland workspaces, SystemTray, GameMode, Notifs.dnd, Bluetooth, Audio, BatteryInfo/PowerProfile, Net, Recorder, Config, Wallpaper.

Menus content (labels pt_BR): System (Sobre, Ajustes ⌘,, Atualizações, CC, Modo Jogo ✓, Modo Console, Forçar Encerrar ⌥⌘Esc, Repouso/Reiniciar/Desligar, Bloquear ⌃⌘Q, Encerrar Sessão ⇧⌘E), App, Janela, Ir (Mesa 1-4 ✓, folders, Lixo). Separators as listed in code.

---

## 4. MacMenu.qml (+ MenuItem / MenuSection / MenuSeparator / MenuDevice / BarPopup)

### MacMenu (NSMenu dropdown)
| Property | Value |
|---|---|
| Position (bar) | panel top-left at (title.x, **35**) |
| Position (dock context) | opens **upward**, panel bottom = 80 px from screen bottom (plateHeight 64 + 6 + 10); x = icon left |
| Width | `max(minWidth, content + 10)`; minWidth 230 (dock 200) |
| Panel | radius 12, menuBg, 1 px border menuBorder, padding 5 |
| Shadow | black .45, r≈21.6 (0.9·24), y +8 |
| Row | height 24, radius 7 (hover bg = **accent**), inner L/R margin 10, spacing 6 |
| Check column | only if any item has `checked`; 14 wide, `check` 14 px wght 600, opacity 0/1 |
| Label | 13 px Normal, label colour (disabled → tertiaryLabel). **Note: label colour does NOT switch to white on hover** |
| Shortcut | 13 px Normal, left margin 24, tertiaryLabel; on hover → label |
| Separator | item height 11, line 1 px, width = menu width − 20, separator colour |
| Open anim | opacity 0→1 90 ms linear + scale 0.97→1 140 ms OutCubic, origin top-centre (bottom-centre for dock). **No close animation** (destroyed). |
| Dismiss | click outside (Hyprland focus grab; bar is in grab set so clicking another title switches); click item → close first, then run action |
| Keyboard | none (no arrow navigation) |

### BarPopup (base for Sound/Bluetooth/Wi-Fi/Battery menus)
Width 290, top 35, right 6. Radius 12, menuBg, border menuBorder, content margins 8, column spacing 2, height = content + 16. Opacity 100 ms (no scale). Same shadow as MacMenu (.45, r 21.6, y 8).
- **MenuItem**: height 26, radius 6, hover bg accent, text x 8, 13 px Normal.
- **MenuSection**: 11 px Bold tertiaryLabel, left 8, top margin 6.
- **MenuSeparator**: 1 px separator, L/R margin 8, T/B margin 4.
- **MenuDevice**: height 38, radius 7, hover `overlay(.08)`; circle 26 Ø at x 6 (active accent / else overlay .12, 120 ms colour), glyph 16 filled white; title 13 Normal at circle+10; subtitle 11 secondary; trailing text 12 secondary right 8.

---

## 5. Dock.qml + services/DockGeometry.js

### Geometry (default `dockSize` 52, magnify on)
| | Value |
|---|---|
| icon size `S` | 52 |
| maxScale | 1.6 (1.0 if magnify off) |
| pad (plate inner padding) | 6 |
| spacing | 4 |
| separator cell | 13 wide; line 1 px × 0.75·S (39), `overlay(.22)` |
| bottom margin | 6 |
| plate height | S + 2·pad = **64** |
| plate width | row width (with *magnified* icons) + 12, horizontally centred |
| plate | radius **20**, dockBg, 1 px border `overlay(.16)` |
| window height | ceil(S·1.6)+pad+6+44 = 140 |
| exclusive zone | 74 only if autohide=false and smartHide=false; default smartHide=true → 0 |

Icons are bottom-aligned (6 px above plate bottom); magnified icons grow **upward/out of the plate**, the plate does not grow vertically, only horizontally.

Items order: Launchpad · pinned apps · [sep + running-unpinned apps] · sep · minimised windows (thumbnail r 8 inset 6 % + app icon 42 % bottom-right) · [Downloads, Documents folder stacks (Material `folder_open`/`folder` 0.85·size, filled, blue)] · Trash (`user-trash`/`user-trash-full`).

### Magnification (exact)
Computed against **un-magnified** base positions so icons don't run from the cursor:
```
baseCenters[i]: x = pad; for each item: w = (sep ? 13 : S); c = x + w/2; x += w + spacing
baseWidth = Σ(w + spacing) - spacing + 2·pad
baseLeft  = (screenWidth - baseWidth)/2
d         = mouseX - (baseLeft + baseCenters[i])
sigma     = 1.25·S                       // 65 px
scale_i   = 1 + (maxScale-1)·exp(-d²/(2σ²))      // separators always 1
```
scale = 1 when not hovered. Cell size = S·scale (both axes). Smoothing: scale changes animate 90 ms OutCubic; x 120 ms OutCubic. Hover leave is delayed 120 ms before resetting.

### Other behaviour
| | Value |
|---|---|
| Running dot | 4×4 r 2, `overlay(.75)`, 1 px below icon bottom (inside pad) |
| Tooltip | above icon by 10 px; height 26, width text+22, radius 8, menuBg, 1 px `overlay(.14)`, text 13 px Medium; shown on hover unless menu open; no animation |
| Press | icon scale 0.9 (100 ms) |
| Launch bounce (app not running) | offset 0 → 0.35·S (18.2 px up) in 220 ms OutQuad → 0 in 260 ms OutBounce; **2 loops** |
| Drag reorder (pinned only) | threshold 7 px horizontal; dragged cell follows cursor (z 10); drop index = count of pinned base centres left of drop x |
| Right click | context menu (MacMenu, edge bottom): window list, Nova Janela/Abrir, Manter no Dock ✓, Encerrar |
| Autohide/smart hide | Spring(0.35, ζ1) slide by plateHeight+6+6 = 76 px down; shown = hovered ∨ menu open ∨ (!autohide ∧ !obstructed). Hidden hit strip: 2 px at bottom edge |
| Obstruction (smartHide) | any mapped, non-hidden window on the active (or special, or pinned) workspace whose rect intersects the **base** dock rect `x = mon.x + (mw - baseWidth)/2, y = mon.y + mh - 6 - 64, w = baseWidth, h = 64` (mw/mh logical = size/scale, axes swapped for transforms 1,3,5,7) |
| Trash state | `ls -A ~/.local/share/Trash/files` on hover |
| Services | Config.s.pinned, Apps, ToplevelManager, Windows.minimized, Hyprland monitor |

### LaunchpadIcon.qml (Launchpad tile in dock)
- Tile inset 7 % of cell, radius 22.5 % of tile width; vertical gradient `#fdfdff` → `#d5d6de`; border max(1, 1.2 % width) white .7.
- Gloss overlay inset by border: gradient white .55 at 0 → white 0 at 0.5.
- 3×3 grid centred, spacing 7.5 % of tile, squares 18 % of tile, radius 32 % of square, each vertical gradient `lighter(c,1.15)` → `c`; colours row-major: `#ff4245 #ff9230 #ffd600 #30d158 #00d2e0 #0091ff #6d7cff #db34f2 #ff375f`.

---

## 6. Spotlight.qml

Fullscreen transparent `kyro-panel` overlay, exclusive keyboard. Click outside panel closes.

| | Value |
|---|---|
| Panel x | centred; **y = 22 % of screen height** |
| Width | min(680, W-40); clipboard mode min(820, W-40) |
| Height | 56 + (results ? listH + 12 : mode-without-results ? 70 : 0); clipboard mode 56+380. Height anim 140 ms OutCubic |
| Panel | radius **24**, menuBg, 1 px menuBorder, clip |
| Shadow | black .5, r 40, y +12 |
| Open | opacity 120 ms linear, scale 0.96→1 180 ms OutCubic (no close anim) |
| Field | height 56; `search` glyph 24 px secondaryLabel at x 18; input 22 px **Light (300)** label, 12 px after glyph; placeholder same font tertiaryLabel "Busca do Spotlight" (or mode title) |
| Mode chips (right, margin 14, spacing 4) | 4 circles 32 Ø: apps/bolt/content_paste/folder_open, 17 px; on = accent bg + filled textOnAccent; hover overlay .10; idle overlay .05, glyph secondary |
| Divider | 1 px separator under field (when results or mode) |
| List | top = field+6, left 6, width W-12, max height 400, scrolls |
| Section header | height 26, x 12, 11 px Bold tertiaryLabel (on first row of each section) |
| Row | height 40, radius 9, selected bg **accent**; margins L10 R12, spacing 10; icon 28 (app icon, or 28×28 r 7 tile coloured by item colour or overlay .12 with white filled glyph 18); title 14 px Medium (label / textOnAccent); subtitle 12 Normal tertiary (selected textOnAccent .9), max 260 wide |
| Hover | moves selection; click activates |
| Clipboard preview | right pane 42 % width, margins 12, r 12, overlay .04, 1 px separator; mono 12 px text; hint "↩ Copiar ⇧⌦ Apagar" 11 tertiary |

Results order (no mode, non-empty query): Calculator (1) · Apps (≤6, subtitle genericName/comment) · Settings pages (≤3) · Actions (≤3) · Files (≤8) · Web search "Pesquisar “q” na web" (DuckDuckGo). Modes: Ctrl+1 Apps (all apps when empty), Ctrl+2 Actions+Settings, Ctrl+3 Clipboard (also ⌘⇧V), Ctrl+4 Files (≥2 chars). Same chip/key toggles mode off.
Keys: ↑/↓ select (clamped, no wrap), Enter activate (closes then runs), Esc close, Ctrl+Space Quick Look (`sushi`) on file, Shift+Delete delete clipboard item. Open: ⌘Space.

### services/Calculator.js (supported syntax)
- Input ≤ 160 chars; lowercased; `,`→`.`, `×`→`*`, `÷`→`/`, `x` between digit/`)` and digit/`(` → `*`.
- Precedence-climbing: `+ -` (1) < `* /` (2) < `^` (4, **right-assoc**); unary `+/-` binds at level 3 (so `-2^2 = -4`); postfix `%` = ÷100; parentheses; nesting depth ≤ 40.
- Numbers: `123`, `1.5`, `.5`, exponent `1e3`, `2.5e-4`. Constants `pi`, `e`. Functions `sqrt()`, `abs()`, `round()` (parentheses required).
- Output: `toPrecision(12)` then trimmed, decimal point rendered as **comma** (`"0,5"`); non-finite or trailing junk → no result. Enter copies value (comma→dot) via `wl-copy`. Note a bare number also yields a calc row.

---

## 7. Launchpad.qml (+ LaunchpadIcon above)

Fullscreen `kyro-overlay` (compositor blurs whole screen behind veil), exclusive keyboard.

| | Value |
|---|---|
| Veil | `rgba(5,5,13,.35)`, fade 200 ms |
| Content open | opacity 0→1 220 ms, scale **1.08→1** 320 ms OutCubic (zoom-out). No close anim |
| Search box | centred, y 56, 260×32, r 9, fill white .12, 1 px white .12; empty-state centred `search` 16 + "Buscar" 13 Medium, white .55, spacing 6; input 14 px centred white .85, margins 12 |
| Category pills | 20 px below search, spacing 8, height 30 r 15, width content+26; glyph 15 filled + label 13 DemiBold, spacing 6; idle white .12 + 1 px white .10; hover white .20; **selected white .90, text/icon `#1c1c1e`, no border**; colour 120 ms; row opacity .45 while typing |
| Grid | 32 px below pills, bottom margin 40, centred; cols = clamp(floor((W-120)/150), 3, 7); cell 150×140; vertical scroll (no pages) |
| Cell | icon 84 px + 8 + label 12 px Medium white, width 130 centred, elide, `Raised` text style (shadow rgba(0,0,0,.35), 1 px down); hover scale 1.05, press 0.9, 140 ms OutCubic; hit area inset 10 |
| Empty | "Nenhum resultado" 18 px white .55 |

Keys: typing filters (searches all apps), Tab/Shift+Tab cycle category, Enter opens first, Esc / click empty closes. Open: ⌘A or dock tile.

---

## 8. MissionControl.qml

Fullscreen `kyro-overlay`, exclusive keyboard.

| | Value |
|---|---|
| Dim | black .45, fade 250 ms (+ compositor blur) |
| Spaces bar | centred row, y 24 (animates from -140, 320 ms OutCubic), spacing 16 |
| Space thumb | 160×100 r 10, black; border 3 accent if current else 1 white .25; wallpaper cover-cropped inset by border; window outlines scaled (x/W·160, y/H·100), r 3, `rgba(51,51,56,.9)`, 1 px white .25 |
| Space label | "Mesa N" 12 px, white (current) / white .55, 6 below |
| Add tile | 60×100 r 10, white .10 (hover .20), `add` 28 |
| Window area | x 60, y 170, w = W-120, h = H-230 |
| Grid | `cols = max(1, ceil(sqrt(n·(aw/ah)/1.5)))`, `rows = ceil(n/cols)`, cw=aw/cols, ch=ah/rows; `s = min((cw-40)/w, (ch-50)/h, 1)`; last row centred: `rowOffset = (aw - inRow·cw)/2`; `x = 60 + rowOffset + col·cw + (cw - w·s)/2`, `y = 170 + row·ch + (ch - 30 - h·s)/2` |
| Window anim | from real geometry to slot, x/y/w/h 340 ms OutCubic (reverse on close not visible — window destroyed) |
| Thumbnail | r 10, bg `#1c1c1e`, static screencopy; **hover: 3 px accent border**, content inset 3; shadow .5, r 19.2, y +6 |
| Caption | 8 below thumb: app icon 22 + title pill h 22 r 6, black .5 (hover accent), 12 px white, width min(text+16, thumbW); spacing 6; fade 200 ms |
| Empty | "Nenhuma janela nesta mesa" 18 px white .55 |

Click window → close + focus it; click space → switch; Esc / background click → close. Open ⌘↑.

---

## 9. AppSwitcher.qml (⌘Tab)

| | Value |
|---|---|
| Panel | screen-centred, height 150, width = row + 28, radius **26**, menuBg, 1 px menuBorder |
| Shadow | black .5, r 40, y +10 |
| Row | y 16, spacing 6; cell 96×96 r 18, selected bg surfaceHover (fg .14); app icon 80 |
| Minimised marker | dot 5 Ø secondaryLabel, 3 above cell bottom |
| Name | 13 px DemiBold label, 12 above panel bottom |
| Animation | none (selection jumps) |

Order: MRU windows, one entry per app. Super+Tab next / Super+Shift+Tab prev (index wraps); releasing Super commits (focus window), Esc cancels. Pointer input masked out.

---

## 10. ControlCenter.qml

Top 35, right 6, width 320 (+24 shadow pad each side in the surface).

| | Value |
|---|---|
| Panel | radius **22**, menuBg, 1 px menuBorder, padding 12, vertical spacing 10 |
| Shadow | black .45, r 21.6, y +8 |
| Open | opacity 110 ms, scale 0.96→1 180 ms OutCubic, origin **top-right** |
| Module | radius 18, `overlay(.08)`, 1 px `overlay(.05)` |
| Row 1 | two columns (spacing 10 → each 143 wide): left module h 128 (2 toggles: Wi-Fi/Rede, Endereço IP), right two modules h 59 each (Foco/DND – purple, Modo Jogo – green) |
| Toggle | height 34; circle 32 Ø (on accent-colour / off overlay .12, 160 ms), glyph 18 filled; title 13 DemiBold, subtitle 11 Normal secondary, 9 px after circle; module margin 12 |
| Quick tiles row | 4 tiles, spacing 10 (~66.5×62), radius 18; glyph 20 filled + label 11 Medium tnum, spacing 4; on → its accent (Gravar red, Noturna orange, default blue); pressed overlay .16; disabled opacity .4; colour 160 ms. Tiles: Captura, Gravar (shows elapsed), Noturna, Bloquear |
| Slider module | h 64; title 13 DemiBold at (14, 9); track: margins 12 L/R/B, height 22, full radius, bg overlay .14; fill **pure white `#ffffff`** min width 22; glyph 15 filled `#7a7a7e` at x 4 inside track; value = (mx+24)/trackW from press/drag; fill width anim 90 ms. Modules: Tela (brightness), Som (click glyph = mute) |
| Now playing (if player) | h 64, margin 10, art 44 r 8, title 13 DemiBold, artist 12 secondary, SymbolButtons play/pause 24, fast_forward 22 |
| Height | ≈ 348 (+74 with media) content + 24 |

Services: Net, Notifs.dnd, GameMode, Recorder, Config.nightShift, Brightness, Audio, Media, Shell.

---

## 11. NotifCenter.qml (click on clock)

Top 35, right 6, width **340**. **No shared background**: separate floating cards, column spacing 10, **no shadow** on cards; opacity fade 120 ms.

| | Value |
|---|---|
| Card | radius 18, menuBg, 1 px menuBorder |
| Calendar card | height = calendar + 28, calendar centred |
| MiniCalendar | header "Novembro 2026" 12 DemiBold; grid 7 cols, col width 26; weekday row "D S T Q Q S S" (Sunday first) 26×15, 10 DemiBold fg .55; day cells 26×17, row spacing 1; days 11 Normal fg .85; **today**: 19×17 capsule r 8.5 accent, 11 Bold white; 5 rows (6 only if needed) |
| Header | margins 6: "Notificações" 13 Bold; "Limpar" 12 secondary (hover label) |
| Notif card | h 64, margin 12, spacing 10; icon tile 34 r 9 surfaceHover + `notifications` 18 filled secondary; summary 13 DemiBold; time 11 Normal tertiary; body 12 Normal secondary (1 line). Max 6 |

---

## 12. Notch.qml (summary)

Centred at top of each screen, inside the bar surface. Black silhouette (`NotchShape`): top edge flush with screen, concave "ears" at top corners, rounded bottom corners, all **quadratic curves with control point at the corner**:
```
total width = bodyW + 2·ear; h = max(bodyH, ear+2); r = min(radius, h-ear, bodyW/2); rx = ear+bodyW
M0,0  Q(ear,0) ear,ear  L ear,h-r  Q(ear,h) ear+r,h  L rx-r,h  Q(rx,h) rx,h-r  L rx,ear  Q(rx,0) width,0  Z
```
A second black copy 1 px larger (x-1, w+2, h+1, r+0.5, ear+0.5) is drawn underneath to kill the light halo at AA edges.

| Mode (priority high→low) | Body size (w×h) | Shape |
|---|---|---|
| auth (polkit) | 420×180 | open |
| drop (file drag) | 640×150 | open |
| capture (screenshot toast, 8 s) | 460×122 | open |
| notif | 440×88; hovered/expanded 480×min(168, 32+max(38,text)+(actions?40:0)+14) | open |
| open (hover 200 ms / click) | **640×168** | open |
| charging (2.8 s after plug-in) | (185+2·104)=393×32 | closed |
| hud (volume/brightness, 1.5 s) | (185+2·96)=377×32 | closed |
| music (playing, not in game mode) | (185+2·38)=261×32 | closed |
| idle | **185×32** | closed |

- Closed shape: ear 6, bottom radius 14; open: ear 19, bottom radius 24. Size springs: open (0.42, 0.8), closed (0.45, 1.0); radii springs ζ 1.
- Content per mode: fixed width (no reflow), centred; enters with Spring(0.4,ζ1) / exits Spring(0.22,ζ1): opacity = p, blur (1-p)·30 px, scale-Y 0.6→1 from top (side "wing" content scales X 0.6→1 instead).
- Idle: `user@host` 9 px Medium white .55 over active-window title 11 DemiBold white .85, centred, width body-28, line spacing -1. Game mode: green 5 px dot at right 9.
- Music: art 22×22 r 6 at x 9; visualizer right 11, max height 14, colour lighter(media accent,1.15).
- HUD: glyph 18 filled at x 14; bar 64×5 r 2.5 white .2 track, fill white .85, right 16, 90 ms.
- Charging: `bolt` 16 + "Carregando" 12 DemiBold green left; "NN%" 12 DemiBold + 24×12 r 3.5 battery (green border .6), fill grows 900 ms OutCubic.
- Notif: app name 11 DemiBold (red if critical) at x 18 in top 32 px; "agora"/"+N" right 18; icon 38 r 10 (image or blue/red tile with 22 glyph) at x 16, y 30; summary 13 Bold, body 12 white .55 (1 line, 4 when expanded); action pills h 28 r 14 white .13 (hover .22) 12 DemiBold; close X SymbolButton 14 pad 4. Auto-advance after `notifSeconds` (5) unless hovered/critical. Left click invoke, right click next.
- Open: tab pills (Início / Bandeja) at x 16, h 26 r 13, current white .14, hover .07; right: CPU/RAM stats 11 DemiBold white .55. Content inset top 42, sides/bottom 18: NowPlaying 236 wide | 1 px separator | week strip + date 12 white .4 | separator | round Game Mode button 84 Ø (white .10/.16 hover; green .22 when on), scale .93 press.
- Drop: two cards r 20 — Tray (dashed 2.5 px `#0091ff` dash 3.5/3) and Copy (filled `rgba(0,56,115,.6)`, hot `rgba(0,77,153,.75)`); hot target scale 1.02 OutBack 160 ms; target = copy if x > 55 % width.
- Hover leave closes after 250 ms (and resets tab). Wheel over notch = volume ±5 %. **No drop shadow implemented** (design.md mentions 70 % black, code doesn't draw it).

---

## 13. LockScreen.qml

| | Value |
|---|---|
| Background | wallpaper cover, decoded at **half resolution**, blur r 33.6 (0.7·48), brightness −0.12 |
| Date | top at 10 % H, centred; "Segunda-feira, 16 de novembro" 22 px DemiBold white .85 |
| Time | directly below, **112 px Bold, letter-spacing −2**, white .82 |
| Login column | bottom margin 10 % H, centred, spacing 12 |
| Avatar | 64 Ø circle; `~/.face` or `#0b0f1f` + KyroLogo 64 % (`#eef1ff` crescent, `#6c7dff` dot) |
| Name | 15 px DemiBold white |
| Password field | 220×32, r 16, white .18, 1 px white .20; text 13 px white, left 14, mask char "●"; placeholder 13 Normal white .55: "Digite a senha" / "Verificando…" / "Senha incorreta" (`#ffb4b0`) |
| Go button | 24 Ø, right 4 inside field, white .12 (→ .30 when text), `arrow_forward` 16 white |
| Wrong password shake | translateX: →−12 (50 ms) →+12 (70) →−8 (60) →+6 (60) →0 (50), linear |
| Power buttons | bottom-right margin 24, spacing 18: circles 40 Ø white .16 (hover .28), glyph 20 white; label 11 px white .75, 4 below (Repouso, Reiniciar, Desligar) |

PAM service `kyro-lock`; Enter submits; ext-session-lock.

---

## 14. ScreenshotBar.qml (⇧⌘5, summary)

Bottom-centred, 110 px above screen bottom (+24 inner). Toolbar h 52, r 14, menuBg, 1 px `overlay(.12)`, shadow .5 r≈25.6 (0.8·default blurMax 32), y +6. Row spacing 4: Close (`cancel`) | sep | full (`screenshot_monitor`), window (`web_asset`), area (`screenshot_region`) | sep | record full (`screen_record`), record area (`select` + red 7 px dot) | sep | "Opções ▾" | accent "Capturar"/"Gravar" button.
- Tool button 42×36 r 8, glyph 22 label; selected overlay .20, hover .10; unavailable opacity .35.
- Separators 1×26 overlay .16, margins 4.
- Options button h 36 r 8 (overlay .14 open/hover); action button h 32 r 8 accent (hover lighter 1.1, press darker 1.2), 13 DemiBold textOnAccent.
- Hover hint bubble above: h 22 r 6 menuBg, 11 px.
- Options menu 220 wide, r 12: headers 11 DemiBold secondary; items h 24 r 5, hover accent with textOnAccent; check at x 6, text x 24.
- Enter: opacity 0→1 and y +10→0, 140 ms OutCubic. Keys ←/→ mode, Enter run, Esc close; click outside closes.

---

## 15. System assets (fonts, icons, cursor)

| Asset | Source | License (as stated) |
|---|---|---|
| UI font **Inter** (variable) | Arch `inter-font`; fontconfig makes Inter the `sans-serif` default; GTK `Inter 11` | (OFL 1.1, upstream; not stated in repo) |
| Icon glyph font **Material Symbols Rounded** variable | Arch `ttf-material-symbols-variable` | (Apache-2.0 upstream; not stated in repo) |
| Monospace **JetBrainsMono Nerd Font** | Arch `ttf-jetbrains-mono-nerd` | (OFL upstream) |
| Fallback | `noto-fonts`, `noto-fonts-emoji` | — |
| App icon theme **WhiteSur** (`WhiteSur-dark` / `WhiteSur-light` by mode) | `kyro-setup` downloads github.com/vinceliuice/WhiteSur-icon-theme @ `73d8040da51a9ed74e47c7366e7e9ff437601a5c`, `./install.sh -d /usr/share/icons` | **GPL-3.0** (stated) |
| Cursor **MacTahoe-cursors** (size 24) | vendored in `assets/cursors/MacTahoe-cursors` from github.com/vinceliuice/MacTahoe-icon-theme (cursors/dist) @ `839848b9a8a38a92a6936e30c4abe35cc6f2546d` (= AUR `mactahoe-cursor-theme-git`) | **GPL-3.0** (stated, LICENSE file included) |
| GTK theme | `adw-gtk3` / `adw-gtk3-dark` (`adw-gtk-theme`), window buttons on the left | — |
| No SF Pro / SF Symbols are installed or referenced. |

Font rendering: grayscale AA, slight hinting, no subpixel (avoids colour fringes on translucent bar).

---

## 16. Shared animation/shadow cheat-sheet

| Surface | Radius | Fill | Border | Shadow | Open anim |
|---|---|---|---|---|---|
| MacMenu | 12 | menuBg | 1 menuBorder | .45, r21.6, y8 | op 90 lin + scale .97→1 140 OutCubic |
| BarPopup | 12 | menuBg | 1 menuBorder | .45, r21.6, y8 | op 100 |
| Control Center | 22 | menuBg | 1 menuBorder | .45, r21.6, y8 | op 110 + scale .96→1 180 OutCubic (top-right) |
| Notif Center cards | 18 | menuBg | 1 menuBorder | none | op 120 |
| Spotlight | 24 | menuBg | 1 menuBorder | .5, r40, y12 | op 120 + scale .96→1 180 OutCubic; height 140 OutCubic |
| App Switcher | 26 | menuBg | 1 menuBorder | .5, r40, y10 | none |
| Dock plate | 20 | dockBg | 1 overlay .16 | none (compositor blur only) | slide spring .35/1 |
| Dock tooltip | 8 | menuBg | 1 overlay .14 | none | none |
| Screenshot bar | 14 | menuBg | 1 overlay .12 | .5, r25.6, y6 | op + y10→0 140 OutCubic |
| MC thumbnails | 10 | #1c1c1e | 3 accent on hover | .5, r19.2, y6 | geom 340 OutCubic |
| Windows (compositor) | 12 (power 3) | — | 0 | black .333, range 18, y6 | spring 260/25, popin 90% |

All translucent panels rely on compositor blur (Kawase size 16 × 4 passes) behind them; without blur the menuBg alphas (.72 dark / .90 light) and dockBg (.42/.65) will look wrong — implement a backdrop blur before compositing these surfaces.
