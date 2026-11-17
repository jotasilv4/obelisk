# Análise do design e das funções do macOS — base para o Obelisk

Objetivo: entender **o que torna o macOS reconhecível** (design, ícones, marca, funções) para o Obelisk reproduzir os *padrões e a sensação*, com implementação e assets 100 % próprios, em Rust (compositor Wayland com Smithay + shell próprio).

**Legenda de status (coluna "Obelisk"):**
- **MVP** = planejado para a primeira versão utilizável (v0.1–0.2)
- **Roadmap** = desejado, depois do MVP
- **Não replicável** = depende de hardware/serviço/propriedade da Apple, ou não faz sentido no Linux

**Aviso sobre números:** valores marcados com **≈** são aproximações de observação visual/documentação pública e variam entre versões do macOS. Nenhum deles é "norma"; os valores oficiais do Obelisk estão em [`02-design-system.md`](02-design-system.md).

---

## 0. Regras de IP (propriedade intelectual) — leia primeiro

O macOS é protegido por marcas registradas, direitos autorais e *trade dress*. No Obelisk:

| Podemos | Não podemos |
|---|---|
| Replicar **padrões de interface** (barra de menus no topo, dock embaixo, botões de janela à esquerda, busca central, vidro fosco) | Usar o **logotipo da maçã** ou qualquer variação reconhecível |
| Usar formas genéricas (squircle, gradientes, sombras suaves) | Copiar, traçar ou "redesenhar fielmente" **ícones de apps da Apple** (Finder, Safari, Mail, Fotos etc.) |
| Escolher nomes descritivos ("Arquivos", "Navegador") | Usar nomes/marcas Apple (Finder, Spotlight, Mission Control, AirDrop, Time Machine, Siri…) como nome de produto |
| Usar **Inter** (OFL) como substituta de fonte do sistema | Distribuir **SF Pro / SF Mono / New York** (licença restrita) |
| Criar wallpapers e sons originais | Redistribuir **wallpapers, sons de boot/sistema, cursores** da Apple |
| Referenciar o macOS em documentação para fins de análise | Alegar afiliação ou dizer "compatível com macOS" |

Termos internos neutros recomendados na UI: `Spotlight → Busca`, `Mission Control → Visão Geral`, `Launchpad → Apps`, `Finder → Arquivos`, `Control Center → Central de Controle`. "Dock" é termo genérico de painel de lançadores e pode ser mantido (ou trocado por "Base" se preferirem).

Outras regras: ícones e wallpapers do Obelisk são **originais** (ver `branding/`); fontes só OFL/livres; sons a produzir/licenciar livremente (CC0/CC-BY); código sob licença escolhida pelo projeto (sugestão: GPL-3.0-or-later ou MPL-2.0, compatível com Smithay MIT).

---

## 1. Filosofia de design

| Princípio | Como aparece no macOS | Tradução para o Obelisk | Obelisk |
|---|---|---|---|
| **Clareza e deferência** | UI recua; o conteúdo é o centro; cromo mínimo | Janelas com titlebar unificada, controles discretos | MVP |
| **Consistência** | Mesma barra de menus, atalhos e componentes em todos os apps | Shell dita atalhos (⌘-like) e a barra de menus global | MVP (shell) / Roadmap (toolkits) |
| **Profundidade** | Camadas translúcidas, sombras difusas, blur | Materiais com blur no compositor | MVP |
| **Direct manipulation** | Arrastar e soltar em todo lugar; gestos de trackpad | DnD via `wl_data_device`; gestos libinput | MVP (DnD) / Roadmap (gestos) |
| **Integração hardware+software** | Cores calibradas, trackpad háptico, ProMotion | Gestão de cor/escala por saída | Roadmap / Não replicável (háptico) |
| **Padrões sensatos** | Pouca configuração obrigatória | Primeira execução guiada, defaults bons | MVP |

---

## 2. Linguagem visual — evolução

| Era | Traços | O que reproduzir |
|---|---|---|
| **Aqua (2001–2013)** | Botões "gotas" translúcidas, listras, skeuomorfismo | Nada (datado) |
| **Yosemite→Catalina (2014–2019)** | Flat + blur, tipografia fina | Blur de sidebar, barra de menus translúcida |
| **Big Sur→Sequoia (2020–2024)** | Ícones squircle com profundidade, janelas com raio maior, barra de menus translúcida, Dock flutuante, SF Symbols | **Base principal do Obelisk** |
| **Tahoe / "Liquid Glass" (2025→)** | Materiais de vidro com refração/reflexo dinâmico, barra de menus transparente, ícones em camadas com translucidez, raios maiores, concentricidade de cantos | **Inspiração para vidro/transparência**; refração real é Roadmap |

Linhas-guia que sobrevivem em todas: cantos arredondados contínuos, sombras longas e suaves, tipografia limpa, acento azul, modo claro/escuro com *tint* de wallpaper.

---

## 3. Geometria

| Elemento | Característica no macOS | ≈ Valor | Obelisk |
|---|---|---|---|
| Janelas | Raio grande, sombra difusa, borda hairline interna | raio ≈ 10–16 px conforme versão; Tahoe maior (≈ 20+ em janelas com toolbar grande) | MVP (raio 12) |
| Ícone de app | **Squircle** (superelipse / "cantos contínuos"), grade 1024×1024 com corpo ≈ 824 px | n ≈ 5 | MVP (nossos ícones) |
| Dock | Pílula translúcida flutuante, ícones ≈ 48–64 px | raio ≈ 20–26 px | MVP |
| Menus | Raio ≈ 8–10 px, itens com realce arredondado | | MVP |
| Botões | Capsule ou retângulo raio ≈ 5–6; "push" 22 px de altura | | MVP |
| Concentricidade | Cantos internos = raio externo − padding (Tahoe enfatiza) | | MVP (regra de design) |
| Sombras | Janela em foco: grande/difusa; sem foco: menor | dy ≈ 20, blur ≈ 50, α ≈ 0.3 | MVP |

Detalhes: o contorno do squircle não é "retângulo + círculo nos cantos"; a curvatura aumenta gradualmente. Aproximamos com superelipse `|x|^5 + |y|^5 = 1` (ver `branding/gen.py`).

---

## 4. Cor e materiais

### 4.1 Cores de sistema
macOS define **cores semânticas** (label, secondaryLabel, separator, windowBackground, controlAccent…) que mudam com modo claro/escuro e com acessibilidade; e **cores de sistema** (blue, green, indigo, orange, pink, purple, red, teal, yellow, gray) com variações claro/escuro/alto contraste.

- Azul padrão ≈ `#007AFF` (claro) / `#0A84FF` (escuro) — **Obelisk usa azul próprio `#2F7BF5` / `#3D8BFF`** para não clonar a paleta.
- Cor de destaque do usuário: 8 opções (azul, roxo, rosa, vermelho, laranja, amarelo, verde, grafite) e cor de realce de seleção. **MVP**.
- **Tinting** do desktop: janelas/sidebar absorvem levemente a cor do wallpaper. **Roadmap**.

### 4.2 Materiais (vibrancy)
Tipos: *ultraThin, thin, regular, thick, ultraThick* + materiais semânticos (menu, sidebar, popover, headerView, sheet, HUD, tooltip). Fundamento: **blur gaussiano do que está atrás + aumento de saturação + tint + hairline**.

| Item | Obelisk |
|---|---|
| Blur de fundo (backdrop) por surface no compositor | MVP |
| Saturação ≈ 1.6–1.8× | MVP |
| Reduzir transparência (acessibilidade) | MVP |
| Refração/"lensing" estilo Liquid Glass (distorção da borda) | Roadmap |
| Vibrancy de texto (texto que mistura com o fundo) | Roadmap |

### 4.3 Modo claro/escuro
Troca automática por horário/ pôr do sol, transição suave; wallpapers dinâmicos (claro/escuro). Obelisk: tokens duplos (ver design system). **MVP** (manual), **Roadmap** (automático/dinâmico).

---

## 5. Tipografia

| Fonte Apple | Uso | Substituta livre no Obelisk | Obelisk |
|---|---|---|---|
| **SF Pro** (Text/Display, tamanhos ópticos) | UI geral | **Inter** (OFL, variável com eixo `opsz`) | MVP |
| **SF Mono** | Terminal/código | **JetBrains Mono** | MVP |
| **New York** (serifada) | Leitura editorial | Source Serif 4 / Noto Serif | Roadmap |
| **SF Compact / Rounded** | Watch/Apps específicos | — | Não replicável |

**Estilos de texto macOS (≈, pt = px lógicos):** Large Title 26, Title1 22, Title2 17, Title3 15, Headline 13 (semibold), Body 13, Callout 12, Subheadline 11, Footnote 10, Caption 10/9. Corpo de 13 px é menor que no iOS (17) — desktop é mais denso.

Detalhes: tracking varia por tamanho (tabela "tracking" da SF); números tabulares em relógios; *text smoothing* em grayscale. **Obelisk:** `cosmic-text` + AA grayscale; MVP.

---

## 6. Iconografia

### 6.1 Ícones de app
- Grade de produção 1024×1024; corpo squircle ≈ 824×824 com margem para sombra; **profundidade** por gradiente, brilho no topo, sombra de contato.
- Cada app com **um glifo simples** e fundo de cor sólida/gradiente; sem texto; metáfora reconhecível.
- Tahoe adiciona **camadas translúcidas** e variantes (padrão, escuro, tintado, claro-transparente).
- **Obelisk:** 12 ícones originais (squircle, gradiente + brilho) em `branding/svg/icons/`. **MVP**. Variantes escuro/tintado: **Roadmap**.

### 6.2 SF Symbols (iconografia de UI)
≈ 5 000+ símbolos, 9 pesos (ultralight→black) alinhados à fonte, 3 escalas (small/medium/large), modos de renderização (monochrome, hierarchical, palette, multicolor), variáveis (valor 0–1) e animações.

| Item | Obelisk |
|---|---|
| Conjunto próprio monolinear (traço ~1.5 px), 3 pesos × 3 escalas, SVG | MVP (conjunto mínimo ≈ 80 símbolos) / Roadmap (completo) |
| Renderização hierárquica/palette | Roadmap |
| Cópia/uso de SF Symbols | **Proibido** (licença só para plataformas Apple) |
| Alternativa livre de partida: Lucide / Phosphor / Tabler (MIT/ISC) | MVP (opção de atalho) |

### 6.3 Cursores
Seta preta com contorno branco, I-beam, mão, lupa, relógio (beach ball). Obelisk: tema de cursor **original** XCursor/PNG (seta com contorno fino, sombra leve), 24 px / 48 px. **MVP** (cursor padrão), **Roadmap** (conjunto completo). Beach ball → **Não replicável** (usamos spinner próprio).

---

## 7. Logos e marcas

| Aspecto no macOS | Descrição | Obelisk |
|---|---|---|
| Logotipo no menu superior-esquerdo | Silhueta monocromática, abre o menu do sistema (Sobre, Preferências, Apps recentes, Forçar Encerrar, Repouso, Reiniciar, Desligar, Sair) | MVP — **marca Obelisk** (obelisco) em tamanho 14–16 px no mesmo lugar, mesmo conceito de menu do sistema |
| Boot | Tela preta/cinza com logo branco e barra de progresso fina | MVP — fundo `obelisk-login.svg` + marca + barra (Plymouth ou splash do compositor) |
| Login | Wallpaper borrado + avatar circular + campo de senha pill | MVP |
| Ícone do sistema/"Sobre" | | Roadmap |
| Wordmark | Tipografia limpa, sem decoração | MVP — `obelisk-wordmark-*.svg` |

**Regra:** nada derivado do logotipo da Apple. A marca Obelisk foi desenhada do zero (pilar afunilado + pirâmide, duas faces).

---

## 8. Componentes de shell (área de trabalho)

### 8.1 Barra de menus (Menu Bar)
- Global, no topo, ≈ 24 px (≈ 37 px em telas com notch); em Tahoe, **transparente**.
- Esquerda: logo do sistema → **nome do app ativo (negrito)** → menus do app (Arquivo, Editar, Visualizar…).
- Direita: *menu extras* (Wi-Fi, Bluetooth, bateria, som, **Central de Controle**, busca, relógio).
- **Menu global (global menu)**: o app ativo coloca seus menus na barra do topo.
- Obelisk: barra própria 28 px, logo + app ativo + status items à direita (Wi-Fi, volume, bateria, relógio, Central de Controle). Menus por app: via `dbusmenu`/`com.canonical.dbusmenu` + GTK/Qt appmenu (Roadmap); MVP exibe só o nome do app e menu do sistema. | **MVP** (barra e status) / **Roadmap** (menu global dos apps)

### 8.2 Dock
- Pílula translúcida embaixo (ou lateral); ícones com **magnificação** por proximidade; **indicador** (ponto) de app aberto; **bounce** ao abrir/notificar; **separador** entre apps e pastas/lixeira; **minimizar para o dock** com animação genie/scale; pastas com leque/grade; clique direito → opções.
- ≈ ícone 48–64 px; zoom até ≈ 128 px.
- Obelisk: dock com magnificação (cosseno), ponto de app aberto via `wlr-foreign-toplevel`/protocolo interno, bounce, separador, Lixeira, arrastar para reordenar. | **MVP** (núcleo) / **Roadmap** (pastas em leque, posição lateral, autohide completo)

### 8.3 Finder (gerenciador de arquivos)
Visualizações ícones/lista/colunas/galeria; barra lateral (Favoritos, iCloud, Locais, Tags); caminho; painel de visualização; tags coloridas; Quick Look; Smart Folders; "Ir para pasta".
Obelisk: app **Arquivos** próprio (Rust, ex.: iced/egui/gtk4-rs) com colunas/lista/ícones. | **Roadmap** (MVP usa gerenciador existente, ex.: Nautilus/Thunar/pcmanfm, apenas tematizado ou com ícones Obelisk)

### 8.4 Spotlight (busca universal)
Atalho ⌘+Espaço; caixa centrada com resultados agrupados (apps, arquivos, calculadora/conversões, dicionário, web, ações); prévia à direita; filtros; ações rápidas em Tahoe (executar comandos).
Obelisk: **Busca** (overlay centralizado 680 px) com apps (.desktop), arquivos recentes (índice `tantivy`/`plocate`), calculadora inline (`fend`/`meval`), conversões, comandos de sistema, web. | **MVP** (apps+calc+comandos) / **Roadmap** (arquivos, prévia, ações)

### 8.5 Mission Control / Exposé / Spaces
Visão geral das janelas agrupadas por app + barra de Spaces no topo; mover janelas entre Spaces; tela cheia vira Space próprio; gestos de 3/4 dedos.
Obelisk: **Visão Geral**: grade de miniaturas (via captura de surface), workspaces como faixa no topo, arrastar para mover. Gestos: swipe 3/4 dedos (libinput). | **MVP** (overview + workspaces) / **Roadmap** (gestos fluidos, agrupar por app)

### 8.6 Launchpad / "Apps"
Grade de ícones paginada, busca, pastas. (No Tahoe o Launchpad foi substituído por uma vista **Apps** integrada ao Spotlight.) Obelisk: **Apps** = grade paginada de `.desktop`, busca, pastas por arrastar. | **MVP**

### 8.7 Central de Controle (Control Center)
Painel em vidro, tiles: Wi-Fi, Bluetooth, AirDrop, Foco, Espelhamento, Brilho, Volume, Reprodução; tiles expansíveis; menus extras fixáveis. Obelisk: tiles Wi-Fi (NetworkManager D-Bus), Bluetooth (BlueZ), Brilho (backlight), Volume (PipeWire/WirePlumber), Não Perturbe, Modo noturno, Tema claro/escuro. | **MVP** (Wi-Fi, volume, brilho, tema) / **Roadmap** (resto)

### 8.8 Central de Notificações e Widgets
Notificações em banners no canto superior-direito, agrupadas por app, ações inline; painel lateral (clicar no relógio) com widgets e calendário. Obelisk: servidor `org.freedesktop.Notifications` próprio em Rust (`zbus`), banners em vidro, histórico; widgets simples (calendário, clima, bateria). | **MVP** (banners+histórico) / **Roadmap** (widgets)

### 8.9 Stage Manager
Modo que agrupa janelas do mesmo app/tarefa em "palco" central e miniaturas laterais. | **Roadmap** (depois de Overview estável)

### 8.10 Hot Corners
Ação ao encostar o cursor num canto (Mission Control, Desktop, Proteção de tela, Notas rápidas). | **MVP** (4 cantos configuráveis)

### 8.11 Janelas: controles e estrutura
| Elemento | Descrição | ≈ | Obelisk |
|---|---|---|---|
| **Semáforos** | 3 círculos à esquerda: fechar (vermelho), minimizar (amarelo), zoom/tela cheia (verde); glifos no hover; cinza se janela inativa | Ø ≈ 12 px, espaçamento ≈ 8 px | MVP (decoração desenhada pelo compositor via `xdg-decoration` server-side) |
| **Barra de título + toolbar unificada** | Título centralizado, botões na mesma linha, arrastável | ≈ 28–52 px | MVP (CSD respeitado se o app desenha; SSD para o restante) |
| **Sidebar** | Material translúcido, seleção arredondada na cor de destaque | | Roadmap (toolkit) |
| **Sheets** | Painéis modais presos à barra de título, deslizam de cima | | Roadmap (via `xdg_dialog`) |
| **Popovers** | Balão com seta, vidro | | Roadmap |
| **Tela cheia** | Vira Space, barra de menus some e aparece ao encostar no topo | | MVP |
| **Dividir tela (tiling)** | Segurar o botão verde → esquerda/direita; Sequoia: tiling por snap | | MVP (snap metade/quartos) |
| **Redimensionar/mover** | Bordas de 1 px de hit ampliado | | MVP |

### 8.12 Switching de apps
⌘+Tab alterna **apps** (não janelas) com painel de ícones; ⌘+` alterna janelas do mesmo app. Obelisk: `Super+Tab` (switcher de apps, painel em vidro), `Super+\`` janelas do app. | **MVP**

---

## 9. Movimento e animação

| Animação | macOS | ≈ | Obelisk |
|---|---|---|---|
| Abrir/fechar janela | Escala 0.9→1 + fade (Sonoma); ease-out | 200–250 ms | MVP |
| **Genie / Scale** ao minimizar | Janela "escorre" até o dock | 400–600 ms | MVP simplificado (warp de malha ou escala+translação); genie completo Roadmap |
| Mission Control | Janelas migram com mola | ≈ 350 ms | MVP |
| Troca de Space | Deslize horizontal com inércia (gestos) | ≈ 300–400 ms | MVP |
| Dock magnification | Contínua, sem *easing* perceptível (segue cursor) | tempo real | MVP |
| Bounce de app | 3 saltos | ≈ 700 ms | MVP |
| Springs (SwiftUI) | `response`/`dampingFraction` | | MVP (engine de molas própria) |
| Redução de movimento | Troca por *cross-fade* | | MVP |
| Taxa de quadros | 60/120 Hz (ProMotion) VRR | | MVP (sincronizado no vblank), VRR Roadmap |

---

## 10. Som

Sons característicos: **chime de boot**, "Basso", "Glass", "Pop", clique de lixeira esvaziando, captura de tela (obturador), notificações, "ding" de erro. Todos protegidos. Obelisk: banco de sons **original** (CC0) para alerta, notificação, lixeira, captura, login/boot (opcional, desligado por padrão), reproduzido via PipeWire/`rodio`. | **MVP** (alerta + notificação + captura) / **Roadmap** (resto)

---

## 11. Boot, login e bloqueio

| Etapa | macOS | Obelisk |
|---|---|---|
| Boot | Logo + barra de progresso | Plymouth (ou splash do compositor) com marca Obelisk | **Roadmap** (MVP: splash do GRUB/systemd-boot) |
| Login | Avatares circulares, campo de senha arredondado, wallpaper borrado, Touch ID | Greeter próprio em Rust no compositor (`greetd` + `obelisk-greeter`): avatar, senha, fundo `obelisk-login` | **MVP** |
| Bloqueio | Mesmo visual; relógio grande em Tahoe | `ext-session-lock-v1` + tela de bloqueio própria | **MVP** |
| FileVault/Touch ID | Criptografia/biometria | LUKS (instalador); fprintd Roadmap | **Roadmap** |

---

## 12. Wallpapers

Padrão: imagens abstratas coloridas/paisagens, **dinâmicas** (claro/escuro/hora do dia), *tela de bloqueio* igual ao desktop borrado. Obelisk: wallpapers originais (blobs com blur) claro e escuro 4K em `branding/wallpapers/`; modo dinâmico (alternar por tema/hora) via `wp-viewporter` + cross-fade. | **MVP** (estático claro/escuro) / **Roadmap** (dinâmico por hora)

---

## 13. Acessibilidade

VoiceOver (leitor de tela), Zoom, Texto maior, Cursor grande, Reduzir movimento/transparência, Aumentar contraste, Teclado de acessibilidade, Controle por voz/Switch, Legendas. Obelisk: reduzir transparência/movimento, aumentar contraste, texto maior, cursor grande, zoom no compositor (`magnifier`) — **MVP** (reduzir transparência/movimento, cursor/texto grandes); leitor de tela via **Orca + AT-SPI**; **Roadmap**. Voz/Switch: **Roadmap**.

---

## 14. Funções do sistema e produtividade

| Função (macOS) | O que faz | Equivalente Obelisk | Obelisk |
|---|---|---|---|
| **Spotlight** | Busca universal ⌘+Espaço | Busca (§8.4) | MVP |
| **Quick Look** | Barra de espaço → prévia imediata | `obelisk-preview` (imagens/PDF/texto/vídeo via `image`, `pdfium`/`mupdf`, `symphonia`) acionado pelo gerenciador de arquivos | Roadmap |
| **Mission Control/Spaces** | Visão geral e áreas de trabalho | §8.5 | MVP |
| **Hot Corners** | Ações nos cantos | §8.10 | MVP |
| **Cmd (⌘) como modificador de app** | ⌘C/V/X/Z/A/S/W/Q/N/T/, usados em todos os apps; `Ctrl` fica p/ terminal/emacs | Opção "Modo Mac": no compositor, remapear `Super+<tecla>` → `Ctrl+<tecla>` **apenas para apps GUI** (não terminal), mantendo `Super+Tab/Espaço/Q/W/M/H/,` como atalhos do shell | MVP (opt-in) |
| **Atalhos de sistema** | ⌘Q sair, ⌘W fechar janela, ⌘M minimizar, ⌘H ocultar, ⌘, preferências, ⌘⇧3/4/5 capturas, ⌘⌥Esc forçar encerrar, ⌃⌘Q bloquear | Equivalentes no compositor (`Super+Q/W/M/H/,`, capturas, `Super+Alt+Esc`, `Super+Ctrl+Q`) | MVP |
| **Captura de tela** (⌘⇧3/4/5) | Tela inteira/região/janela/gravação, miniatura flutuante | `wlr-screencopy`/`ext-image-copy-capture`, UI de seleção + miniatura, gravação via PipeWire | MVP (imagem) / Roadmap (vídeo) |
| **Time Machine** | Backups incrementais com timeline | Btrfs/`snapper` ou `restic` + UI "Cápsula do Tempo" | Roadmap |
| **AirDrop / Continuity / Handoff / Universal Clipboard** | Integração iPhone↔Mac | KDE Connect / LocalSend (protocolos abertos) | Roadmap parcial / **Não replicável** (iPhone/iCloud nativos) |
| **iCloud** | Sincronização de arquivos/fotos | Nextcloud/Syncthing integrados | Roadmap |
| **Handoff/Sidecar/Continuity Camera** | Recursos de ecossistema Apple | — | **Não replicável** |
| **Siri/Apple Intelligence** | Assistente | Roadmap: integração opcional com assistente local; fora do núcleo | Roadmap |
| **Atalhos (Shortcuts)/Automator** | Automação visual | Scripts + `obelisk-actions` (CLI/D-Bus) | Roadmap |
| **Focus (Não Perturbe)** | Modos de foco | Servidor de notificações com modos | MVP (DND) / Roadmap (modos) |
| **Night Shift / True Tone** | Cor quente/adaptativa | `wlr-gamma-control` (temperatura de cor por horário) | MVP (Night) / **Não replicável** (True Tone requer sensor) |
| **Gatekeeper/Notarização/SIP** | Verificação de apps | Pacman + assinaturas + Flatpak sandbox | **Não replicável** (modelo diferente) |
| **Keychain / Senhas** | Cofre de credenciais | `libsecret` (org.freedesktop.secrets) próprio, Roadmap | Roadmap |
| **FileVault** | Criptografia total | LUKS no instalador | Roadmap |
| **Gestos do trackpad** | Pinch, 2/3/4 dedos, inércia | `libinput` gestos no compositor | Roadmap (MVP: scroll natural, tap-to-click) |
| **Preview / Notas / Fotos / Música / Calendário / Calculadora / Contatos…** | Apps nativos integrados | Apps livres existentes com ícones Obelisk; apps próprios simples (Calculadora, Notas) depois | Roadmap |
| **App Store** | Loja única | Loja = frontend para pacman/Flatpak ("Loja") | Roadmap |
| **Menu "Sobre este Mac"** | Resumo de hardware | `obelisk-about` | MVP |
| **Arrastar para a lixeira / Esvaziar** | Lixeira com ícone cheio/vazio | `trash-rs` (FreeDesktop Trash) + ícone de estado | MVP |
| **Renomear em linha, tags, pilhas (Stacks)** | Organização no desktop/Finder | Pilhas no desktop Roadmap | Roadmap |
| **Múltiplos monitores** | Barra de menus em cada tela, Dock no principal, Spaces por tela | Suporte multi-saída no compositor (`wl_output`, `xdg-output`) | MVP |
| **HiDPI/escala** | Retina 2×, escalas fracionadas | `wp-fractional-scale-v1`, `wp-viewporter` | MVP |
| **Spotlight em apps (Pesquisa no menu Ajuda)** | Buscar itens de menu | Depende do global menu | Roadmap |

---

## 15. Resumo de escopo para o MVP (v0.1)

Compositor Wayland (Smithay) com: janelas flutuantes + snap, decoração com semáforos, sombras, blur de materiais, workspaces, **barra de menus**, **dock** com magnificação, **Busca** (apps+calc), **Apps** (grade), **Visão Geral**, **Central de Controle** básica, **notificações**, **login/bloqueio**, tema claro/escuro, hot corners, capturas de tela, atalhos "estilo Mac", wallpapers/ícones/marca originais.

Fora do MVP: global menu de apps, Quick Look, Stage Manager, Time Machine, Continuity, refração Liquid Glass, VoiceOver-equivalente completo, apps próprios (Arquivos, Notas…).

**Estado real de implementação:** este escopo é o *plano*. O que existe hoje (v0.1) e o que foi
verificado está em [`README.md`](../README.md#estado-do-v01).

## 16. Fontes de consulta (para comparar versões)

- Documentação pública de diretrizes de interface da Apple (conceitos e terminologia)
- Observação direta do sistema em máquinas licenciadas pelo usuário (valores ≈)
- Smithay (`anvil`), `wlroots`, `cosmic-comp`/COSMIC e `niri` como referências de implementação de compositor em Rust
