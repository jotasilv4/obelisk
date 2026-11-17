# Obelisk — Design System v0.1

Fonte de verdade: [`branding/tokens.json`](../branding/tokens.json). O código Rust do compositor/shell espelha esses valores (um módulo `theme` carregando o JSON ou constantes geradas dele). Este documento explica **por que** cada número existe.

> Unidades: pixels **lógicos** (multiplicados pelo `scale` da saída, ex.: 1.0, 1.25, 1.5, 2.0). Todos os valores são decisões nossas, calibradas para *parecer* com o padrão macOS, não cópias de valores proprietários.

## 1. Princípios

1. **Calma antes de densidade**: muito respiro, poucos elementos por tela.
2. **Profundidade por material**: translucidez + blur + sombra suave comunicam hierarquia (janela > menu > dock > desktop), não bordas pesadas.
3. **Cantos contínuos**: formas "squircle" (superelipse) em ícones; raios generosos em contêineres.
4. **Cor com propósito**: UI quase neutra; o accent azul indica foco/seleção/ação primária.
5. **Movimento com física**: molas e curvas de desaceleração; nada linear.

## 2. Cores

### 2.1 Papéis semânticos (claro / escuro)

| Papel | Claro | Escuro | Justificativa |
|---|---|---|---|
| `window_bg` | `#FFFFFF` | `#1E1E24` | Escuro levemente azulado, evita preto puro (menos "halo" em OLED) |
| `window_bg_secondary` | `#F5F5F7` | `#26262E` | Áreas rebaixadas (listas, painéis) |
| `sidebar_bg` | `#F0F0F4` a 80 % | `#2A2A32` a 70 % | Translúcida: recebe blur do que está atrás |
| `menubar_bg` | branco 35 % | `#1E1E24` 35 % | Barra quase transparente sobre o wallpaper (estilo Tahoe) |
| `dock_bg` | branco 35 % | branco 12 % | Vidro fosco; no escuro o branco baixo dá o "brilho" |
| `text_primary` | `#1D1D22` | `#FFFFFF` | Contraste ≥ 15:1 |
| `text_secondary` | 60 % do primário | 60 % | Metadados, legendas |
| `text_tertiary` | 30 % | 30 % | Placeholders/desabilitado |
| `separator` | preto 10 % | branco 10 % | Hairline de 1 px |
| `accent` | `#2F7BF5` | `#3D8BFF` | Azul próprio; escuro é ~8 % mais claro para manter contraste |
| `selection_bg` | = accent | = accent | Seleção ativa; inativa usa cinza 15 % |
| `focus_ring` | accent 50 % | accent 60 % | Anel de 3 px |

### 2.2 Semáforos (traffic lights)

Fechar `#FF5F57`, Minimizar `#FEBC2E`, Maximizar `#28C840`. São convenção de UI amplamente difundida (não propriedade). Inativos: cinza 12 % (claro) / 15 % (escuro). Glifos (×, −, ⤢) aparecem só no *hover* do grupo; cor do glifo = tom 55 % mais escuro do botão.

### 2.3 Accents selecionáveis

Azul `#2F7BF5` (padrão), roxo, rosa, vermelho, laranja, amarelo, verde, grafite. O usuário escolhe em Ajustes; troca `accent`/`selection_bg`/`focus_ring` em tempo real.

### 2.4 Materiais (vibrancy)

Cada superfície translúcida = *backdrop blur* + *saturação* + *tint* (cor `*_bg` com alfa). Ver `blur` no JSON.

| Superfície | Raio de blur (px) | Saturação | Observação |
|---|---|---|---|
| Menubar | 24 | 1.8× | |
| Dock | 30 | 1.8× | |
| Menus/Popovers | 30 | 1.8× | |
| Sidebar | 40 | 1.6× | só quando a janela é ativa |
| Spotlight / Central de Controle | 50 | 1.8× | mais "grosso" por ser overlay |

**Implementação sugerida (Rust/GLES):** *dual-Kawase blur* em 3–4 passes sobre o framebuffer atrás do elemento, com cache por frame; fallback sem blur → tint opaco (`alpha ≥ 0.92`) quando `reduce_transparency` ou GPU fraca.

## 3. Geometria

| Token | Valor | Racional |
|---|---|---|
| `radius.window` | 12 px | Janela moderna, discretamente arredondada. (Aproximação visual do padrão de mercado; sem fullscreen → 0) |
| `radius.menu` | 10 px | Menus ligeiramente menos arredondados que janelas |
| `radius.menu_item` | 5 px | Realce de item recuado 5 px da borda do menu |
| `radius.dock` | 22 px | Dock "pílula suave", proporcional a ícones de 56 px |
| `radius.spotlight` | 18 px | Overlay grande, canto generoso |
| ícone | squircle n = 5 | `\|x\|ⁿ + \|y\|ⁿ = 1` — transição de curvatura contínua (diferente de retângulo com cantos de círculo) |

**Squircle no Rust:** gerar o contorno por parametrização (como em `branding/gen.py::squircle`) → máscara de stencil/alpha pré-renderizada por tamanho. Para janelas (retângulos grandes) basta SDF de retângulo arredondado com raio 12 (fragment shader).

**Espaçamento:** grade de 4 px (`xs`4, `sm`8, `md`12, `lg`16, `xl`24, `xxl`32).

## 4. Elevação e sombras

| Elemento | dy | blur | alfa | Nota |
|---|---|---|---|---|
| Janela em foco | 18 | 50 | 0.34 | sombra grande e difusa (spread −4 evita vazar nos cantos) |
| Janela sem foco | 10 | 30 | 0.20 | recuo visual = hierarquia |
| Menu / popover | 8 | 24 | 0.28 | |
| Dock | 6 | 20 | 0.20 | |
| Ícone | 4 | 8 | 0.30 | |
| Hairline de borda | 1 px | — | 10 % (claro) / 18 % (escuro) | em janelas e dock, dá definição ao vidro |

Sombra de janela: pré-calculada como 9-patch borrado (uma textura por tamanho de janela em cache) ou SDF gaussiano aproximado no shader.

## 5. Tipografia

- **UI:** Inter (stand-in livre de SF Pro; licença OFL). Pilha: `Inter, Inter Variable, Cantarell, sans-serif`.
- **Mono:** JetBrains Mono (OFL).
- **Serifada:** Source Serif 4 (stand-in livre de uma serifada editorial).
- **NÃO** distribuir SF Pro/SF Mono/New York (licença Apple restritiva).

| Estilo | Tamanho | Peso | Uso |
|---|---|---|---|
| caption | 11 | 400/500 | legendas, rótulos do dock (12) |
| body / menu | 13 | 400 | corpo e menus (equivale ao Body padrão do desktop macOS) |
| headline | 13 | 600 | títulos de seção em listas |
| title3 | 15 | 400/600 | |
| title2 | 17 | 600 | |
| title1 | 22 | 600 | |
| large title | 26 | 700 | |
| spotlight input | 22 | 300/400 | |

Altura de linha 1.3×. Tracking ≈ −0,1 px em 13 px; ≈ −0,3 px em ≥ 22 px. Renderização: `cosmic-text` (shaping) + `swash`/`tiny-skia`, com *subpixel positioning*; sem hinting forte, grayscale AA (como o macOS moderno).

## 6. Tamanhos de shell

| Token | Valor | Racional |
|---|---|---|
| `menubar_height` | 28 | confortável para alvo de clique 28 px; texto 13 px |
| `titlebar_height` | 38 | barra de título + toolbar unificada |
| `traffic_light_diameter` | 12 | proporção clássica de botões de janela |
| `traffic_light_gap` | 8 | |
| `dock_icon` | 56 | repouso |
| `dock_icon_magnified` | 88 | zoom máx. (≈1,57×) |
| `dock_magnify_radius` | 120 | distância do cursor em que o zoom decai (cosseno) |
| `dock_height` | 72 | 56 + 2×8 padding |
| `dock_bottom_margin` | 6 | flutuante, não colado na borda |
| `spotlight_width` | 680 | |
| `control_center_width` | 340 | |

**Fórmula de magnificação do dock:** `scale(d) = 1 + (M−1) · ½(1+cos(π·min(d/R,1)))`, com `M = 88/56`, `R = 120`, `d` = distância horizontal do cursor ao centro do ícone.

## 7. Movimento

Duração (ms): instant 80 · fast 150 · base 220 · slow 350. Easings *cubic-bezier*: standard `(.25,.1,.25,1)`, decelerate `(0,0,.2,1)`, accelerate `(.4,0,1,1)`, emphasized `(.2,0,0,1)`. Molas (stiffness/damping/mass): default 300/26/1, bouncy 220/14/1, snappy 500/34/1.

| Animação | Timing | Detalhe |
|---|---|---|
| Abrir janela | 220 ms decelerate | escala 0.94 → 1 + fade |
| Fechar janela | 160 ms accelerate | 1 → 0.96 + fade |
| Minimizar | 420 ms | "genie" simplificado: warp da janela para o ícone no dock (malha 16×16 com interpolação por *ease-in-out*) |
| Overview (Mission) | 350 ms mola default | janelas migram para a grade |
| Workspace switch | 380 ms | deslize horizontal com mola snappy |
| Bounce do dock | 700 ms | 3 saltos decrescentes ao abrir app |
| Spotlight | 140 ms | fade + escala 0.97 → 1 |

`reduce_motion` → todas viram fade de 100 ms.

## 8. Ícones

- Grade 512×512; corpo 464×464 com 24 px de inset (espaço para sombra).
- Formato: squircle `n=5`, gradiente vertical (claro em cima → escuro embaixo), **brilho** (gloss) branco 40 % → 0 na metade superior, **filete** interno branco 28 %, sombra 0/7 px blur 7 a 30 %.
- Glifo: 1 forma simples, centrada, branca ou de contraste, cantos arredondados, sem texto.
- Conjunto inicial (12): Arquivos, Terminal, Navegador, Ajustes, Launcher, Lixeira, Notas, Música, Fotos, Calendário, Calculadora, Loja — em `branding/svg/icons/`.
- Ícones de UI (setas, busca, etc.): tema de símbolos **monolineares** 1.5 px de traço, cantos arredondados, 3 pesos (regular/medium/semibold) e 3 escalas (S 13 / M 16 / L 20) — a criar (roadmap), para casar com Inter.

## 9. Marca

- **Marca:** obelisco afunilado com pirâmide, duas faces (clara/escura) e plinto. Uma forma única, funciona a 16 px e em um tom só.
- **Wordmark:** "Obelisk" em Inter SemiBold com tracking −4 (em 188 px), cor `#14182E` (claro) ou branco.
- **Cores da marca:** índigo `#2D3FB5` → `#171D5C`, azul `#4F7FF0`, lavanda `#C9D6FF`.
- **Proteção:** margem mínima = largura do plinto; tamanho mínimo 16 px (marca) / 96 px (wordmark).
- Assets: `branding/svg/`, `branding/wallpapers/`, rasters via `branding/render.sh`.

## 10. Acessibilidade (tokens associados)

- Contraste mínimo 4.5:1 para texto 13 px sobre qualquer material; se medir < 4.5 → aumentar alfa do tint.
- `reduce_transparency`: materiais viram sólidos (alfa 0.95).
- `increase_contrast`: separadores 1 px a 30 %, bordas visíveis em controles.
- `reduce_motion`: ver §7.
- Escala de texto: multiplicador global 0.85–1.5 sobre `font.sizes`.
- Cursor grande: 24 → 48 px.
