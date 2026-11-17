# O que o macOS tem e o Obelisk ainda não

Atualizado em 2026-11-16 (terceira rodada). Foco do Obelisk: **desempenho e jogos**.
Legenda: ✅ feito e visto funcionando · 🟡 feito, sem teste real (ex.: só no backend nativo, que nunca rodou)
· ⬜ falta · — não se aplica (serviço/hardware da Apple).

## Jogos e desempenho

| Recurso | Estado | Observação |
|---|---|---|
| Apps OpenGL/Vulkan pela GPU (linux-dmabuf) | ✅ | `es2gears` e `vkcube` rodaram dentro do Obelisk |
| Steam, Proton e apps X11 (XWayland) | ✅ | `glxgears` (X11) rodou com a barra de título do Obelisk; Steam/jogo real não testado |
| Mira com mouse em FPS (relative-pointer + pointer-constraints) | 🟡 | só tem efeito no backend nativo (o aninhado não entrega movimento relativo) |
| O app se declara jogo (content-type) → Modo Jogo | 🟡 | |
| Jogo/vídeo impede o bloqueio (idle-inhibit) | 🟡 | |
| Modo Jogo + Feral GameMode (prioridade de CPU/GPU no processo do jogo) | 🟡 | registra o PID no `gamemoded` por D-Bus |
| Taxa variável (VRR / ProMotion), só com jogo em tela cheia | 🟡 | backend nativo |
| Jogo em tela cheia composto sozinho (*direct scanout*) | 🟡 | backend nativo |
| Sem *tearing* quando o jogo pede (`wp_tearing_control`) | ⬜ | o Smithay 0.7 não traz o protocolo nem *async page flip*; precisa de atualização do Smithay |
| Forçar Encerrar (⌥⌘Esc) e Monitor de Atividade | ✅ | |
| Modo de Pouca Energia (menu da bateria) | ✅ | via power-profiles-daemon |
| Composição ociosa: ~0,5 % de CPU parado | ✅ | medido |
| Latência menor de controles Bluetooth no Modo Jogo | ⬜ | exige mudar parâmetros do BlueZ/kernel (root); não há API de usuário |
| Modo Console (Steam Big Picture no gamescope, com FSR do gamescope) | 🟡 | atalho na Busca e no menu; Steam não testada aqui |
| Gravação de tela (⇧⌘5: tela ou área) | ✅ | quadros vão ao ffmpeg (H.264, até 60 fps); o notch mostra ponto vermelho e tempo, clique para parar |

## Área de trabalho

| Recurso | Estado | Observação |
|---|---|---|
| Notificações (servidor D-Bus) no notch, estilo Dynamic Island | ✅ | testado com `notify-send` num D-Bus privado |
| Central de Notificações (clique no relógio) | ✅ | |
| Foco/Não Perturbe e Modo Jogo silenciam banners | ✅ / 🟡 | |
| Tela de bloqueio (⌘L, ⌃⌘Q, menu, Central, inatividade) com PAM | ✅ / 🟡 | tela vista; senha não testada aqui (o faillock do Arch conta erros) |
| Capturas ⇧⌘3, ⇧⌘4 (área), ⇧⌘5 (tela/janela/área, timer), Print | ✅ | recorte conferido pixel a pixel |
| Animações de abrir e minimizar (para o Dock) | ✅ | desligadas no Modo Jogo; fechar ainda sem animação |
| Encaixe de janelas, canto ativo, App Exposé | ✅ / 🟡 | |
| Gestos de trackpad (3 dedos, pinça) | 🟡 | só backend nativo |
| Night Shift | 🟡 | gamma no backend nativo; desativado no aninhado |
| Aparência automática (claro 7h–19h) | ✅ | |
| Bandeja de status (Discord, Steam…) com menu do app | ✅ | testado com um indicador Ayatana: ícone na barra e menu dbusmenu |
| ⌘C/⌘V nos apps (⌘ vira Ctrl; Ctrl+Shift em terminais) e tabela de atalhos do macOS | 🟡 | só na sessão real (aninhado o ⌘ é o Alt); inserção sintética de teclas testada no `foot` |
| Desfoque real das janelas atrás dos painéis | ⬜ | **deliberado**: custaria GPU em todo quadro (contra o foco em jogos); o vidro desfoca só o papel de parede |
| Stage Manager (miniaturas ao vivo à esquerda) | ✅ | |
| Widgets na mesa (relógio, calendário, sistema) | ✅ | |
| Emojis (⌃⌘Espaço) coloridos, inseridos no app | ✅ | emoji colado no terminal; só emojis de um código (sem sequências ZWJ/bandeiras) |
| Manter na Frente (PiP) | ✅ | menu Janela |
| Ajustes do Sistema | ✅ / 🟡 | as 26 páginas do kyro (Wi‑Fi com IP/DNS manual, Acesso Pessoal, Bluetooth, Som, Modo Jogo, Controles, Proton‑GE, Kernel, Aparência, Papel de Parede, Dock, Telas, Bateria, Bloqueio, Instantâneos, Atualizações, Teclado, Mouse, Data e Hora, Idioma, Apps Padrão, Impressoras, Armazenamento, Sobre, Usuários); leitura do sistema vista funcionando; ações com root via pkexec + agente polkit não testadas com senha. Inventário em `docs/06-ajustes-kyro.md` |
| Instantâneos (papel do Time Machine) | 🟡 | snapper + snap-pac pelo instalador em discos btrfs |
| Zoom de acessibilidade (⌥⌘8, ⌥⌘=, ⌥⌘−) | ✅ | |
| Leitor de tela (VoiceOver), AirDrop/Continuity | ⬜ | leitor de tela exige AT-SPI em todo o shell; AirDrop: LocalSend está só no AUR/Flathub |
| Siri, iCloud, Sidecar, True Tone | — | |
