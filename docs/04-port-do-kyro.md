# Port do kyro (moonlast) para o Obelisk

O [kyro](../../moonlast) é um shell em QML/Quickshell sobre o Hyprland. O Obelisk reimplementa o mesmo
design e as mesmas features **em Rust, dentro do próprio compositor**, sem Hyprland. As medidas vêm de
[`03-especificacao-kyro.md`](03-especificacao-kyro.md), extraída do código QML do kyro.

Legenda: ✅ feito e visto em captura · 🟡 feito, sem teste de interação real · ⬜ ainda não.

## Fase 1 (feita)

| Feature do kyro | Obelisk | Observação |
|---|---|---|
| Tokens de design (rótulos 85/55/25/10 %, cores HIG 2025, escuro padrão) | ✅ | `ui/theme.rs` |
| Vidro (blur atrás dos painéis) | ✅ | wallpaper desfocado e saturado na CPU (como o *xray* do Hyprland): não desfoca janelas por trás |
| Barra Tahoe: transparente, 32 px, texto claro/escuro pela luminância do wallpaper (> 0,62), sombra no texto | ✅ | |
| Menus da barra: sistema, app (negrito), Janela, Ir; troca ao passar o mouse | ✅ / 🟡 | renderização vista; troca por hover não testada com mouse real |
| Pontos das mesas na barra (atual = pílula 18×6) | ✅ | |
| Status: som (rolar muda volume), Bluetooth, bateria desenhada, rede (Wi‑Fi/cabo), glifo da Central, relógio "Seg 16 de nov  19:10" | ✅ | via `nmcli`, `wpctl`, `bluetoothctl`, `brightnessctl`, `playerctl` |
| Menus NSMenu: raio 12, linhas 24, coluna de marca, atalhos, destaque azul | ✅ | |
| Central de Controle: Wi‑Fi, Bluetooth, Foco, Aparência, tiles, sliders de Tela e Som, tocando agora | ✅ / 🟡 | visual verificado; arrastar sliders não testado com mouse real. Tiles Noturna e Bloquear ficam desativados (fase 2) |
| Dock: placa 64/raio 20, ícones 52, magnificação gaussiana 1,6×, ponto 4 px, separadores, salto duplo | ✅ / 🟡 | magnificação e salto não vistos em movimento |
| Dock: Launchpad desenhado, fixados, abertos não fixados, minimizados, Lixo (cheio/vazio) | ✅ | minimizados usam cartão + ícone, não miniatura real |
| Dock: menu de contexto (Abrir/Nova Janela, Manter no Dock, Encerrar), fixação salva em `~/.config/obelisk/dock` | ✅ / 🟡 | |
| Busca (⌘Espaço): 680 px a 22 %, seções Calculadora · Aplicativos · Ações · Arquivos · Web | ✅ | busca de arquivos com os limites do kyro (6 níveis, 12 mil entradas, 1,2 s) |
| Calculadora: `+ - * / ^ %`, `pi`, `e`, `sqrt/abs/round`, `1e3`, vírgula; Enter copia | ✅ | testes unitários |
| Launchpad (⌘A): busca, 7 categorias, grade 150×140 com ícone 84 | ✅ | |
| Mission Control (⌘↑): grade do kyro, miniaturas **ao vivo** (buffers reais reescalados na GPU), barra de mesas, "+" | ✅ | animação de 340 ms implementada, não vista em movimento |
| ⌘Tab: painel, ícones 80, ordem de uso, soltar ⌘ confirma | ✅ / 🟡 | soltar a tecla não testado com teclado real |
| Mesas ⌘1–9, ⌃⌘1–9 move a janela | 🟡 | |
| ⌘Q fecha, ⌘M minimiza, ⌘H oculta, ⌘F tela cheia, ⌘↩ terminal, ⌘E arquivos, ⌘, ajustes, ⇧⌘E sair | 🟡 | |
| Captura ⇧⌘3 (tela inteira) em `~/Imagens/Capturas de Tela`, copiada com `wl-copy` | ✅ | feita pelo compositor; só no backend aninhado |
| Ícones WhiteSur e cursor MacTahoe (GPL-3.0), Material Symbols Rounded, Inter | ✅ | cursor via protocolo `cursor-shape` |
| Janelas: raio 12, sombra 33 %, semáforo colorido na janela ativa e glifos ao passar o mouse | ✅ | |

## Fase 1.5 (feita)

| Feature do kyro | Obelisk | Observação |
|---|---|---|
| Notch: ocioso, música (capa + visualizador), HUD volume/brilho, "Carregando", aberto (mídia, calendário em faixa, CPU/RAM, Modo Jogo) | ✅ / 🟡 | forma com curvas quadráticas e molas SwiftUI; abrir por hover não testado com mouse real |
| Modo Jogo (detecção automática, efeitos desligados, leituras pausadas, perfil de desempenho, ponto verde) | ✅ / 🟡 | detecção vista com a flag de teste; nenhum jogo real rodado |
| Encaixe de janelas (bordas, cantos, topo, atalhos) | ✅ / 🟡 | encaixe pelo teclado visto; arrastar até a borda não testado |
| Forçar Encerrar (⌥⌘Esc) | ✅ | |
| Teclas de mídia/volume/brilho com HUD | 🟡 | |

## Fase 2 e seguintes (não feitas)

| Feature do kyro | Por que ainda não |
|---|---|
| Notch: bandeja de arquivos, notificações tipo Dynamic Island, cartão de captura, autenticação | o notch base foi feito (ver abaixo); esses modos dependem de notificações/arrastar arquivos/polkit |
| Notificações + Central de Notificações (clique no relógio) | precisa de um servidor D-Bus `org.freedesktop.Notifications` (zbus) |
| Tela de bloqueio e login gráfico (⌘L) | precisa de PAM e do protocolo `ext-session-lock` |
| ⇧⌘4 / ⇧⌘5 (área, janela, gravação, timer), cartão da captura no notch | |
| Histórico da área de transferência (⇧⌘V) e modos ⌃1–4 da Busca | |
| Menus de status próprios (lista de redes Wi‑Fi com senha, dispositivos Bluetooth, saída de áudio) | hoje os ícones abrem a Central de Controle |
| Luz Noturna e brilho por software | precisa de `wlr-gamma-control` no compositor |
| Ocultação inteligente do Dock, pastas Downloads/Documentos no Dock, arrastar para reordenar | |
| Monitor de Atividade | |
| Ajustes do Sistema (25 páginas), App Store (Flathub), Jogos, Boas-vindas, app OTP | apps inteiros; no kyro são QML |
| Modo Console (gamescope), MangoHud, Proton-GE, kernel zen, controles | o Modo Jogo base foi feito |
| Instantâneos btrfs, drivers automáticos, instalador próprio | o Obelisk usa `archinstall` |
| Bandeja do sistema (StatusNotifierItem) | |
