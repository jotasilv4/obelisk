# 06 — Ajustes do Sistema do kyro (inventário para reimplementação)

Fonte: `moonlast/config/quickshell/settings/*.qml`, `services/*.qml`, `bin/kyro-*`. Somente leitura.
Convenções: `S.x` = chave `x` em `~/.config/kyro/settings.json` (via `Config.s`). "Elevação" = como o kyro obtém root.

---

## 0. Infraestrutura comum

### 0.1 Janela (`SettingsApp.qml`)
| Item | Valor |
|---|---|
| Título da janela | `Ajustes do Sistema` (FloatingWindow 900×620, fundo transparente) |
| Abrir/fechar | `Shell.settingsOpen`; página inicial = `Shell.settingsPage`; `Shell.openSettings(id)` abre direto numa página (usado por Spotlight, Controles→Bluetooth, Usuários→Bloqueio, Data→Teclado) |
| Barra lateral | 236 px, `Theme.sidebarBg`; separador 1 px `rgba(0,0,0,.5)` |
| Topo da lateral | semáforo (TrafficLights, fecha = `settingsOpen=false`), altura 26 |
| Busca | campo 28 px, raio 8, ícone `search`, placeholder `Buscar` (13 px, tertiaryLabel) |
| Cartão do usuário | 54 px, raio 9; Avatar 40 px + `User.displayName` (13 DemiBold) + `Usuário, foto e senha` (11); clique → página `users`; destacado com accent quando `page==="users"` |
| Cabeçalho de categoria | 24 px, texto 11 Bold tertiaryLabel; clique recolhe/expande (estado `collapsed[título]` só em memória); chevron `expand_more` visível em hover (rotação −90° quando recolhido, 150 ms) |
| Item de página | 30 px, raio 7; ícone = quadrado 22×22 raio 6 na cor da página + Material Symbol preenchido 15 px branco; título 13 px; selecionado = fundo accent + `textOnAccent`; hover = overlay 6% |
| Conteúdo | `Theme.windowBg`; título da página em (24,14) 15 px Bold = `SettingsPages.byId(page).title`; Flickable com topMargin 50, conteúdo x=24, largura−48 |

### 0.2 Busca na lateral
```
filter = texto.toLowerCase()
visiblePages(grupo) = pages.filter(p => p.id !== "users" && available(p) && (filter==="" || (p.title+" "+p.keywords).toLowerCase().includes(filter)))
grupo.visible = visiblePages.length > 0
grupo.open = filter !== "" || !collapsed[titulo] || visiblePages contém a página atual
```
- Substring simples, sem normalização de acento (só lower-case). `users` nunca aparece como item (é o cartão).
- `available(p)`: `requires:"battery"` → `BatteryInfo.present` (UPower DisplayDevice é bateria de laptop); `requires:"wifi"` → `Net.wifiDevice !== ""`.
- Spotlight usa o mesmo índice (`SettingsPages.all`, mesmo filtro, top‑3 na busca geral; todos no modo "Ações"), seção `Ajustes do Sistema`, subtítulo `Ajustes`, ação `Shell.openSettings(id)`.

### 0.3 Categorias e páginas (ordem exata)
| # | Categoria | id | Título | Ícone (Material Symbols) | Cor | keywords | requires |
|---|---|---|---|---|---|---|---|
| 1 | Conexões | network | Rede | `lan` | #0091ff | wi-fi wifi internet ip ethernet cabo | — |
| | | hotspot | Acesso Pessoal | `wifi_tethering` | #30d158 | hotspot roteador compartilhar internet celular tethering ponto de acesso | wifi |
| | | bluetooth | Bluetooth | `bluetooth` | #0091ff | fone controle mouse sem fio | — |
| | | printers | Impressoras | `print` | #8e8e93 | imprimir impressora cups scanner fila papel | — |
| 2 | Notificações e Som | notifications | Notificações | `notifications` | #ff4245 | alertas foco não perturbe | — |
| | | sound | Som | `volume_up` | #ff375f | áudio volume alto-falante microfone saída entrada | — |
| 3 | Jogos | game | Modo Jogo | `sports_esports` | #30d158 | jogos desempenho fps steam | — |
| | | controllers | Controles | `stadia_controller` | #30d158 | controle joystick gamepad xbox playstation dualsense dualshock switch bateria testar botões | — |
| 4 | Personalização | appearance | Aparência | `contrast` | #3a3a3c | cor destaque tema claro escuro automático janelas efeitos | — |
| | | dock | Dock | `dock_to_bottom` | #3a3a3c | ampliação ocultar ícones tamanho | — |
| | | display | Telas | `desktop_windows` | #0091ff | monitor resolução escala hz frequência luz noturna night shift | — |
| | | wallpaper | Papel de Parede | `landscape` | #00d2e0 | fundo imagem | — |
| 5 | Energia e Segurança | battery | Bateria | `battery_full` | #30d158 | energia carregar notebook pouca energia saúde | battery |
| | | lock | Bloqueio e Energia | `lock` | #3a3a3c | senha tela bloqueada repouso suspender desligar tela | — |
| 6 | Teclado e Mouse | keyboard | Teclado | `keyboard` | #8e8e93 | layout abnt2 idioma teclas atalhos área de transferência clipboard histórico | — |
| | | mouse | Mouse e Trackpad | `mouse` | #8e8e93 | sensibilidade aceleração rolagem natural | — |
| 7 | Sistema | users | Usuários (só cartão) | `account_circle` | #0091ff | conta foto avatar imagem nome senha perfil usuário | — |
| | | datetime | Data, Hora e Região | `schedule` | #0091ff | relógio fuso horário hora data idioma região 24 horas ntp | — |
| | | defaults | Apps Padrão | `apps` | #8e8e93 | abrir com navegador padrão player vídeo música pdf imagem | — |
| | | updates | Atualizações | `system_update` | #8e8e93 | pacman atualizar software | — |
| | | storage | Armazenamento | `hard_drive` | #8e8e93 | disco espaço livre limpar cache lixo ssd hd pacotes | — |
| | | snapshots | Instantâneos | `history` | #30d158 | backup time machine snapshot restaurar btrfs snapper | — |
| | | about | Sobre | `info` | #8e8e93 | sistema versão hardware cpu memória disco | — |

`byId(id)` cai em `all[0]` (Rede) se id desconhecido.

### 0.4 Controles (widgets)
| Widget | API | Visual |
|---|---|---|
| `SGroup` | `title`, `footer`, filhos | título 13 px x=4; caixa raio 10, fundo overlay 5%, borda overlay 6%; rodapé 11 px secondaryLabel, wrap; espaçamento 6 |
| `SRow` | `label`, `subtitle`, `symbol`, `symbolColor` (default accent), filhos à direita | altura 40 (padrão); separador 1 px entre linhas (x=12); ícone 22×22 raio 6; label 13, subtitle 11 secondaryLabel; controles à direita com spacing 8 |
| `MacSwitch` | `checked`, sinal `toggled(bool)` | pílula, on = accent, off = overlay 16%, botão branco, anim 140 ms. **Não muda sozinho**: o pai grava e o binding relê |
| `MacSlider` | `value` 0..1, `stepSize` (0=contínuo), sinal `moved(v)` | trilho 4 px overlay 16%, preenchido accent, knob branco 20 px |
| `MacSelect` | `options:[{label,value}]`, `value`, `maxWidth` 300, sinal `activated(v)` | botão com `unfold_more`; popup com `check` na opção atual; mostra `—` se valor não está nas opções |
| `MacField` | `text`, `placeholder`, `password`, `invalid`, `readOnly`, sinais `edited(t)` / `accepted` (Enter) | raio 6, borda 2 px red se inválido / accent com foco |
| `MacButton` | `text`, `primary`, sinal `clicked` | raio 7; primário = accent + `textOnAccent` |

### 0.5 settings.json (`Config.qml`)
- Arquivo `~/.config/kyro/settings.json`, JsonAdapter com watch: editar à mão recarrega; toda mudança reescreve o arquivo inteiro; se não existe é criado com defaults.
- Defaults: `accent:"blue"`, `appearance:"dark"`, `barStyle:"transparent"`, `wallpaper:""`, `effectsProfile:"visual"`, `windowMode:"mac"`, `pinned:["org.gnome.Nautilus","firefox","kyro-games","steam","kyro-store","foot","org.gnome.TextEditor","kyro-settings"]`, `dockMonitor:"primary"`, `dockSize:52`, `dockMagnify:true`, `dockAutohide:false`, `dockSmartHide:true`, `dockClickMinimize:false`, `dockFolders:true`, `edgeSnap:true`, `notifSeconds:5`, `nightShift:false`, `softBrightness:1.0`, `nightTemp:4200`, `kbLayout:"br"`, `kbVariant:""`, `mouseSensitivity:0`, `mouseAccel:false`, `naturalScroll:false`, `monitors:[]`, `loginLock:true`, `autoGameMode:true`, `gameClasses:[]`, `mangohud:"off"`, `welcomeDone:false`, `noiseSuppression:false`, `clock24:true`, `clockDate:true`, `bootSession:"desktop"`, `batteryPercent:true`, `lockAfter:300`, `screenOffAfter:600`, `suspendAfter:0`, `clipboardHistory:true`, `barSound:true`, `barBluetooth:true`, `captureMode:"area"`, `captureDelay:0`, `captureCursor:false`, `captureMic:false`.
- Cores de destaque (`Config.accents`): blue #0091ff, purple #db34f2, pink #ff375f, red #ff4245, orange #ff9230, yellow #ffd600, green #30d158, graphite #8e8e93 (ordem de exibição = essa).
- Aplicação ao compositor: `hyprctl eval '<lua>'` (Hyprland 0.56 Lua). Na reimplementação Rust isso vira chamada interna ao compositor. Reaplicado no evento `configreloaded` e 800 ms após iniciar.

### 0.6 Elevação de privilégio (resumo)
| Mecanismo | Usado em |
|---|---|
| polkit via D‑Bus (agente gráfico próprio `Quickshell.Services.Polkit` → diálogo no notch) | `timedatectl set-ntp/set-timezone` (systemd-timedated), `nmcli` (NetworkManager, sessão local ativa costuma não pedir), `powerprofilesctl` (ppd) |
| `pkexec <cmd>` (mesmo agente polkit) | `usermod -c` (nome completo), `paccache`, `journalctl --vacuum-size` |
| `sudo` dentro de terminal `foot --app-id=kyro-update -T "<título>" sh -c "...; echo; read -p '...' x"` | `pacman -Syu`, `kyro-kernel zen/default`, `kyro-locale set`, `kyro-rollback N`, `kyro-drivers` (usa sudo internamente), `passwd` (sem sudo) |
| sem root (config do snapper `ALLOW_USERS="$USER"`, `SYNC_ACL="yes"` gravado por `kyro-setup`) | `snapper create/delete/list` |

---

## 1. Rede (`network`) — PageNetwork.qml + services/Net.qml

### Leitura (Net.refresh) — um único `sh -c` com seções:
```
echo '#general'; nmcli -t -f RUNNING,WIFI general
echo '#devices'; nmcli -t -f DEVICE,TYPE,STATE,CONNECTION device
echo '#saved';   nmcli -t -f NAME,UUID,TYPE,DEVICE connection show
echo '#route';   ip -j route get 1.1.1.1
echo '#dns';     (resolvectl dns || grep ^nameserver /etc/resolv.conf)
echo '#wifi';    nmcli -t -f ACTIVE,SSID,SIGNAL,SECURITY device wifi list --rescan no
```
Parsing:
- Campos `-t`: separador `:`, escape `\:` e `\\` (parser char a char: `\` + próximo char = literal).
- `nm = general[0]=="running"`, `wifiEnabled = general[1]=="enabled"`.
- `wifiDevice` = 1º device com TYPE `wifi`.
- `saved` = `{name,uuid,type,device}` exceto `loopback`.
- route JSON `[0]`: `iface=dev`, `address=prefsrc`, `gateway=gateway`.
- `dns` = todos os IPv4 por regex `\d+\.\d+\.\d+\.\d+` únicos.
- `wifiConnection` = CONNECTION do device wifi se STATE=`connected`.
- `networks` = linhas wifi → `{active: f0=="yes" || ssid==wifiConnection, ssid, signal:int, secure: f3 não vazio e ≠"--"}`; descarta SSID vazio, duplicados e o SSID do próprio hotspot quando ativo; `known` = SSID ∈ nomes de conexões `802-11-wireless`; ordena active desc, known desc, signal desc.
- Ícone de sinal: >66 `wifi`, >33 `wifi_2_bar`, senão `wifi_1_bar`.

Atualização: `python3 watch_process.py network` → exec `nmcli monitor` (PR_SET_PDEATHSIG); cada linha → debounce 400 ms → refresh. Ao abrir a página: `refresh()` + `scan()`.
Scan: `nmcli device wifi rescan; sleep 2` → `scanning=false` → refresh.

### Controles (ordem)
| # | Grupo | Tipo | Texto | Lê | Escreve |
|---|---|---|---|---|---|
| 0 | (sem NM, visível se `!nm`) | info row, ícone `lan` | label `Conectado · <iface>` / `Sem conexão`; subtitle = IP | route | — ; footer: `O NetworkManager não está ativo, então a rede é configurada pelo sistema. Para usar Wi‑Fi e mudar o IP por aqui, ative-o com: sudo systemctl enable --now NetworkManager` |
| 1 | Wi‑Fi (se nm && wifiDevice) | switch, ícone `wifi` | `Wi‑Fi` | wifiEnabled | `nmcli radio wifi on|off` |
| 2 | idem | lista (redes active) | label SSID, subtitle `Conectado`, ícone `lock` se secure, botão `Detalhes…` | networks | abre Detalhes(ssid, wifi=true) |
| 3 | `Redes conhecidas` (known && !active) | lista NetworkRow | SSID, `lock`, ícone sinal, botões `Esquecer`, `Conectar` | | ver abaixo |
| 4 | `Outras redes` / `Outras redes  ·  procurando…` | lista NetworkRow (!known) | idem sem Esquecer | | |
| 4b | idem, se vazia | info + botão | `Procurando redes…` / `Nenhuma outra rede por perto`, botão `Procurar` | | `Net.scan()` |
| 5 | `Outras conexões` (ethernet salvas `802-3-ethernet`) | lista, ícone `settings_ethernet` | nome; subtitle `Conectado` (+ ` · <IP>` se é a iface principal) / `Desconectado`; `Detalhes…` | saved | Detalhes(name, wifi=false) |
| 6 | — | texto vermelho 12 px | `Net.lastError` (última linha de stderr/stdout sem `Error: `) | | |

NetworkRow: `Conectar` → se secure && !known: expande linha (76 px) com MacField senha, placeholder `Senha da rede — Enter para conectar`; Enter → connect.
```
conhecida sem senha : nmcli connection up id <ssid>
com senha           : nmcli device wifi connect <ssid> password <pwd>
aberta              : nmcli device wifi connect <ssid>
Esquecer            : nmcli connection delete id <name>
(desconectar, usado em outro lugar): nmcli device disconnect <wifiDevice>
```
Toda ação → refresh ao terminar; erro → `lastError`.

### Detalhes (subpágina, substitui a lista)
Leitura: `nmcli -t -f ipv4.method,ipv4.addresses,ipv4.gateway,ipv4.dns,IP4.ADDRESS,IP4.GATEWAY connection show id <name>` → k:v (1ª ocorrência, `\:`→`:`); addr = `ipv4.addresses` || `IP4.ADDRESS[1]` (1º da lista por vírgula) → `ip/prefix` (prefix default 24); gateway = `ipv4.gateway` (se ≠ `--`) senão `IP4.GATEWAY`; dns = `ipv4.dns` split `,` sem `--`; method = `manual` ou `auto`.

| # | Tipo | Texto |
|---|---|---|
| a | botão + título | `‹ Voltar`, nome da conexão 15 Bold |
| b | grupo `TCP/IP` · dropdown | `Configurar IPv4`: `Usando DHCP`=auto / `Manualmente`=manual |
| c | text field (readOnly se DHCP) | `Endereço IP` placeholder `192.168.0.10` |
| d | text field | `Máscara de Sub-rede` placeholder `255.255.255.0` (prefix↔máscara) |
| e | text field | `Roteador` placeholder `192.168.0.1` (opcional) |
| f | grupo `DNS` · text field 260 px | `Servidores DNS`, placeholder `Automático`; footer `Separe os servidores por vírgula. Em branco, usa os servidores informados pela rede (DHCP).` (split `[\s,]+`) |
| g | botões | `Esquecer Esta Rede` (só wifi) → forget; `Cancelar`; `OK` (primário, opacidade .4 se inválido) |

Validação: IPv4 = 4 octetos `^\d{1,3}$` ≤255; manual exige ip+máscara válidos, router vazio ou válido; todos DNS válidos.
Escrita (OK):
```
sh -c 'n="$1"; shift; nmcli connection modify id "$n" "$@" && nmcli connection up id "$n"' sh <name> \
  ipv4.method manual ipv4.addresses <ip>/<prefix> ipv4.gateway <gw>      # manual
  ipv4.method auto   ipv4.addresses ""            ipv4.gateway ""        # DHCP
  ipv4.dns <a,b> ipv4.ignore-auto-dns <yes se dns não vazio | no>
```
Root: não (polkit do NM).

---

## 2. Acesso Pessoal (`hotspot`) — PageHotspot.qml + Net.qml

Visível na lateral só se há placa Wi‑Fi. Nome do perfil NM: `Acesso Pessoal`. `hotspotActive = wifiConnection === "Acesso Pessoal"`.

Leitura ao abrir: `nmcli -s -t -f 802-11-wireless.ssid,802-11-wireless-security.psk connection show id "Acesso Pessoal"` (k:v, desescapa `\x`). Defaults se não existe: SSID `kyro de $USER`, senha aleatória 12 chars do alfabeto `abcdefghjkmnpqrstuvwxyz23456789` em blocos de 4 com `-` (`xxxx-xxxx-xxxx`).

| # | Grupo | Tipo | Texto / regra |
|---|---|---|---|
| 0 | sem placa | info `portable_wifi_off` | `Nenhuma placa Wi‑Fi encontrada`; footer `O Acesso Pessoal precisa de uma placa Wi‑Fi.` |
| 1 | — | switch `wifi_tethering` (verde se ativo) | `Acesso Pessoal`; subtitle `Ligando…`/`Desligando…` (busy) / `Ligado · nenhum aparelho conectado` / `Ligado · N aparelho(s)` / `Desligado`. Ligar só se SSID 1–32 e senha 8–63. Footer: lastError, ou `Os aparelhos usam a internet deste computador (<iface>).` (se online por outra iface), ou `Com uma placa Wi‑Fi só, ligar o Acesso Pessoal desconecta este computador do Wi‑Fi: os aparelhos só terão internet se houver um cabo de rede.` |
| 2 | `Rede` | text field 240 | `Nome da rede` (inválido se vazio/>32) |
| 3 | idem | botão olho (`visibility`/`visibility_off`) + text field senha 200 + botão `Gerar` | `Senha`; `Gerar` → randomPassword; footer `A senha precisa ter de 8 a 63 caracteres.` |
| 4 | idem (se mudou && ativo) | botão primário `Aplicar` | label `Os aparelhos precisam entrar de novo depois da mudança` → start de novo |
| 5 | `Conectar um celular` | imagem QR 160 px em caixa branca 176 raio 12 | footer `Aponte a câmera do celular para o código para entrar sem digitar a senha.` |
| 6 | `Aparelhos conectados` (se ativo) | lista, ícone `smartphone` | label nome ou `Aparelho`; subtitle `IP · MAC(maiúsc.)`; footer vazio → `Nenhum aparelho conectado ainda.` |

Comandos:
```
ligar   : sh -c 'n="$1"; shift; nmcli connection delete id "$n" >/dev/null 2>&1; nmcli device wifi hotspot con-name "$n" "$@"' sh "Acesso Pessoal" ifname <wifiDev> ssid <ssid> password <pwd>
desligar: nmcli connection down id "Acesso Pessoal"
QR      : qrencode -s 6 -m 2 -o $XDG_RUNTIME_DIR/kyro-hotspot-qr.png 'WIFI:T:WPA;S:<ssid>;P:<pwd>;;'   (escapa \ ; , : " com \)
clientes (a cada 4 s com hotspot ativo):
  iw dev <wifiDev> station dump | awk '/^Station/ {print "S",$2}'
  awk '{print "L",$1,$2,$3}' /run/kyro/hotspot-leases      # "MAC IP NOME|*"
```
`/run/kyro/hotspot-leases` é escrito pelo `dhcp-script=/usr/lib/kyro/kyro-hotspot-lease` (config `system/hotspot/kyro.conf` no dnsmasq-shared do NM; args `add|old|del MAC IP NOME`). Novo MAC após a 1ª leitura → sinal `hotspotJoined(nome||MAC)` (notificação). Root: não.

---

## 3. Bluetooth (`bluetooth`) — PageBluetooth.qml (Quickshell.Bluetooth = BlueZ D‑Bus)

Equivalente D‑Bus: `org.bluez`, `ObjectManager.GetManagedObjects` + `InterfacesAdded/Removed` + `PropertiesChanged` (lista viva, sem polling).

| # | Grupo | Tipo | Texto | Lê | Escreve |
|---|---|---|---|---|---|
| 0 | sem adaptador | info `bluetooth_disabled` cinza | `Nenhum adaptador Bluetooth encontrado`; footer `Conecte um adaptador Bluetooth e verifique se o serviço está ativo: sudo systemctl enable --now bluetooth` | | |
| 1 | — | switch `bluetooth` | `Bluetooth`; subtitle `Procurando dispositivos…` / `Ligado` / `Desligado` | Adapter1.Powered, Discovering | Adapter1.Powered = bool |
| 2 | — | switch | `Mostrar Bluetooth na barra de menus` | S.barBluetooth | S.barBluetooth |
| 3 | `Dispositivos` (se ligado) | lista de todos Device1 | label `Name` ou `Address`; subtitle `Pareando…`/`Conectando…`/`Desconectando…`/`Conectado`/`Pareado`/""; ícone por `Icon`: contém `audio`→`headphones`, `input-gaming`→`sports_esports`, `mouse`→`mouse`, `keyboard`→`keyboard`, senão `bluetooth`; cor accent se conectado | Device1.{Name,Address,Icon,Paired,Connected} + estado transitório | botão `Desconectar` → `Device1.Disconnect()`; `Conectar` → `Connect()`; `Parear` → `Pair()`; `Aguarde…` desabilitado durante transição. Footer vazio: `Nenhum dispositivo por perto. Coloque o fone, controle ou mouse em modo de pareamento.` |
| 4 | — | botão | `Procurar dispositivos` / `Parar busca` | Discovering | iniciar: `PairableTimeout=180; Pairable=true; StartDiscovery()`; parar: `PairableTimeout=0; Pairable=false; StopDiscovery()` |

Root: não. (Pareamento com PIN não tratado aqui.)

---

## 4. Impressoras (`printers`) — PagePrinters.qml (CUPS)

Leitura (ao abrir e a cada 3 s enquanto aberta):
```
sh -c "command -v lpstat >/dev/null || { echo NOCUPS; exit; }; LANG=C lpstat -p -d; echo '## jobs'; LANG=C lpstat -o | awk '{ sub(/-[0-9]+$/, \"\", \$1); print \$1 }'"
```
Parsing: `^printer (\S+) (is idle|now printing|disabled)` → idle/printing/paused; `^system default destination: (\S+)` → padrão; após `## jobs` cada linha = nome da impressora → contagem.

| # | Tipo | Texto | Ação |
|---|---|---|---|
| 0 | info `print_disabled` (sem CUPS) | `Impressão indisponível`; footer `O sistema de impressão (CUPS) não está instalado. Rode o install.sh do kyro de novo.` | |
| 1 | grupo `Impressoras`, vazio | `Nenhuma impressora` / `Ligue a impressora e clique em Adicionar` | |
| 2 | lista, ícone `print` (azul; cinza se pausada) | `<nome>` + ` (padrão)`; subtitle `Pronta`/`Imprimindo`/`Pausada` + ` · N documento(s) na fila` | `Tornar Padrão` (se não padrão) → `lpoptions -d <nome>`; `Fila…` → `system-config-printer --show-jobs <nome>` |
| 3 | linha + botão primário | `Adicionar, remover ou configurar` / `Abre o gerenciador de impressoras`, botão `Adicionar Impressora…` | `system-config-printer` |
Footer grupo: `Impressoras USB e da rede (Wi‑Fi) são encontradas ao adicionar. A primeira impressão pode demorar alguns segundos: o serviço de impressão só liga quando é usado.` Root: não (system-config-printer pede via polkit).

---

## 5. Notificações (`notifications`) — PageNotifications.qml + Notifs.qml

| # | Grupo | Tipo | Texto | Lê/Escreve |
|---|---|---|---|---|
| 1 | — | switch `dark_mode` roxo | `Não Perturbe` / `Notificações vão só para o histórico, sem aparecer no notch` | `Notifs.dnd` (memória, **não persistido**) |
| 2 | — | dropdown | `Tempo na tela` / `Quanto tempo cada notificação fica no notch`; opções 2,3,5,8,10 → `N segundos` | S.notifSeconds |
| 3 | `Histórico` | info + botão | `Nenhuma notificação` / `N notificações recentes`; botão `Limpar` (se >0) | `Notifs.history` (memória, máx. 30) → `clearHistory()` |
Servidor: o próprio shell é o `org.freedesktop.Notifications` (NotificationServer). Com DND: entra no histórico e `expire()` imediato. Modo Jogo força DND e restaura o valor anterior ao sair.

---

## 6. Som (`sound`) — PageSound.qml + Audio.qml (PipeWire nativo)

Listas (vivas via PipeWire registry):
- saídas = nodes `audio && isSink && !isStream`
- entradas = nodes `audio && !isSink && !isStream && name != "kyro_noise"`
- nome exibido = `description || nickname || name || "—"`

| # | Grupo | Tipo | Texto | Lê | Escreve (equivalente CLI) |
|---|---|---|---|---|---|
| 1 | `Saída` | dropdown | `Dispositivo de saída` | default sink id | `Pipewire.preferredDefaultAudioSink` (metadata `default.configured.audio.sink`) ≈ `wpctl set-default <id>` / `pactl set-default-sink <name>` |
| 2 | | slider 200 px (ícones `volume_mute` / `volume_up`) | `Volume de saída` | `muted?0:volume` (0..1) | desmuta e define volume clamp 0..1 ≈ `wpctl set-volume @DEFAULT_AUDIO_SINK@ <v>` |
| 3 | | switch | `Sem som` | sink muted | toggle ≈ `wpctl set-mute @DEFAULT_AUDIO_SINK@ toggle` |
| 4 | | switch | `Mostrar som na barra de menus` | S.barSound | S.barSound |
| 5 | `Entrada` | dropdown (se há mics) | `Dispositivo de entrada` | `micSource` (= mic real atrás do filtro se supressor ativo, senão default source) | `preferredDefaultAudioSource`; se supressor ligado, 400 ms depois `kyro-noise on` de novo (retarget) |
| 6 | | switch `graphic_eq` azul (desabilitado enquanto `kyro-noise` roda) | `Supressão de ruído` / `Tira ventilador, teclado e barulho de fundo da voz (RNNoise, leve)` | S.noiseSuppression | grava S.noiseSuppression e roda `kyro-noise on|off` |
| 7 | | slider 200 px (se source tem audio) | `Volume de entrada` | source volume | source volume ≈ `wpctl set-volume @DEFAULT_AUDIO_SOURCE@ <v>` |
Footer `Entrada` sem mics: `Nenhum microfone encontrado.`

`kyro-noise`:
- `on`: exige `/usr/lib/ladspa/librnnoise_ladspa.so`; `cur=$(pactl get-default-source)`; se ≠ `kyro_noise` grava em `${XDG_STATE_HOME:-~/.local/state}/kyro/noise-mic`; escreve `~/.config/kyro/noise-suppression.conf` (pipewire filter-chain LADSPA `noise_suppressor_mono`, VAD 50%, grace 200 ms, capture `target.object=<mic>`, playback `node.name=kyro_noise` `media.class=Audio/Source`, descrição `Microfone (sem ruído)`) e `~/.config/systemd/user/kyro-noise.service` (`ExecStart=/usr/bin/pipewire -c <conf>`, BindsTo pipewire); `systemctl --user daemon-reload; restart kyro-noise.service`; espera até 5 s por `kyro_noise` em `pactl list short sources`; `pactl set-default-source kyro_noise`.
- `off`: se default é `kyro_noise` volta para o salvo; `systemctl --user stop kyro-noise.service`.
- `status`: `systemctl --user is-active` → on/off.
- Shell observa o arquivo `noise-mic` (watch). 3 s após iniciar: se S.noiseSuppression e filtro inativo → `kyro-noise on`.
Root: não.

---

## 7. Modo Jogo (`game`) — PageGame.qml + GameMode.qml + Effects.qml

Ao abrir: `vulkaninfo --summary | grep -m1 deviceName; ... | grep -m1 driverName` (regex `<chave>\s*=\s*(.+)`); `sh -c "command -v gamescope && command -v steam"` (exit 0 → consoleApps); `kyro-proton list` + `kyro-proton latest`; `kyro-kernel status`.
`hasConsole = consoleApps && driver != "" && driver != "llvmpipe"`.

| # | Grupo | Tipo | Texto | Lê | Escreve |
|---|---|---|---|---|---|
| 1 | — | switch `sports_esports` verde | `Modo Jogo` / subtitle = status: `Ativado manualmente` · `Automático · N jogo(s)` · `Suspenso para estes jogos` · `Desativado` | GameMode.active | `GameMode.toggle()` (ver lógica) |
| | footer | | `Silencia as notificações e desliga blur, sombras e animações do compositor. Solicita energia em desempenho, se disponível. Ao desligar, retoma o perfil de efeitos escolhido em Aparência, as notificações e o perfil de energia anterior.` | | |
| 2 | `Ativação automática` | switch | `Ativar ao abrir um jogo` | S.autoGameMode | S.autoGameMode |
| 3 | | text field (Enter salva) | `Outros jogos` / `Classes exatas por vírgula; Enter para salvar` | `S.gameClasses.join(", ")` | `S.gameClasses = split(",").trim().filter(nonEmpty)` |
| | footer | | `Reconhece jogos Steam (steam_app_), gamescope, cs2 e janelas marcadas como jogo. Mantém o modo até fechar a última janela, mesmo ao trocar de aplicativo. O botão Modo Jogo suspende a ativação até fechar todos os jogos.` | | |
| 4 | `Modo Console` | switch `stadia_controller` índigo (desab. se !hasConsole, opac .4) | `Iniciar no Modo Console` / `Ao ligar o computador, abre direto na Steam` | `S.bootSession=="console"` | `S.bootSession = "console"|"desktop"` (lido por `bin/kyro-session`: `jq -r '.bootSession // "desktop"'`) |
| 5 | | botão `Mudar` | `Mudar para o Modo Console agora` / `Fecha a sessão e abre a Steam em tela cheia` | | `kyro-session-select console` |
| | footer | | llvmpipe: `Indisponível aqui: o Modo Console precisa de uma placa de vídeo com Vulkan (AMD, Intel ou NVIDIA com driver), e este computador só tem renderização por software.` / ok: `A Steam em tela cheia, como num console ou no Steam Deck: roda direto no gamescope, sem a área de trabalho por trás, o jeito mais leve de jogar. Para voltar, use "Mudar para Área de Trabalho" no menu de energia da Steam.` / sem apps: `Precisa da Steam e do gamescope (instalados com os jogos: ./install.sh sem --no-games).` | | |
| 6 | `Kernel` | switch `memory_alt` índigo | `Usar o kernel para jogos (linux-zen)`; subtitle `Rodando: <uname -r>` + ` · o zen entra ao reiniciar` / ` · o do Arch entra ao reiniciar` / `Verificando…` | `kyro-kernel status` → `rodando|linux|linux-zen|zen(0/1)` → checked = default=="linux-zen" | `foot --app-id=kyro-update -T Kernel sh -c "sudo kyro-kernel zen|default; echo; read -p 'Enter para fechar.' x"` → ao sair relê status |
| | footer | | `O linux-zen tem o agendador e a preempção ajustados para desktop e jogos: o sistema responde melhor com a CPU ocupada. O kernel do Arch continua instalado e aparece no menu de boot como reserva. Vale depois de reiniciar.` | | |
| 7 | `Placa de vídeo` | info `memory` | label = deviceName / `Verificando…`; subtitle `Driver Vulkan: <driverName>` | vulkaninfo | footer: llvmpipe → `Renderização por software: sem aceleração 3D. Jogos não vão rodar bem até haver uma GPU com driver.` senão `Os drivers são escolhidos automaticamente pelo fabricante da placa (AMD, Intel ou NVIDIA).` |
| 8 | | botão `Verificar e Instalar` | `Drivers de vídeo` / `Detecta a placa e instala os drivers certos (64 e 32 bits)` | | `foot --app-id=kyro-update -T "Drivers de vídeo" sh -c "kyro-drivers; echo; read -p 'Pronto. Reinicie se a placa for NVIDIA. Enter para fechar.' x"` |
| 9 | `Apps de jogos` | botão `Abrir Jogos` | `Biblioteca de jogos` / `Steam e Lutris num só lugar` | | `Shell.openGames()` |
| 10 | `Desempenho na tela` | dropdown `speed` laranja | `Mostrar nos jogos (MangoHud)`; subtitle `FPS, tempo de quadro, CPU, GPU, temperaturas e memória` / `Só o FPS, num cantinho` / `Desligado`; opções `Desligado`=off, `Só FPS`=fps, `Completo`=full | S.mangohud | S.mangohud → applyMangoHud |
| | footer | | `Vale para os jogos abertos depois de mudar (se a Steam ou o Lutris já estavam abertos, feche e abra de novo). No jogo, Shift direito + F12 mostra ou esconde.` | | |
| 11 | `Proton-GE` | info + botão `deployed_code` vermelho | `Mais nova: <tag>` / `Verificando a versão mais nova…`; subtitle instalando: `<STEP>…` ou `Baixando — N% de ~560 MB`; senão `Já instalada` / `Baixa do GitHub e confere o arquivo antes de instalar`; botão `Instalar` (primário) / `Cancelar` (SIGTERM) | | `kyro-proton install` (stdout `PROGRESS N`, `STEP Conferindo`, `STEP Instalando`, `OK <tag>`; stderr última linha → `Erro: ...` no footer) |
| 11b | | barra de progresso 4 px accent | | PROGRESS | |
| 11c | | lista instaladas | `<GE-ProtonX-Y>` / `Instalada`, botão `Remover` | `kyro-proton list` | `kyro-proton remove <nome>` |
| | footer | | `Versão do Proton com correções extras para jogos que não rodam no oficial. Depois de instalar, reabra a Steam e escolha em Propriedades do jogo > Compatibilidade; no Lutris, em Configurar > Opções do executor > Versão do Wine.` | | |
| 12 | `Sempre ativo` | info rows com `check_circle` verde | `Jogos em tela cheia vão direto para a tela` / `Direct scanout: o compositor sai do caminho`; `Tearing liberado para jogos` / `Menor latência (regra “immediate” do Hyprland)`; `VRR (FreeSync/G-Sync) em tela cheia`; `Mouse sem aceleração` / `Ajuste em Mouse e Trackpad` (ícone `cancel` cinza se S.mouseAccel) | | footer `Esses recursos valem o tempo todo, com ou sem o Modo Jogo.` |

Lógica GameMode:
- Detecção: `hyprctl -j clients` (debounce 200 ms em `openwindow/closewindow/windowtitle/windowtitlev2/configreloaded` e mudança de gameClasses/autoGameMode). Jogo = `mapped!==false` && (class lower `^steam_app_[0-9]+$` || `gamescope` || `cs2` || `contentType=="game"` || class ∈ gameClasses lower).
- `automatic = S.autoGameMode && games>0`; `active = manual || (automatic && !suppressed)`; sem jogos → suppressed=false.
- `toggle()`: ativo → `manual=false; suppressed=automatic`; inativo → `suppressed=false; if(!automatic) manual=true`.
- Ao ativar: salva DND, liga DND; perfil de efeitos efetivo = `game`. Energia: processo vivo `powerprofilesctl launch --profile performance --reason "Modo Jogo do kyro" --appid kyro sh -c "read -r kyro_hold"` (hold do ppd, liberado ao matar) se `performance` disponível.
- Perfis de efeito (`hyprctl eval`): `hl.config({ animations={enabled=A}, decoration={ blur={enabled=B,size=S,passes=P}, shadow={enabled=Sh} } })` — visual: true/true/16/4/true; balanced: true/true/5/1/true; game: false/false/3/1/false.
- MangoHud: escreve `~/.config/kyro/mangohud.conf` = `toggle_hud=Shift_R+F12`, `position=top-left`, `blacklist=qs,quickshell,foot,firefox,nautilus,loupe,mpv,gnome-text-editor,gnome-calculator,steamwebhelper`, + (`full`: `fps frametime frame_timing cpu_stats cpu_temp gpu_stats gpu_temp ram vram` uma por linha | senão `fps_only`); env do compositor `MANGOHUD_CONFIGFILE=<conf>` e `MANGOHUD=0|1`.

Scripts:
- `kyro-kernel status`: `uname -r` | default (`/boot/loader/entries/kyro.conf` existe → systemd-boot: `default kyro-zen.conf` em loader.conf; senão GRUB: `saved_entry=.*gnulinux-linux-zen-` em grubenv) | `pacman -Q linux-zen`.
- `kyro-kernel zen` (root): `pacman -S --needed --noconfirm linux-zen linux-zen-headers`; systemd-boot: gera `kyro-zen.conf` a partir de `kyro.conf` (sed título/vmlinuz/initramfs) + `default kyro-zen.conf`; GRUB: `GRUB_DEFAULT=saved`, `grub-mkconfig`, `grub-set-default "<submenu>><entry>"`. `default` volta para linux.
- `kyro-proton`: API `https://api.github.com/repos/GloriousEggroll/proton-ge-custom/releases/{latest|tags/T}`; destino `~/.local/share/Steam/compatibilitytools.d`; `list` = `find -name 'GE-Proton*' | sort -rV`; download para `~/.cache/kyro/proton`, progresso = tamanho do arquivo / `size` do asset a cada 1 s; confere `.sha512sum`; `tar -xzf`. `remove` valida `^GE-Proton[0-9A-Za-z._-]+$`. "Já instalada" = algum nome == tag ou começa com `tag-`.
- `kyro-session-select console|desktop`: recusa se só llvmpipe (`notify-send`); grava `~/.local/state/kyro/next-session`; `hyprctl dispatch 'hl.dsp.exit()'` (ou `steam -shutdown`/`pkill gamescope`).
- `kyro-drivers`: `lspci -nn` vendor 1002/8086/10de → pacotes mesa/vulkan-*/lib32-* (+nvidia, headers por kernel), habilita `[multilib]`, `sudo pacman -Syu --needed --noconfirm`, gera `~/.config/hypr/gpu.lua`.

---

## 8. Controles (`controllers`) — PageControllers.qml + bin/kyro-gamepad

Lista: `kyro-gamepad list` ao abrir e a cada 2 s → linhas `eventN|Nome|usb|bluetooth|outro|bateria(-1..100)|carregando(0/1)`.
- Implementação: blocos de `/proc/bus/input/devices` com handler `jsN`; `N: Name=`, `I: Bus=` (0003 usb, 0005 bluetooth), `U: Uniq=` (MAC), `eventN` em `H: Handlers=`; bateria = `/sys/class/power_supply/*` cujo caminho contém o MAC (lower) → `capacity`, `status==Charging`.

| # | Grupo | Tipo | Texto | Ação |
|---|---|---|---|---|
| 1 | `Controles conectados` vazio | info + botão `Bluetooth…` | `Nenhum controle conectado` / `Ligue pelo USB ou pareie pelo Bluetooth` | `Shell.openSettings("bluetooth")` |
| 2 | lista `stadia_controller` verde | | nome; subtitle `Bluetooth`/`USB`/`Conectado` + ` · bateria N%` + ` (carregando)`; botão `Testar` / `Parar Teste` (primário) | alterna `testing=eventN` |
| | footer | | `Xbox, PlayStation, Switch Pro e outros funcionam pelo USB ou Bluetooth. Na Steam, cada jogo também aceita ajustes em Configurações > Controle.` | |
| 3 | `Teste: <nome>` (se testando) | desenho do controle 380×200 raio 60 | footer `Aperte os botões e mexa os analógicos e gatilhos: o que responde acende aqui.` | `kyro-gamepad test eventN` |

Teste: `exec stdbuf -o0 od -An -v -w24 -tu2 /dev/input/eventN` → awk: palavra 9 = type, 10 = code, 11+12·65536 = value (s32); type 1 → `B code value`, type 3 → `A code value`. Encerrado com SIGTERM; desconectar o controle encerra o teste. (Leitura de `/dev/input/event*` exige o usuário no grupo `input`/uaccess.)
Mapeamento (códigos evdev): botões A=304 B=305 X=308 Y=307 LB=310 RB=311 Select=314 Start=315 Home=316 L3=317 R3=318 DPad 544–547 (cima/baixo/esq/dir); eixos LX=0 LY=1 RX=3 RY=4, LT=2|10, RT=5|9, HAT X=16 Y=17.
Normalização: HAT bruto (−1/0/1); gatilho `v/(max>255?1023:255)`; analógico: se max visto >255 ou v<0 → `v/32767` clamp, senão `(v−128)/128`. Stick: bolinha desloca ±16 px; acesa se |x|,|y|>0.15 ou clique.

---

## 9. Aparência (`appearance`) — PageAppearance.qml

| # | Grupo | Tipo | Texto | Lê/Escreve | Efeito colateral |
|---|---|---|---|---|---|
| 1 | — | dropdown | `Aparência` / `Automático: claro das 7h às 19h`; `Claro`=light `Escuro`=dark `Automático`=auto | S.appearance | `dark = mode=="dark" || (auto && (h<7 || h>=19))`; reavaliado por hora (SystemClock Hours); ao mudar `kyro-theme --appearance-only dark|light` |
| 2 | — | seletor de cores (8 bolinhas 20 px, espaçamento 9, linha 54 px; selecionada = borda branca 2 + ponto branco 6) | `Cor de destaque` | S.accent | `gsettings set org.gnome.desktop.interface accent-color <blue|purple|pink|red|orange|yellow|green|slate(graphite)>` |
| 3 | `Janelas` | dropdown | `Organização das janelas`: `Mosaico`=tiled, `Estilo Mac`=mac | S.windowMode | applyWindowMode |
| | footer | | `Estilo Mac abre os apps comuns em janelas flutuantes centralizadas, com foco por clique e maximização pelo app. Jogos mantêm suas regras. O modo de abertura vale para novas janelas; use ⌘ V para alternar uma janela já aberta. ⌘ + arrastar move; ⌘ + botão direito redimensiona.` | | |
| 4 | `Organização de janelas` | switch | `Organizar ao arrastar para as bordas` | S.edgeSnap | applyWindowMode |
| | footer | | `Arraste uma janela para uma borda para ocupar metade da tela; os cantos ocupam um quarto. Segurar Super e arrastar também funciona. Super + Alt + setas organiza por teclado; Super + Alt + Backspace restaura. Jogos e janelas em tela cheia preservam seu comportamento.` | | |
| 5 | `Efeitos visuais` | dropdown | `Perfil de efeitos`: `Visual`=visual `Equilibrado`=balanced `Jogo`=game | S.effectsProfile | Effects.apply (perfil efetivo = game se Modo Jogo) |
| 5b | | info (se Modo Jogo ativo) | `Modo Jogo ativo` / `Sua escolha será aplicada ao sair do Modo Jogo` | | |
| | footer | | `Visual mantém o vidro e as animações. Equilibrado reduz o desfoque. Jogo desliga desfoque, sombras e animações do compositor. O Modo Jogo usa esse perfil temporariamente e depois retoma sua escolha.` | | |
| 6 | `Barra de menus` | dropdown | `Fundo da barra`: `Transparente`=transparent `Vidro desfocado`=blur `Preto`=black | S.barStyle | barra: black #000; blur rgba(.08,.08,.09,.12) escuro / (.92,.92,.94,.12) claro; transparente |
| 7 | | info | `Texto da barra` → `Escuro (fundo claro)` / `Claro (fundo escuro)` | `barOnLight = barStyle!="black" && luminância(wallpaper) > 0.62` | luminância = 0.2126r+0.7152g+0.0722b da cor dominante (ColorQuantizer depth 0, 32 px) |
| | footer | | `Escolha um fundo transparente, vidro desfocado ou preto para a barra superior.` | | |

applyWindowMode (lua): `kyroLayout.enabled = edgeSnap`; `kyroDesktopRule:set_enabled(mac)`; `kyroMaximizeRule:set_enabled(!mac)`; `input.follow_mouse = mac?0:1`. Vale para novas janelas.
`kyro-theme --appearance-only <mode>`: `gsettings set org.gnome.desktop.interface color-scheme prefer-light|prefer-dark`, `gtk-theme adw-gtk3|adw-gtk3-dark`, `icon-theme WhiteSur-light|WhiteSur-dark`; atualiza `~/.config/gtk-{3,4}.0/settings.ini` `[Settings]` `gtk-theme-name`, `gtk-icon-theme-name`, `gtk-application-prefer-dark-theme=1|0` (backup único `settings.ini.kyro-before-appearance`). Root: não.

---

## 10. Dock (`dock`) — PageDock.qml

| # | Grupo | Tipo | Texto | Lê/Escreve |
|---|---|---|---|---|
| 1 | — | slider 220 | `Tamanho` | `S.dockSize` 36..80 px (`v=(size−36)/44`, grava `round(36+v·44)`) |
| 2 | — | dropdown (maxWidth 230) | `Mostrar Dock em` / `Escolha a tela onde o Dock aparece`; `Tela principal`=primary + `Tela N · <nome>` para screens[1..] | S.dockMonitor |
| 3 | — | switch | `Ampliação` / `Os ícones crescem quando o mouse passa por cima` | S.dockMagnify |
| 4 | — | switch | `Ocultar e mostrar o Dock automaticamente` / `Aparece ao encostar o mouse na borda de baixo` | S.dockAutohide |
| 5 | (grupo 2) | switch | `Ocultar quando uma janela cobrir o Dock` / `Reaparece ao encostar o mouse na borda inferior` | S.dockSmartHide |
| 6 | | switch | `Minimizar o app ativo ao clicar no Dock` / `Clique novamente para restaurar` | S.dockClickMinimize |
| 7 | (grupo 3) | switch | `Mostrar Downloads e Documentos` / `Clique para abrir os arquivos em grade` | S.dockFolders |
| 8 | `Apps fixados` | dropdown + botão primário `Adicionar` | `Adicionar app`; opções = apps (.desktop sem NoDisplay, ordenados por nome pt-BR) não fixados; default 1º | append id a S.pinned |
| 9 | | lista | nome do app (`Apps.byId` ou id cru) + botão `Remover` | remove de S.pinned |
| 10 | | info (vazio) | `Nenhum app fixado` / `Escolha um app acima para adicioná-lo ao Dock.` | |
Footer apps: `Os apps fixados ficam no Dock mesmo quando não estão abertos. Arraste os ícones no Dock para mudar a ordem.` IDs = id do .desktop sem `.desktop` (match case-insensitive + heurística). Root: não.

---

## 11. Telas (`display`) — PageDisplay.qml

Leitura: `hyprctl -j monitors` ao abrir e 700 ms após aplicar. Campos usados: `name, description, make, model, width, height, refreshRate, scale, x, y, availableModes[]`.
Formato salvo `S.monitors[]`: `"nome|LARGxALT@Hz|escala|x|y"`; aplicar = substitui entradas de mesmos nomes e para cada entrada: `hl.monitor({ output="<n>", mode="<WxH@Hz>", position="<x>x<y>"|"auto", scale=<s> })`.

| # | Grupo | Tipo | Texto | Ação |
|---|---|---|---|---|
| 1 | `Organizar telas` | info | `Arraste os monitores para organizar` / `Os números identificam cada monitor. Arraste para posicionar.` | |
| 2 | | canvas 210 px (arrastável) | retângulos proporcionais (tamanho lógico = px/scale), mín. 52×42, número 22 Bold + nome 9 px; selecionado = accent 20% + borda 2 | pressionar: seleciona, `Shell.identifyDisplays=true` (mostra número em cada tela; esconde 1,4 s depois de soltar); soltar (>4 px) → x,y += delta/escala do canvas (arredondado), pendente |
| 3 | | botões (se pendente) | `Alterações pendentes`: `Cancelar` (restaura) / `Confirmar alterações` (primário) | setMonitors de todos com novo x,y |
| 4 | | botões numéricos | `Tela selecionada` 1..N | seleciona |
| 5 | | info | description / name do selecionado | |
| | footer | | <2 telas: `Conecte outra tela para organizar a posição.`; senão `Arraste os retângulos e clique em Confirmar alterações para aplicar. Cancelar restaura a organização anterior.` | |
| 6 | (um grupo por monitor, título = description||name; desabilitado se pendente/carregando) | info `desktop_windows` | name / `make model` | |
| 7 | | dropdown | `Resolução`: modos únicos `WxH@round(Hz)` de availableModes, rótulo `W × H  ·  Hz Hz` | Config.setMonitor(name, mode, scaleAtual, x, y) |
| 8 | | dropdown | `Escala` / `Aumenta textos e janelas em telas de alta densidade`: 100/125/150/175/200% | setMonitor(name, modoAtual, s, x, y) |
| 9 | `Luz noturna` | switch `nightlight` laranja | `Luz noturna` | S.nightShift |
| 10 | | slider 180 entre textos `Mais quente` / `Menos quente` | `Temperatura` | `S.nightTemp` 2500..6000 K, passo 100 (`v=(t−2500)/3500`) |
| | footer | | `Deixa as cores da tela mais quentes para cansar menos a vista à noite.` | |

Luz noturna/brilho por software: processo `hyprsunset -t <nightShift?nightTemp:6500> -g <round(clamp(softBrightness,0.2,1)·100)>` rodando enquanto `nightShift || softBrightness<0.999`; mudança de temp/brilho/nightShift reinicia com debounce 300 ms. (`softBrightness` é controlado pela Central de Controle, não pela página.) Root: não.

---

## 12. Papel de Parede (`wallpaper`) — PageWallpaper.qml

- Atual = `S.wallpaper || ~/.local/share/kyro/wallpaper.jpg` (padrão).
- Lista (ao abrir): `for d in <padrão> ~/.local/share/kyro/wallpapers "~/Imagens/Papéis de Parede" /usr/share/backgrounds; do [ -d "$d" ] && find "$d" -maxdepth 2 -type f \( -iname '*.jpg' -o -iname '*.jpeg' -o -iname '*.png' -o -iname '*.webp' \); done` → padrão primeiro, sem duplicar.

| # | Tipo | Texto |
|---|---|---|
| 1 | preview grande 200 px raio 12 (crop) | imagem atual |
| 2 | título | `Escolha um papel de parede` (Bold) |
| 3 | grade 4 colunas, altura = 0,62·largura, raio 9, atual = borda accent 3 | clique → `S.wallpaper = caminho` (`""` se for o padrão) |
| 4 | nota 11 px | `Para adicionar imagens, coloque-as em ~/Imagens/Papéis de Parede.` |
Sem atualização ao vivo da pasta. Root: não.

---

## 13. Bateria (`battery`) — PageBattery.qml + BatteryInfo.qml (UPower) + PowerProfile.qml

UPower D‑Bus: `/org/freedesktop/UPower/devices/DisplayDevice` → `Percentage`, `State` (1 Charging, 2 Discharging, 4 FullyCharged, 5 PendingCharge), `TimeToEmpty`, `TimeToFull`, `Capacity` (saúde), `EnergyFull/EnergyFullDesign`, `Type==Battery`/`IsPresent` (isLaptopBattery); `org.freedesktop.UPower.OnBattery`.
- `stateText`: `Carregada` (cheia e na tomada; ≥99,5%) / `Carregando` / `Conectada à energia, sem carregar` / `Usando a bateria`.
- `timeText`: `H:MM até carregar` / `H:MM restantes`.
- Histórico: 1 amostra/min, últimos 60 (em memória desde o início do shell).
- health = Capacity/100 se suportado; senão 1 se EnergyCapacity>0; senão 0.

PowerProfile (power-profiles-daemon): leitura `sh -c "powerprofilesctl get && powerprofilesctl list | grep -oE '^[ *] +[a-z-]+:' | tr -d ' *:'"` → linha1 = atual, demais = disponíveis. Escrita `powerprofilesctl set <p>`. Nomes: `power-saver`=`Economia`, `balanced`=`Equilibrado`, `performance`=`Desempenho`. `supported = available.length>0`.

| # | Grupo | Tipo | Texto | Lê/Escreve |
|---|---|---|---|---|
| 1 | cartão 150 px | info grande | `NN%` (40 Bold tnum), stateText (DemiBold), timeText ou `Adaptador de energia`/`Bateria`; `Última hora` | |
| 2 | | gráfico 60 barras, altura 52 | cor: <20% red; senão verde (carregando) / amarelo (pouca energia) / verde | history |
| 3 | — | switch `energy_savings_leaf` amarelo (se supported) | `Modo de Pouca Energia` | `current=="power-saver"` → `set(on?"power-saver":"balanced")` |
| 4 | | switch | `Mostrar porcentagem na barra de menus` | S.batteryPercent |
| 5 | | dropdown (se supported) | `Modo de energia` | PowerProfile.current / set |
| | footer | | `O Modo de Pouca Energia reduz o consumo (perfil de energia “Economia”) e deixa o ícone da bateria amarelo, como no Mac.` | |
| 6 | `Saúde da Bateria` | info `health_metrics` (verde ≥80% senão laranja) | `Normal` / `Manutenção recomendada` / `Indisponível`; `Capacidade máxima: N%` | |
| | footer | | `Capacidade máxima é quanto a bateria ainda guarda em comparação com quando era nova.` | |
Avisos (serviço): ≤20% `notify-send -a Bateria -i battery-low "Bateria fraca" "N% restantes (...). Ative o Modo de Pouca Energia nos Ajustes."`; ≤10% `-u critical -i battery-caution "Bateria muito fraca" "N% restantes. Conecte o carregador."` (uma vez por descida). Root: não.

---

## 14. Bloqueio e Energia (`lock`) — PageLock.qml

Opções de tempo (todos os 3 dropdowns): `1 minuto`=60, `2 minutos`=120, `5 minutos`=300, `10 minutos`=600, `15 minutos`=900, `30 minutos`=1800, `1 hora`=3600, `Nunca`=0.

| # | Grupo | Tipo | Texto | Lê/Escreve | Consumidor |
|---|---|---|---|---|---|
| 1 | `Tela bloqueada` | switch | `Pedir senha ao ligar` / `O sistema abre na tela de bloqueio` | S.loginLock | `bin/kyro-session-start`: se `jq -r .loginLock` == `false` não chama lock |
| 2 | | dropdown | `Bloquear a tela quando inativo por` | S.lockAfter | IdleMonitor (ext-idle-notify, respeita inhibitors) → `Shell.lock()` |
| 3 | | dropdown | `Desligar a tela quando inativo por` | S.screenOffAfter | idle → `hyprctl dispatch 'hl.dsp.dpms("off")'`; ao voltar `"on"` |
| 4 | | dropdown | `Repousar quando inativo por` | S.suspendAfter | idle → `systemctl suspend` |
| 5 | | botão | `Bloquear agora` → `Bloquear Tela` | | `Shell.lock()` |
| 6 | `Energia` (se ppd) | dropdown `bolt` verde | `Modo de energia` | PowerProfile | `powerprofilesctl set` ; footer `O Modo Jogo troca para “Desempenho” automaticamente.` |
| 7 | grupo vazio | footer | `Jogos e vídeos em tela cheia impedem o bloqueio automático.` | | |
Root: não (`systemctl suspend` via logind/polkit).

---

## 15. Teclado (`keyboard`) — PageKeyboard.qml

| # | Grupo | Tipo | Texto | Lê/Escreve |
|---|---|---|---|---|
| 1 | — | dropdown | `Layout do teclado`: `Português (Brasil, ABNT2)`=`br|`, `Português (Portugal)`=`pt|`, `Inglês (EUA)`=`us|`, `Inglês (EUA, internacional)`=`us|intl`, `Espanhol`=`es|` | `S.kbLayout + "|" + S.kbVariant` (grava variant depois layout) |
| 2 | `Área de Transferência` | switch | `Guardar histórico da área de transferência` | S.clipboardHistory → liga/desliga processo `wl-paste --watch cliphist -max-items 200 store` (via watch_process.py clipboard) |
| 3 | | botão | `Apagar o histórico` → `Limpar Histórico` | `sh -c 'cliphist wipe; rm -rf "<cache de imagens>"'` |
| | footer | | `O histórico fica só neste computador (até 200 itens). Abra com ⇧⌘ V ou no Spotlight com ⌃3.` | |
| 4 | `Atalhos` (info rows, valor em secondaryLabel) | | Spotlight `⌘ Espaço`; Launchpad `⌘ A`; Ajustes do Sistema `⌘ ,`; Terminal `⌘ ↩`; Arquivos `⌘ E`; Fechar janela `⌘ Q`; Tela cheia `⌘ F`; Minimizar `⌘ M`; Alternar entre apps `⌘ Tab`; Mission Control `⌘ ↑`; Forçar encerrar apps `⌥⌘ Esc`; Histórico da área de transferência `⇧⌘ V`; Bloquear tela `⌃⌘ Q`; Captura da tela inteira `⇧⌘ 3`; Captura de uma área `⇧⌘ 4`; Opções de captura `⇧⌘ 5 ou Print`; Ir para a mesa N `⌘ 1…9`; Mover janela para a mesa N `⌃⌘ 1…9` | estático |
| | footer | | `⌘ é a tecla Super (a tecla do Windows).` | |

applyInput (lua, a cada mudança de kbLayout/kbVariant/mouse*): `hl.config({ input = { kb_layout="<l>", kb_variant="<v>", sensitivity=<s>, accel_profile="adaptive"|"flat", natural_scroll=<b>, touchpad={ natural_scroll=<b> } } })`. Root: não.

---

## 16. Mouse e Trackpad (`mouse`) — PageMouse.qml

| # | Tipo | Texto | Lê/Escreve |
|---|---|---|---|
| 1 | slider 220, step 0,05 | `Velocidade do cursor` | `S.mouseSensitivity` −1..1 (`v=(s+1)/2`, grava `round((2v−1)·100)/100`) |
| 2 | switch | `Aceleração do mouse` / `Desligada é melhor para mirar em jogos` | S.mouseAccel (`adaptive`/`flat`) |
| 3 | switch | `Rolagem natural` / `O conteúdo acompanha o movimento dos dedos` | S.naturalScroll (mouse e touchpad) |
Aplica via applyInput. Root: não.

---

## 17. Usuários (`users`, via cartão) — PageUsers.qml + User.qml

Leitura (User.refresh): `sh -c 'getent passwd "$USER" | cut -d: -f5 | cut -d, -f1; id -nG | tr " " "\n" | grep -qx wheel && echo admin || echo user; test -s "$HOME/.face" && echo face || echo noface'` → linha1 fullName, linha2 admin, linha3 hasFace. Foto = `~/.face` (URL com `?v=<revisão>` para recarregar).

| # | Grupo | Tipo | Texto | Ação |
|---|---|---|---|---|
| 1 | cabeçalho | Avatar 96 + nome 18 Bold + `<user> · Administrador`/`Usuário padrão` | | |
| 2 | `Foto` | grade de 8 presets (círculos 48 px, degradê a→b + símbolo branco 24) | person #6c7dff→#3b46c4; sports_esports #30d158→#1a8f3a; rocket_launch #ff9230→#d9480f; pets #ff375f→#b3123a; music_note #bf5af2→#7a2bb0; sunny #ffd60a→#e09b00; nightlight #64d2ff→#0a6fb8; local_florist #ff6482→#c2185b | renderiza 256×256 (degradê + símbolo 128) → grava PNG 512×512 em `~/.face` → refresh |
| 3 | | botões | `Usar uma imagem` / `PNG, JPG ou WebP`: `Remover Foto` (se hasFace) → `rm -f ~/.face`; `Escolher Imagem…` (primário) → FileDialog `Escolher foto`, filtro `Imagens (*.png *.jpg *.jpeg *.webp *.bmp)`, pasta `~/Imagens` → crop central quadrado → PNG 512 em `~/.face` | |
| | footer | | `Aparece na tela de bloqueio e nos Ajustes. Imagens são recortadas no centro, em quadrado.` | |
| 4 | `Conta` | text field 200 + botão `Salvar` (se mudou) | `Nome completo` / `Pede a senha de administrador`; placeholder = login | `pkexec usermod -c "<nome sem : e ,>" <user>` → refresh |
| 5 | | botão | `Senha` / `A mesma do login, do sudo e da tela de bloqueio` → `Alterar Senha…` | `foot --app-id=kyro-update -T "Alterar Senha" sh -c "passwd; echo; read -p 'Enter para fechar.' x"` |
| 6 | | botão | `Tela de bloqueio` / `Pedir a senha ao ligar, bloquear depois de um tempo` → `Abrir` | `Shell.openSettings("lock")` |
Root: pkexec só para nome. Sem lista de outros usuários (apenas o atual).

---

## 18. Data, Hora e Região (`datetime`) — PageDateTime.qml

Leitura ao abrir:
```
sh -c "timedatectl show -p Timezone -p NTP; localectl status | sed -n 's/.*LANG=//p'"   # Timezone=..., NTP=yes|no, resto = LANG
timedatectl list-timezones           # filtra linhas com "/"
kyro-locale list                     # "código|nome"
```
Rótulo de fuso: `Região/…/Cidade` → `Cidade (Região-pt)`, `_`→espaço; regiões: America→América, Europe→Europa, Asia→Ásia, Africa→África, Australia→Austrália, Pacific→Pacífico, Atlantic→Atlântico, Indian→Índico, Antarctica→Antártida, Arctic→Ártico, Etc→Outros; acentua Sao Paulo, Maceio, Belem, Cuiaba, Araguaina.
Busca de fuso: ≥2 chars; compara `(zona + " " + rótulo)` lower + NFD sem diacríticos; máx. 8 resultados.

| # | Grupo | Tipo | Texto | Lê | Escreve |
|---|---|---|---|---|---|
| 1 | relógio 76 px | info | `HH:mm:ss` (ou `h:mm:ss AP`) 26 DemiBold tnum + data `dddd, d 'de' MMMM 'de' yyyy` pt_BR capitalizada | SystemClock (s) | |
| 2 | `Data e hora` | switch `schedule` azul | `Ajustar automaticamente` / `Pela internet (servidores de hora)` | NTP | `timedatectl set-ntp true|false` |
| 3 | | text field 200 `public` azul | `Fuso horário`; subtitle `<rótulo> · <Zona>` / `…`; placeholder `Buscar cidade…` | Timezone | — |
| 4 | | lista resultados | rótulo / zona; botão `Usar` (primário) / `Atual` (desab.) | | `timedatectl set-timezone <Zona>` |
| | footer | | `Mudar o fuso ou a hora automática pede a senha de administrador.` | | |
| 5 | `Relógio` | switch | `Relógio de 24 horas`, subtitle `14:30` / `2:30 PM` | S.clock24 | formato global `HH:mm` / `h:mm AP` |
| 6 | | switch | `Mostrar a data na barra de menus` / `Dia da semana e data ao lado da hora` | S.clockDate | |
| 7 | `Idioma e região` | dropdown `translate` índigo | `Idioma do sistema` / `Pede a senha de administrador` (lista + LANG atual se fora dela) | LANG | `foot --app-id=kyro-update -T Idioma sh -c "sudo kyro-locale set <código>; echo; read -p 'Enter para fechar.' x"` → relê |
| 8 | | botão `keyboard` | `Teclado` / `Layout e atalhos` → `Abrir` | | `Shell.openSettings("keyboard")` |
| | footer | | `O idioma dos apps (Firefox, Arquivos, Steam…) e dos formatos de data e número. Vale depois de encerrar a sessão e entrar de novo. A interface do kyro (barra, Ajustes) continua em português.` | | |
Após qualquer `timedatectl` → relê status. Elevação: polkit (timedated) com o agente do kyro; idioma via sudo no terminal.
`kyro-locale`: idiomas fixos `pt_BR, pt_PT, en_US, en_GB, es_ES, es_MX, fr_FR, de_DE, it_IT, ja_JP` (`.UTF-8`); `set`: descomenta/acrescenta em `/etc/locale.gen`, `locale-gen`, `localectl set-locale LANG=<código>`.

---

## 19. Apps Padrão (`defaults`) — PageDefaults.qml + bin/kyro-defaults

Categorias (ordem): web `Navegador` `language` #0091ff; files `Pastas` `folder` #0091ff; text `Texto` `description` #8e8e93; pdf `PDF` `picture_as_pdf` #ff453a; images `Imagens` `image` #30d158; video `Vídeos` `movie` #bf5af2; music `Música` `music_note` #ff375f; archive `Arquivos compactados` `folder_zip` #ff9230.

MIME por categoria (o 1º é o "principal"):
| cat | mimes |
|---|---|
| web | x-scheme-handler/https x-scheme-handler/http text/html application/xhtml+xml |
| files | inode/directory |
| text | text/plain text/markdown application/x-shellscript |
| pdf | application/pdf |
| images | image/png image/jpeg image/gif image/webp image/svg+xml image/bmp image/avif |
| video | video/mp4 video/x-matroska video/webm video/quicktime video/x-msvideo |
| music | audio/mpeg audio/flac audio/ogg audio/x-wav audio/mp4 audio/x-opus+ogg |
| archive | application/zip application/x-tar application/x-7z-compressed application/vnd.rar application/x-xz-compressed-tar application/gzip |

Leitura (ao abrir e após set): `kyro-defaults all` → por categoria `## <cat> <id.desktop atual>` seguido de `id.desktop|Nome`.
- Implementação: indexa `.desktop` de `~/.local/share/applications`, `~/.local/share/flatpak/exports/share/applications`, `/var/lib/flatpak/exports/share/applications`, `$XDG_DATA_DIRS/applications` (primeiro id vence) → `Name[pt_BR]` ou `Name`, `NoDisplay`; para cada cat `LANG=C gio mime <principal>` → `Default application for ...: X` e linhas tab entre `Registered applications:` e `Recommended applications:` (sort -u, só visíveis).
- Página: se padrão atual não está na lista, entra no topo com rótulo = id sem `.desktop`.

| Controle | Texto | Escrita |
|---|---|---|
| 8 linhas, dropdown cada (ícone colorido) | label da categoria; subtitle `Nenhum app instalado abre este tipo` se lista vazia | `kyro-defaults set <cat> <id.desktop>` = `xdg-mime default <id> <todos mimes>` (+ `xdg-settings set default-web-browser <id>` para web); id validado `^[A-Za-z0-9._-]+\.desktop$` |
Footer: `Vale para abrir arquivos no Arquivos, links e anexos. Apps instalados pela App Store aparecem aqui também.` Grava `~/.config/mimeapps.list`. Root: não.

---

## 20. Atualizações (`updates`) — PageUpdates.qml

Leitura ao abrir / `Verificar`: `sh -c "checkupdates 2>/dev/null; true"` (pacman-contrib) → linhas `nome velha -> nova` (split espaço: p0 nome, p1 de, p3 para). `checkedAt = HH:mm`.

| # | Tipo | Texto | Ação |
|---|---|---|---|
| 1 | info 60 px `system_update` (accent) / `verified` (verde) | `Procurando atualizações…` / `N atualizações disponíveis` / `O sistema está atualizado`; subtitle `Verificado às HH:mm`; botões `Verificar` (se não checando), `Atualizar Agora` (primário, se >0) | `foot --app-id=kyro-update -T "Atualização do sistema" sh -c "sudo pacman -Syu; echo; read -p 'Pronto. Enter para fechar.' x"` (não re-verifica sozinho) |
| 2 | grupo `Pacotes`: lista (máx. 40) | nome + `de  →  para` (12 px) | |
Root: sudo no terminal. (Flatpak não incluído aqui.)

---

## 21. Armazenamento (`storage`) — PageStorage.qml

Medição (ao abrir e após cada limpeza):
```
df -B1 --output=size,used / | awk 'NR==2 {print "total=" $1; print "used=" $2}'
sum() { du -sbc "$@" 2>/dev/null | tail -1 | cut -f1; }
echo games=$(sum ~/.local/share/Steam/steamapps ~/Games)
echo flatpak=$(sum ~/.local/share/flatpak /var/lib/flatpak)
echo cache=$(sum ~/.cache)
echo trash=$(sum ~/.local/share/Trash)
echo pkg=$(sum /var/cache/pacman/pkg)
echo journal=$(journalctl --disk-usage | grep -oE '[0-9.,]+[KMGT]?B?' | head -1)
```
Formato: base 1000, unidades `bytes KB MB GB TB`, 1 casa decimal se <100 (vírgula decimal), `Zero KB` se 0.

| # | Grupo | Tipo | Texto | Ação |
|---|---|---|---|---|
| 1 | — | barra empilhada 18 px raio 5 + legenda | `Disco do sistema`; `<livre> livres de <total>` / `Calculando…`; segmentos `Jogos` (green, games), `Apps (Flatpak)` (blue), `Caches e Lixo` (orange, cache+trash+pkg), `Sistema e Outros` (#8e8e93, used − soma) | |
| 2 | `Recomendações` | CleanRow `delete` | `Esvaziar o Lixo`, botão `Esvaziar` | `gio trash --empty` |
| 3 | | CleanRow `cached` laranja | `Cache dos apps` / `~/.cache (miniaturas, navegador, downloads de ferramentas)` | `sh -c 'find "$HOME/.cache" -mindepth 1 -maxdepth 1 ! -name quickshell ! -name fontconfig ! -name qtshadercache\* -exec rm -rf {} +'` |
| 4 | | CleanRow `inventory_2` azul | `Pacotes baixados` / `Mantém só a versão instalada de cada pacote (pede a senha)` | `pkexec sh -c "paccache -rk1; paccache -ruk0"` |
| 5 | | CleanRow `apps` azul, tamanho `—` | `Componentes de apps não usados` / `Runtimes do Flatpak que nenhum app usa mais` | `flatpak uninstall --unused -y --noninteractive` |
| 6 | | CleanRow `receipt_long`, tamanho = texto do journal | `Registros do sistema` / `Guarda só os últimos 50 MB (pede a senha)` | `pkexec journalctl --vacuum-size=50M` |
| | footer | | `Tudo aqui se refaz sozinho quando precisar (caches e pacotes baixados de novo), exceto o Lixo. Clique duas vezes para confirmar.` | |
| 7 | — | botão `Abrir` | `Ver os maiores arquivos` / `Abre a pasta pessoal no Arquivos` | `nautilus --new-window $HOME` |
CleanRow: tamanho (12 px) + botão; 1º clique → `Confirmar` (primário) por 4 s; 2º executa; durante → `Limpando…`; uma limpeza por vez (botões desabilitados, opacidade .5). Botão padrão `Limpar`.

---

## 22. Instantâneos (`snapshots`) — PageSnapshots.qml (snapper/btrfs)

Leitura (ao abrir e após ação): `snapper --jsonout -c root list --disable-used-space` → JSON `.root[]` sem `number==0`, invertido (mais novo primeiro). Falha de parse → indisponível. Campos: `number, date, type (single|pre|post), cleanup, description`.
Rótulo: cleanup `timeline` ou desc `timeline` → `Automático (por horário)`; desc começa com `pacman` ou type pre/post → `Antes de atualizar o sistema` (pre) / `Depois de atualizar o sistema`; senão description ou `Manual`.

| # | Grupo | Tipo | Texto | Ação |
|---|---|---|---|---|
| 0 | indisponível | info `history` cinza | `Instantâneos indisponíveis`; footer `Os instantâneos precisam do sistema de arquivos btrfs (o instalador do kyro já usa). Neste computador o snapper não está configurado.` | |
| 1 | — | info + botão primário | `N instantâneo(s)` / `O mais recente: <date>` ou `Nenhum ainda`; `Criar Agora` / `Criando…` | `snapper -c root create --cleanup-algorithm number -d Manual` |
| | footer | | `O kyro tira um instantâneo antes e depois de cada atualização do sistema e outros de hora em hora (os mais antigos são apagados sozinhos). Para recuperar um arquivo, abra o instantâneo e copie o arquivo de volta. Para voltar o sistema inteiro (ex.: uma atualização deu problema), use Restaurar: a pasta pessoal não muda e o sistema de antes fica guardado.` | |
| 2 | `Histórico` (máx. 30) | lista | label; subtitle `#N  ·  <date>`; botões `Ver Arquivos`, `Apagar`, `Restaurar…`→`Confirmar` (2 cliques, 4 s) | `nautilus --new-window /.snapshots/N/snapshot`; `snapper -c root delete N`; terminal: `foot --app-id=kyro-update -T "Restaurar o sistema" sh -c "echo ...; sudo kyro-rollback N && { read -p 'Reiniciar agora? [s/N] ' r; [ \"$r\" = s ] && systemctl reboot; } || read -p 'Enter para fechar.' x"` |

`kyro-rollback N` (root): exige `/` btrfs em `subvol=/@`; monta subvolid=5 em `/run/kyro-rollback`; `btrfs subvolume snapshot @snapshots/N/snapshot @restaurando`; se `/boot` vfat copia `/usr/lib/modules/<v>` ausentes; `mv @ @antes-<AAAA-MM-DD_HH.MM.SS>`; `mv @restaurando @`. Snapper sem root (ALLOW_USERS).

---

## 23. Sobre (`about`) — PageAbout.qml

Leitura (uma vez):
```
cpu    = grep -m1 'model name' /proc/cpuinfo | cut -d: -f2 (trim) + " (<nproc> núcleos)"
mem    = awk '/MemTotal/ {printf "%.0f GB", $2/1048576}' /proc/meminfo
gpu    = lspci | grep -iE 'vga|3d|display' | head -1 | cut -d: -f3 (trim)
disk   = df -h / | awk 'NR==2 {print $4 " livres de " $2}'
kernel = uname -r
host   = cat /sys/devices/virtual/dmi/id/product_name
hypr   = Hyprland --version | head -1 | awk '{print $2}'
qs     = qs --version | awk '{print $2}'
uptime = uptime -p | sed 's/up //'
```
| # | Tipo | Texto |
|---|---|---|
| 1 | logo | quadrado 110 raio 26 #0b0f1f com KyroLogo 76 (cor #eef1ff, accent #6c7dff); `kyro` 28 DemiBold; `Versão 1.0 · Lua Nova` |
| 2 | info rows | `Computador` host; `Processador` cpu (340 px, alinhado à direita); `Gráficos` gpu; `Memória` mem; `Disco` disk |
| 3 | grupo `Software` | `Kernel`; `Hyprland`; `Quickshell`; `Ligado há` |
Vazio → `—`. Na reimplementação: trocar Hyprland/Quickshell pela versão do compositor/shell próprio.

---

## 24. Resumo de listas dinâmicas e atualização
| Lista | Fonte | Refresh |
|---|---|---|
| Redes Wi‑Fi / conexões salvas | nmcli (refresh composto) | `nmcli monitor` (debounce 400 ms) + rescan ao abrir/botão |
| Clientes do hotspot | `iw station dump` + `/run/kyro/hotspot-leases` | 4 s enquanto ativo |
| Dispositivos BT | BlueZ ObjectManager | sinais D‑Bus |
| Impressoras/fila | `lpstat -p -d`, `lpstat -o` | 3 s com página aberta |
| Saídas/entradas de áudio | PipeWire registry | ao vivo |
| Controles | `/proc/bus/input/devices` + power_supply | 2 s com página aberta |
| Proton-GE instaladas / última | `kyro-proton list/latest` | ao abrir, após instalar/remover |
| Monitores | `hyprctl -j monitors` | ao abrir, 700 ms após aplicar |
| Papéis de parede | find em 4 pastas | ao abrir |
| Apps (Dock) | DesktopEntries (.desktop) | ao vivo |
| Fusos | `timedatectl list-timezones` | ao abrir |
| Idiomas | `kyro-locale list` (fixo) | ao abrir |
| Apps padrão | `kyro-defaults all` | ao abrir, após set |
| Atualizações | `checkupdates` | ao abrir, botão |
| Instantâneos | `snapper --jsonout list` | ao abrir, após ação |
| Notificações (contagem) | memória do servidor | ao vivo |
| Histórico bateria | UPower, 1/min | contínuo |
