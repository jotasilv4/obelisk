//! Páginas dos Ajustes (as mesmas do kyro). Cada uma lê o sistema com ferramentas padrão e
//! devolve grupos de linhas; roda numa thread. Ações com root passam pelo agente polkit.

use super::settings_model::*;

const BLUE: (u8, u8, u8) = (0x30, 0x7c, 0xf6);
const GRAY: (u8, u8, u8) = (0x8e, 0x8e, 0x93);
const DARK: (u8, u8, u8) = (0x3a, 0x3a, 0x3c);
const GREEN: (u8, u8, u8) = (0x30, 0xd1, 0x58);
const PURPLE: (u8, u8, u8) = (0xa5, 0x5e, 0xea);
const ORANGE: (u8, u8, u8) = (0xff, 0x92, 0x30);
const RED: (u8, u8, u8) = (0xff, 0x42, 0x45);
const PINK: (u8, u8, u8) = (0xff, 0x37, 0x8f);
const TEAL: (u8, u8, u8) = (0x00, 0xb8, 0xd4);

/// (id, rótulo, ícone, cor, categoria, palavras para a busca)
pub const PAGES: [(&str, &str, &str, (u8, u8, u8), &str, &str); 26] = [
    ("wifi", "Wi‑Fi", "wifi", BLUE, "Conexões", "rede internet ip dns senha"),
    ("hotspot", "Acesso Pessoal", "wifi_tethering", GREEN, "Conexões", "hotspot roteador compartilhar"),
    ("bluetooth", "Bluetooth", "bluetooth", BLUE, "Conexões", "fone controle dispositivos parear"),
    ("notifications", "Notificações", "notifications", RED, "Notificações e Som", "banners foco nao perturbe"),
    ("sound", "Som", "volume_up", PINK, "Notificações e Som", "volume saida entrada microfone alto falante"),
    ("game", "Modo Jogo", "sports_esports", GREEN, "Jogos", "jogos desempenho gamemode mangohud fps"),
    ("controllers", "Controles", "stadia_controller", PURPLE, "Jogos", "gamepad joystick xbox playstation"),
    ("proton", "Proton-GE", "deployed_code", ORANGE, "Jogos", "steam wine compatibilidade"),
    ("kernel", "Kernel", "memory", DARK, "Jogos", "zen linux boot"),
    ("appearance", "Aparência", "contrast", DARK, "Personalização", "tema claro escuro cor destaque"),
    ("wallpaper", "Papel de Parede", "image", TEAL, "Personalização", "fundo imagem"),
    ("dock", "Mesa e Dock", "dock_to_bottom", BLUE, "Personalização", "dock ampliacao canto stage widgets"),
    ("displays", "Telas", "desktop_windows", BLUE, "Personalização", "monitor escala resolucao night shift"),
    ("battery", "Bateria", "battery_full", GREEN, "Energia e Segurança", "energia economia desempenho"),
    ("lock", "Bloqueio e Energia", "lock", DARK, "Energia e Segurança", "senha bloquear suspender desligar"),
    ("snapshots", "Instantâneos", "history", PURPLE, "Energia e Segurança", "backup restaurar snapper btrfs time machine"),
    ("updates", "Atualizações", "update", GRAY, "Energia e Segurança", "pacman flatpak atualizar"),
    ("keyboard", "Teclado", "keyboard", GRAY, "Teclado e Mouse", "layout atalhos repeticao abnt cmd"),
    ("mouse", "Mouse e Trackpad", "mouse", GRAY, "Teclado e Mouse", "rolagem natural velocidade aceleracao"),
    ("general", "Geral", "settings", GRAY, "Sistema", "animacoes encaixe janelas"),
    ("datetime", "Data e Hora", "schedule", BLUE, "Sistema", "fuso horario relogio 24h ntp"),
    ("language", "Idioma e Região", "language", BLUE, "Sistema", "idioma locale"),
    ("defaults", "Apps Padrão", "apps", GRAY, "Sistema", "navegador abrir com pdf"),
    ("printers", "Impressoras", "print", GRAY, "Sistema", "imprimir cups"),
    ("storage", "Armazenamento", "hard_drive", GRAY, "Sistema", "disco espaco limpar cache"),
    ("about", "Sobre", "info", GRAY, "Sistema", "versao processador memoria"),
];

pub const SECTIONS: [(&str, &[&str]); 7] = [
    ("Conexões", &["wifi", "hotspot", "bluetooth"]),
    ("Notificações e Som", &["notifications", "sound"]),
    ("Jogos", &["game", "controllers", "proton", "kernel"]),
    ("Personalização", &["appearance", "wallpaper", "dock", "displays"]),
    ("Energia e Segurança", &["battery", "lock", "snapshots", "updates"]),
    ("Teclado e Mouse", &["keyboard", "mouse"]),
    ("Sistema", &["general", "datetime", "language", "defaults", "printers", "storage", "about"]),
];

/// Páginas que mudam sozinhas (relidas a cada 4 s enquanto abertas).
pub fn live(page: &str) -> bool {
    matches!(page, "wifi" | "bluetooth" | "sound" | "battery" | "controllers")
}

fn set(k: &str, v: &str) -> Act {
    Act::Set(k.into(), v.into())
}

fn toggle(ctx: &Ctx, k: &str) -> Act {
    set(k, if ctx.on(k) { "false" } else { "true" })
}

fn lines(cmd: &str) -> Vec<String> {
    sh(cmd).lines().map(|s| s.to_string()).filter(|s| !s.is_empty()).collect()
}

/// Divide linha `nmcli -t` respeitando `\:`.
fn nm_fields(l: &str) -> Vec<String> {
    let mut out = vec![String::new()];
    let mut esc = false;
    for c in l.chars() {
        if esc {
            out.last_mut().unwrap().push(c);
            esc = false;
        } else if c == '\\' {
            esc = true;
        } else if c == ':' {
            out.push(String::new());
        } else {
            out.last_mut().unwrap().push(c);
        }
    }
    out
}

pub fn build(page: &str, ctx: &Ctx) -> Vec<Group> {
    if page.starts_with("s:") {
        return super::store_pages::build(page, ctx);
    }
    match page {
        "wifi" => wifi(ctx),
        "hotspot" => hotspot(),
        "bluetooth" => bluetooth(ctx),
        "notifications" => vec![group("Notificações", vec![
            sw("notifications", RED, "Banners no notch", "Desligado: só na Central de Notificações", ctx.on("notif_banners"), toggle(ctx, "notif_banners")),
            sw("bedtime", PURPLE, "Foco (Não Perturbe)", "Silencia banners, menos os críticos", ctx.on("dnd"), toggle(ctx, "dnd")),
        ])],
        "sound" => sound(),
        "game" => game(ctx),
        "controllers" => controllers(),
        "proton" => proton(),
        "kernel" => kernel(),
        "appearance" => vec![group("Mesa", vec![sw("widgets", ORANGE, "Widgets na mesa", "Relógio, calendário e sistema", ctx.on("widgets"), toggle(ctx, "widgets"))])],
        "wallpaper" => vec![group("", vec![note("Coloque imagens em ~/Imagens/Papéis de Parede para que apareçam aqui.")])],
        "dock" => vec![
            group("Dock", vec![
                sw("zoom_in", BLUE, "Ampliação", "Ícones crescem sob o cursor", ctx.on("dock_magnify"), toggle(ctx, "dock_magnify")),
                sw("dock_to_bottom", BLUE, "Ocultar e mostrar o Dock automaticamente", "Aparece ao encostar o mouse na borda de baixo", ctx.on("dock_autohide"), toggle(ctx, "dock_autohide")),
                sw("fullscreen", GRAY, "Dock inteligente", "Some quando uma janela ocupa o espaço dele (maximizada, tela cheia)", ctx.get("dock_smart_hide") != "false", set("dock_smart_hide", if ctx.get("dock_smart_hide") != "false" { "false" } else { "true" })),
            ]),
            group("Mesa", vec![
                sw("space_dashboard", PURPLE, "Canto ativo", "Canto superior esquerdo abre o Mission Control", ctx.on("hot_corners"), toggle(ctx, "hot_corners")),
                sw("view_carousel", BLUE, "Stage Manager", "App em foco no centro, os outros em miniaturas", ctx.on("stage_manager"), toggle(ctx, "stage_manager")),
                sw("dashboard", PURPLE, "Encaixar nas bordas", "Arraste até a borda: metade; canto: um quarto", ctx.on("snapping"), toggle(ctx, "snapping")),
            ]),
        ],
        "displays" => displays(ctx),
        "battery" => battery(),
        "lock" => lock(ctx),
        "snapshots" => snapshots(),
        "updates" => updates(),
        "keyboard" => keyboard(ctx),
        "mouse" => mouse(ctx),
        "general" => vec![group("Janelas", vec![
            sw("open_in_full", BLUE, "Animações de janela", "Abrir e minimizar com movimento", ctx.on("animations"), toggle(ctx, "animations")),
        ])],
        "datetime" => datetime(ctx),
        "language" => language(),
        "defaults" => defaults(ctx),
        "printers" => printers(),
        "storage" => storage(),
        "about" => about(),
        "users" => users(),
        _ => vec![],
    }
}

// ------------------------------------------------------------------ Conexões

fn wifi(ctx: &Ctx) -> Vec<Group> {
    if !has("nmcli") {
        return vec![group("", vec![note("O NetworkManager (nmcli) não está instalado.")])];
    }
    let on = sh("nmcli -t -f WIFI radio").trim() == "enabled";
    let mut gs = vec![group("", vec![sw("wifi", BLUE, "Wi‑Fi", if on { "Ligado" } else { "Desligado" }, on, Act::Run(format!("nmcli radio wifi {}", if on { "off" } else { "on" })))])];
    // conexão ativa (Wi‑Fi ou cabo)
    let devs = lines("nmcli -t -f DEVICE,TYPE,STATE,CONNECTION dev");
    let active: Vec<Vec<String>> = devs.iter().map(|l| nm_fields(l)).filter(|f| f.len() >= 4 && f[2] == "connected" && (f[1] == "wifi" || f[1] == "ethernet")).collect();
    if let Some(a) = active.first() {
        let (dev, kind, conn) = (a[0].clone(), a[1].clone(), a[3].clone());
        let show = lines(&format!("nmcli -t -f IP4.ADDRESS,IP4.GATEWAY,IP4.DNS dev show {}", q(&dev)));
        let get = |k: &str| show.iter().filter(|l| l.starts_with(k)).map(|l| l.split_once(':').map(|x| x.1).unwrap_or("").to_string()).collect::<Vec<_>>().join(", ");
        let method = sh(&format!("nmcli -t -f ipv4.method con show {}", q(&conn)));
        let manual = method.contains("manual");
        let mut rows = vec![
            item(if kind == "wifi" { "wifi" } else { "settings_ethernet" }, BLUE, &conn, if kind == "wifi" { "Conectado" } else { "Cabo conectado" }, &dev, true, Act::Nothing),
            info("Endereço IP", &get("IP4.ADDRESS")),
            info("Roteador", &get("IP4.GATEWAY")),
            info("DNS", &get("IP4.DNS")),
            choice("tune", GRAY, "Configurar IPv4", vec![
                ("Automático", Act::Run(format!("nmcli con mod {c} ipv4.method auto ipv4.addresses '' ipv4.gateway '' ipv4.dns '' && nmcli con up {c}", c = q(&conn)))),
                ("Manual", Act::Expand("ipmanual".into())),
            ], if manual || ctx.expanded.as_deref() == Some("ipmanual") { 1 } else { 0 }),
        ];
        if manual || ctx.expanded.as_deref() == Some("ipmanual") {
            rows.push(field("ip_addr", "Endereço/máscara", "192.168.0.50/24", false));
            rows.push(field("ip_gw", "Roteador", "192.168.0.1", false));
            rows.push(field("ip_dns", "DNS", "1.1.1.1 8.8.8.8", false));
            rows.push(Row::Button { label: "Aplicar IP manual".into(), button: "Aplicar".into(), primary: true, act: Act::Submit {
                template: format!("nmcli con mod {c} ipv4.method manual ipv4.addresses {{ip_addr}} ipv4.gateway {{ip_gw}} ipv4.dns {{ip_dns}} && nmcli con up {c}", c = q(&conn)),
                root: false,
            } });
        }
        rows.push(button("Desconectar", "Desconectar", Act::Run(format!("nmcli con down {}", q(&conn)))));
        rows.push(button("Esquecer esta rede", "Esquecer", Act::Run(format!("nmcli con delete {}", q(&conn)))));
        gs.push(group("Rede atual", rows));
    }
    if on {
        let mut seen = std::collections::HashSet::new();
        let mut nets: Vec<(String, u32, bool, bool)> = Vec::new();
        for l in lines("nmcli -t -f IN-USE,SSID,SIGNAL,SECURITY dev wifi list") {
            let f = nm_fields(&l);
            if f.len() < 4 || f[1].is_empty() || !seen.insert(f[1].clone()) {
                continue;
            }
            nets.push((f[1].clone(), f[2].parse().unwrap_or(0), !f[3].is_empty() && f[3] != "--", f[0] == "*"));
        }
        nets.sort_by(|a, b| b.3.cmp(&a.3).then(b.1.cmp(&a.1)));
        let known: Vec<String> = lines("nmcli -t -f NAME,TYPE con show").iter().map(|l| nm_fields(l)).filter(|f| f.len() >= 2 && f[1].contains("wireless")).map(|f| f[0].clone()).collect();
        let mut rows = Vec::new();
        for (ssid, sig, secure, inuse) in nets.into_iter().take(14) {
            let icon = if sig > 66 { "wifi" } else if sig > 33 { "wifi_2_bar" } else { "wifi_1_bar" };
            let trailing = if secure { "🔒" } else { "" };
            let sub = if inuse { "Conectado".to_string() } else if known.contains(&ssid) { "Rede salva".into() } else { format!("Sinal {sig}%") };
            rows.push(item(icon, BLUE, &ssid, &sub, trailing.trim(), inuse, if inuse { Act::Nothing } else { Act::Expand(format!("net:{ssid}")) }));
            if ctx.expanded.as_deref() == Some(&format!("net:{ssid}")) {
                if secure && !known.contains(&ssid) {
                    let id = format!("wifi_pw:{ssid}");
                    rows.push(field(&id, "Senha", "Senha da rede", true));
                    rows.push(Row::Button { label: format!("Entrar em {ssid}"), button: "Conectar".into(), primary: true, act: Act::Submit { template: format!("nmcli dev wifi connect {} password {{{id}}}", q(&ssid)), root: false } });
                } else {
                    rows.push(Row::Button { label: format!("Entrar em {ssid}"), button: "Conectar".into(), primary: true, act: Act::Run(format!("nmcli dev wifi connect {}", q(&ssid))) });
                }
            }
        }
        if rows.is_empty() {
            rows.push(note("Nenhuma rede encontrada."));
        }
        rows.push(button("Procurar redes", "Atualizar", Act::Run("nmcli dev wifi rescan".into())));
        gs.push(group("Outras redes", rows));
    }
    gs
}

fn hotspot() -> Vec<Group> {
    let active = lines("nmcli -t -f NAME,TYPE,DEVICE con show --active").iter().any(|l| l.starts_with("Hotspot:"));
    let mut rows = vec![sw("wifi_tethering", GREEN, "Acesso Pessoal", "Compartilha a internet deste computador por Wi‑Fi", active, if active { Act::Run("nmcli con down Hotspot".into()) } else { Act::Submit { template: "nmcli dev wifi hotspot ssid {hs_name} password {hs_pw}".into(), root: false } })];
    if !active {
        rows.push(field("hs_name", "Nome da rede", "Obelisk", false));
        rows.push(field("hs_pw", "Senha (8+ caracteres)", "senha", true));
    } else {
        let pw = sh("nmcli -s -g 802-11-wireless-security.psk con show Hotspot");
        let name = sh("nmcli -g 802-11-wireless.ssid con show Hotspot");
        rows.push(info("Nome", name.trim()));
        rows.push(info("Senha", pw.trim()));
        let leases = lines("cat /var/lib/NetworkManager/dnsmasq-*.leases 2>/dev/null");
        rows.push(info("Aparelhos conectados", &leases.len().to_string()));
    }
    vec![group("", rows)]
}

fn bluetooth(_ctx: &Ctx) -> Vec<Group> {
    if !has("bluetoothctl") {
        return vec![group("", vec![note("O BlueZ (bluetoothctl) não está instalado.")])];
    }
    let show = sh("bluetoothctl show");
    if !show.contains("Controller") {
        return vec![group("", vec![note("Nenhum adaptador Bluetooth encontrado.")])];
    }
    let on = show.lines().any(|l| l.trim() == "Powered: yes");
    let mut gs = vec![group("", vec![sw("bluetooth", BLUE, "Bluetooth", if on { "Ligado" } else { "Desligado" }, on, Act::Run(format!("bluetoothctl power {}", if on { "off" } else { "on" })))])];
    if on {
        let mut rows = Vec::new();
        for l in lines("bluetoothctl devices") {
            let mut parts = l.splitn(3, ' ');
            let (_, mac, name) = (parts.next(), parts.next().unwrap_or(""), parts.next().unwrap_or(""));
            let inf = sh(&format!("bluetoothctl info {mac}"));
            let connected = inf.contains("Connected: yes");
            let paired = inf.contains("Paired: yes");
            let icon = inf.lines().find(|l| l.trim_start().starts_with("Icon:")).map(|l| l.split(':').nth(1).unwrap_or("").trim().to_string()).unwrap_or_default();
            let sym = match icon.as_str() {
                "audio-headset" | "audio-headphones" | "audio-card" => "headphones",
                "input-gaming" => "sports_esports",
                "input-keyboard" => "keyboard",
                "input-mouse" => "mouse",
                "phone" => "mobile",
                _ => "bluetooth",
            };
            let battery = inf.lines().find(|l| l.contains("Battery Percentage")).and_then(|l| l.rsplit('(').next()).map(|s| format!("{}%", s.trim_end_matches(')'))).unwrap_or_default();
            let sub = if connected { "Conectado" } else if paired { "Pareado" } else { "Disponível" };
            let act = if connected { Act::Run(format!("bluetoothctl disconnect {mac}")) } else if paired { Act::Run(format!("bluetoothctl connect {mac}")) } else { Act::Run(format!("bluetoothctl pair {mac} && bluetoothctl trust {mac} && bluetoothctl connect {mac}")) };
            rows.push(item(sym, BLUE, name, sub, &battery, connected, act));
        }
        if rows.is_empty() {
            rows.push(note("Nenhum dispositivo. Coloque o aparelho em modo de pareamento e procure."));
        }
        rows.push(button("Procurar dispositivos (10 s)", "Procurar", Act::Run("bluetoothctl --timeout 10 scan on".into())));
        gs.push(group("Dispositivos", rows));
    }
    gs
}

// ------------------------------------------------------------------ Som

fn pactl_list(kind: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut name = String::new();
    for l in sh(&format!("pactl list {kind}")).lines() {
        let t = l.trim();
        if let Some(n) = t.strip_prefix("Name: ") {
            name = n.to_string();
        } else if let Some(d) = t.strip_prefix("Description: ") {
            if !name.ends_with(".monitor") {
                out.push((name.clone(), d.to_string()));
            }
        }
    }
    out
}

fn vol(target: &str) -> (f32, bool) {
    let r = sh(&format!("wpctl get-volume {target}"));
    (r.split_whitespace().nth(1).and_then(|v| v.parse().ok()).unwrap_or(0.0), r.contains("MUTED"))
}

fn sound() -> Vec<Group> {
    if !has("pactl") {
        return vec![group("", vec![note("PipeWire/pactl não encontrado.")])];
    }
    let dsink = sh("pactl get-default-sink").trim().to_string();
    let dsrc = sh("pactl get-default-source").trim().to_string();
    let (ov, om) = vol("@DEFAULT_AUDIO_SINK@");
    let (iv, im) = vol("@DEFAULT_AUDIO_SOURCE@");
    let mut out_rows = vec![
        Row::Slider { icon: "volume_up".into(), color: PINK, label: "Volume de saída".into(), value: ov.min(1.0), template: "wpctl set-volume @DEFAULT_AUDIO_SINK@ {v}%".into(), root: false },
        sw("volume_off", GRAY, "Sem som", "", om, Act::Run("wpctl set-mute @DEFAULT_AUDIO_SINK@ toggle".into())),
    ];
    for (n, d) in pactl_list("sinks") {
        out_rows.push(item("speaker", PINK, &d, "", "", n == dsink, Act::Run(format!("pactl set-default-sink {}", q(&n)))));
    }
    let mut in_rows = vec![
        Row::Slider { icon: "mic".into(), color: ORANGE, label: "Volume de entrada".into(), value: iv.min(1.0), template: "wpctl set-volume @DEFAULT_AUDIO_SOURCE@ {v}%".into(), root: false },
        sw("mic_off", GRAY, "Microfone mudo", "", im, Act::Run("wpctl set-mute @DEFAULT_AUDIO_SOURCE@ toggle".into())),
    ];
    for (n, d) in pactl_list("sources") {
        in_rows.push(item("mic", ORANGE, &d, "", "", n == dsrc, Act::Run(format!("pactl set-default-source {}", q(&n)))));
    }
    vec![group("Saída", out_rows), group("Entrada", in_rows)]
}

// ------------------------------------------------------------------ Jogos

fn game(ctx: &Ctx) -> Vec<Group> {
    let mango_conf = std::path::PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(".config/environment.d/90-obelisk-mangohud.conf");
    let mango_on = std::fs::read_to_string(&mango_conf).unwrap_or_default();
    let mango = if mango_on.contains("fps_only") { 1 } else if mango_on.contains("MANGOHUD=1") { 2 } else { 0 };
    let set_mango = |mode: &str| {
        let body = match mode {
            "off" => "".to_string(),
            "fps" => "MANGOHUD=1\nMANGOHUD_CONFIG=fps_only,position=top-right\n".to_string(),
            _ => "MANGOHUD=1\nMANGOHUD_CONFIG=full,position=top-left\n".to_string(),
        };
        Act::Run(format!("mkdir -p ~/.config/environment.d && printf %s {} > ~/.config/environment.d/90-obelisk-mangohud.conf", q(&body)))
    };
    let mut rows = vec![sw("sports_esports", GREEN, "Ligar sozinho com jogos", "Steam, gamescope, cs2 e apps que se declaram jogo", ctx.on("game_auto"), toggle(ctx, "game_auto"))];
    rows.push(info("Feral GameMode", if has("gamemoded") { "Instalado (prioridade de CPU/GPU no jogo)" } else { "Não instalado" }));
    let mut perf = vec![];
    if has("mangohud") {
        perf.push(choice("speed", ORANGE, "Desempenho na tela (MangoHud)", vec![("Desligado", set_mango("off")), ("Só FPS", set_mango("fps")), ("Completo", set_mango("full"))], mango));
        perf.push(note("Vale para jogos abertos depois da próxima sessão. Shift direito + F12 mostra/esconde no jogo."));
    } else {
        perf.push(note("Instale o MangoHud (pacote mangohud) para ver FPS e uso de CPU/GPU nos jogos."));
    }
    perf.push(button("Modo Console (Steam Big Picture no gamescope)", "Abrir", Act::Set("console".into(), "1".into())));
    vec![
        group("Modo Jogo", rows),
        group("Desempenho", perf),
        group("Jogos reconhecidos", vec![note("Classes extras (uma por linha) em ~/.config/obelisk/games."), button("Lista de jogos", "Editar…", Act::Run("mkdir -p ~/.config/obelisk && touch ~/.config/obelisk/games && xdg-open ~/.config/obelisk/games".into()))]),
    ]
}

fn controllers() -> Vec<Group> {
    let text = std::fs::read_to_string("/proc/bus/input/devices").unwrap_or_default();
    let mut rows = Vec::new();
    for block in text.split("\n\n") {
        if !block.contains("js") || !block.contains("Handlers=") {
            continue;
        }
        let name = block.lines().find(|l| l.starts_with("N: Name=")).map(|l| l.trim_start_matches("N: Name=").trim_matches('"').to_string()).unwrap_or_default();
        let handler = block.lines().find(|l| l.starts_with("H: Handlers=")).and_then(|l| l.split_whitespace().find(|h| h.starts_with("js"))).unwrap_or("").to_string();
        rows.push(item("sports_esports", PURPLE, &name, &format!("/dev/input/{handler}"), "", true, Act::Nothing));
    }
    for l in lines("upower -e") {
        if l.contains("gaming_input") {
            let inf = sh(&format!("upower -i {l}"));
            let pct = inf.lines().find(|x| x.trim_start().starts_with("percentage:")).map(|x| x.split(':').nth(1).unwrap_or("").trim().to_string()).unwrap_or_default();
            rows.push(info("Bateria do controle", &pct));
        }
    }
    if rows.is_empty() {
        rows.push(note("Nenhum controle conectado. Conecte por USB ou Bluetooth."));
    }
    vec![group("Controles conectados", rows)]
}

fn proton() -> Vec<Group> {
    let dir = std::path::PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(".steam/root/compatibilitytools.d");
    let mut rows = Vec::new();
    if let Ok(rd) = std::fs::read_dir(&dir) {
        let mut v: Vec<String> = rd.flatten().map(|e| e.file_name().to_string_lossy().to_string()).collect();
        v.sort();
        for name in v.into_iter().rev() {
            rows.push(item("deployed_code", ORANGE, &name, "Instalado", "", false, Act::Expand(format!("proton:{name}"))));
            rows.push(button(&format!("Remover {name}"), "Remover", Act::Run(format!("rm -rf {}", q(&dir.join(&name).to_string_lossy())))));
        }
    }
    if rows.is_empty() {
        rows.push(note("Nenhuma versão do Proton-GE instalada."));
    }
    // baixa a última versão do GitHub e confere o SHA-512 (como o kyro), num terminal visível
    let script = "set -e; cd /tmp; u=$(curl -fsSL https://api.github.com/repos/GloriousEggroll/proton-ge-custom/releases/latest | grep -o 'https://[^\"]*\\.tar\\.gz' | head -1); \
s=$(curl -fsSL https://api.github.com/repos/GloriousEggroll/proton-ge-custom/releases/latest | grep -o 'https://[^\"]*\\.sha512sum' | head -1); \
curl -fLO \"$u\"; curl -fLO \"$s\"; sha512sum -c $(basename \"$s\"); mkdir -p ~/.steam/root/compatibilitytools.d; tar -xf $(basename \"$u\") -C ~/.steam/root/compatibilitytools.d; echo; echo 'Instalado. Reinicie a Steam.'; read -r _";
    rows.push(button("Instalar a versão mais recente", "Instalar…", Act::Run(format!("foot -e sh -c {}", q(script)))));
    vec![group("Proton-GE (Steam)", rows)]
}

fn kernel() -> Vec<Group> {
    let current = sh("uname -r").trim().to_string();
    let zen = !sh("pacman -Q linux-zen 2>/dev/null").trim().is_empty();
    let mut rows = vec![info("Em uso", &current), info("linux-zen", if zen { "Instalado" } else { "Não instalado" })];
    if zen {
        rows.push(button("Remover o linux-zen", "Remover", Act::Root("pacman -Rns --noconfirm linux-zen linux-zen-headers".into())));
    } else {
        rows.push(button("Instalar o linux-zen (melhor latência para jogos)", "Instalar", Act::Root("pacman -S --needed --noconfirm linux-zen linux-zen-headers".into())));
    }
    rows.push(note("O kernel padrão do Arch fica de reserva no menu de boot. Depois de instalar, reinicie e escolha o linux-zen."));
    vec![group("Kernel", rows)]
}

// ------------------------------------------------------------------ Personalização / Energia

fn displays(ctx: &Ctx) -> Vec<Group> {
    let mut gs = Vec::new();
    // escala: como "Mais espaço ↔ Texto maior" do macOS
    let scales = [("100 %", "1"), ("125 %", "1.25"), ("150 %", "1.5"), ("175 %", "1.75"), ("200 %", "2")];
    let sel = scales.iter().position(|(_, v)| (v.parse::<f64>().unwrap() - ctx.fscale).abs() < 0.01).unwrap_or(0);
    let mut main = vec![choice("zoom_in", BLUE, "Escala", scales.iter().map(|(l, v)| (*l, set("scale", v))).collect(), sel)];
    main.push(note("Escalas fracionárias ficam nítidas nos apps que suportam (GTK 4, Qt 6, Firefox, Chromium); os outros são reduzidos pela GPU."));
    if ctx.modes.is_empty() {
        // modo aninhado: só mostra as telas
        for (n, m) in &ctx.outputs {
            main.insert(0, item("desktop_windows", BLUE, n, m, "", true, Act::Nothing));
        }
        gs.push(group("Telas", main));
    } else {
        for (i, (name, modes, cur, scale)) in ctx.modes.iter().enumerate() {
            let title = if i == 0 { format!("{name} (principal)") } else { name.clone() };
            let mut rows = vec![info("Resolução atual", &format!("{} Hz · escala {} %", cur.replace('@', " a ").replace('x', "×"), (scale * 100.0).round() as i32))];
            let key = format!("res:{name}");
            rows.push(item("aspect_ratio", BLUE, "Resolução e taxa", "", &cur.replace('@', " · ").replace('x', "×"), false, Act::Expand(key.clone())));
            if ctx.expanded.as_deref() == Some(&key) {
                // modos únicos, do maior para o menor; taxas mais altas primeiro
                let mut ms: Vec<&String> = modes.iter().collect();
                ms.sort_by_key(|m| {
                    let (wh, hz) = m.split_once('@').unwrap_or((m, "0"));
                    let (w, h) = wh.split_once('x').unwrap_or(("0", "0"));
                    std::cmp::Reverse((w.parse::<u32>().unwrap_or(0) * h.parse::<u32>().unwrap_or(0), hz.parse::<u32>().unwrap_or(0)))
                });
                ms.dedup();
                for m in ms.into_iter().take(14) {
                    let label = m.replace('@', " · ").replace('x', "×") + " Hz";
                    rows.push(item("subdirectory_arrow_right", GRAY, &label, "", "", m == cur, set(&format!("mode:{name}"), m)));
                }
            }
            if i == 0 {
                rows.extend(main.drain(..));
            } else {
                rows.push(button("A barra de menus, o Dock e o notch vão para esta tela", "Usar como principal", set("primary", name)));
            }
            gs.push(group(&title, rows));
        }
        if ctx.modes.len() > 1 {
            let on = ctx.on("mirror");
            gs.insert(0, group("", vec![sw("screen_share", BLUE, "Espelhar telas", "Todas mostram a tela principal (ex.: projetor)", on, set("mirror", if on { "false" } else { "true" }))]));
        }
    }
    let mut extra = vec![];
    if ctx.nested {
        extra.push(note("Night Shift e VRR funcionam na sessão nativa (o modo aninhado não controla a tela)."));
    } else {
        extra.push(sw("nightlight", ORANGE, "Night Shift", "Cores mais quentes", ctx.night, set("night", if ctx.night { "false" } else { "true" })));
        extra.push(note("VRR liga sozinho com jogos em tela cheia."));
    }
    gs.push(group("Cor", extra));
    gs
}

fn battery() -> Vec<Group> {
    let dev = lines("upower -e").into_iter().find(|l| l.contains("BAT"));
    let Some(dev) = dev else { return vec![group("", vec![note("Este computador não tem bateria.")])] };
    let inf = sh(&format!("upower -i {dev}"));
    let get = |k: &str| inf.lines().find(|l| l.trim_start().starts_with(k)).map(|l| l.split_once(':').map(|x| x.1.trim().to_string()).unwrap_or_default()).unwrap_or_default();
    let profile = sh("powerprofilesctl get").trim().to_string();
    let sel = match profile.as_str() {
        "power-saver" => 0,
        "performance" => 2,
        _ => 1,
    };
    vec![
        group("Bateria", vec![
            info("Nível", &get("percentage:")),
            info("Estado", &match get("state:").as_str() { "charging" => "Carregando".into(), "discharging" => "Na bateria".into(), "fully-charged" => "Carregada".into(), s => s.to_string() }),
            info("Tempo", &[get("time to empty:"), get("time to full:")].into_iter().find(|s| !s.is_empty()).unwrap_or_default()),
            info("Saúde", &get("capacity:")),
        ]),
        group("Energia", vec![choice("bolt", GREEN, "Perfil", vec![("Economia", Act::Run("powerprofilesctl set power-saver".into())), ("Equilibrado", Act::Run("powerprofilesctl set balanced".into())), ("Desempenho", Act::Run("powerprofilesctl set performance".into()))], sel)]),
    ]
}

fn lock(ctx: &Ctx) -> Vec<Group> {
    let m = ctx.get("lock_minutes");
    let sel = match m {
        "0" => 0,
        "5" => 1,
        "10" => 2,
        _ => 3,
    };
    vec![
        group("Tela Bloqueada", vec![choice("lock", DARK, "Bloquear depois de", vec![("Nunca", set("lock_minutes", "0")), ("5 min", set("lock_minutes", "5")), ("10 min", set("lock_minutes", "10")), ("30 min", set("lock_minutes", "30"))], sel)]),
        group("Energia", vec![
            button("Repouso", "Repouso", Act::Run("systemctl suspend".into())),
            button("Reiniciar", "Reiniciar", Act::Run("systemctl reboot".into())),
            button("Desligar", "Desligar", Act::Run("systemctl poweroff".into())),
        ]),
    ]
}

fn snapshots() -> Vec<Group> {
    if !has("snapper") {
        return vec![group("", vec![note("Instantâneos precisam do snapper e de um disco btrfs. O instalador do Obelisk liga isso sozinho em discos btrfs.")])];
    }
    let list = lines("snapper --csvout --no-headers -c root list --columns number,date,description 2>/dev/null");
    let mut rows = Vec::new();
    for l in list.iter().rev().take(12) {
        let f: Vec<&str> = l.splitn(3, ',').collect();
        if f.len() == 3 && f[0] != "0" {
            rows.push(item("history", PURPLE, if f[2].is_empty() { "Instantâneo" } else { f[2] }, f[1], &format!("#{}", f[0]), false, Act::Nothing));
        }
    }
    if rows.is_empty() {
        rows.push(note("Nenhum instantâneo listado (pode exigir permissão: ALLOW_USERS no snapper)."));
    }
    rows.push(button("Criar instantâneo agora", "Criar", Act::Root("snapper -c root create -d 'Manual (Ajustes)'".into())));
    vec![group("Instantâneos", rows)]
}

fn updates() -> Vec<Group> {
    let pending = if has("checkupdates") { lines("checkupdates 2>/dev/null") } else { vec![] };
    let apps: Vec<String> = if has("flatpak") { sh_t("flatpak remote-ls --updates --app --columns=application 2>/dev/null", 30_000).lines().map(str::to_string).filter(|l| !l.is_empty()).collect() } else { vec![] };
    let total = pending.len() + apps.len();
    let mut rows = vec![info(
        "Disponíveis",
        &if !has("checkupdates") { "instale o pacman-contrib".to_string() } else if total == 0 { "Tudo atualizado".to_string() } else { format!("{} do sistema · {} apps", pending.len(), apps.len()) },
    )];
    for l in pending.iter().take(8) {
        let mut w = l.split_whitespace();
        let name = w.next().unwrap_or("");
        let ver = w.collect::<Vec<_>>().join(" ");
        rows.push(item("update", GRAY, name, &ver, "", false, Act::Nothing));
    }
    if pending.len() > 8 {
        rows.push(note(&format!("e mais {} pacotes.", pending.len() - 8)));
    }
    // tudo dentro dos Ajustes, com progresso: a senha é pedida pelo notch (polkit)
    rows.push(Row::Button {
        label: "Atualizar o sistema e os apps".into(),
        button: "Atualizar agora".into(),
        primary: true,
        act: Act::Job(
            "Atualizando o sistema".into(),
            "pkexec pacman -Syu --noconfirm && { command -v flatpak >/dev/null && flatpak update -y --noninteractive || true; }".into(),
        ),
    });
    rows.push(note("O Obelisk procura atualizações a cada 6 horas (fora do Modo Jogo) e avisa no notch."));
    vec![group("Atualizações", rows)]
}

// ------------------------------------------------------------------ Teclado e Mouse

fn keyboard(ctx: &Ctx) -> Vec<Group> {
    let layout = ctx.get("kb_layout");
    let sel = match layout {
        "br" => 1,
        "us(intl)" => 2,
        _ => 0,
    };
    vec![
        group("Teclado", vec![
            choice("keyboard", GRAY, "Layout", vec![("EUA", set("kb_layout", "us")), ("ABNT2", set("kb_layout", "br")), ("EUA intern.", set("kb_layout", "us(intl)"))], sel),
            Row::Slider { icon: "speed".into(), color: GRAY, label: "Repetição de teclas".into(), value: ctx.get("kb_rate").parse::<f32>().unwrap_or(30.0) / 60.0, template: "set:kb_rate:{v}".into(), root: false },
            Row::Slider { icon: "timer".into(), color: GRAY, label: "Atraso até repetir".into(), value: ctx.get("kb_delay").parse::<f32>().unwrap_or(300.0) / 1000.0, template: "set:kb_delay:{v}".into(), root: false },
            sw("keyboard_command_key", GRAY, "⌘ funciona como Ctrl nos apps", if ctx.nested { "Na sessão nativa (aninhado o ⌘ é o Alt)" } else { "⌘C, ⌘V, ⌘W… como no macOS" }, ctx.on("cmd_as_ctrl"), toggle(ctx, "cmd_as_ctrl")),
        ]),
        group("Atalhos", vec![
            info("Busca", "⌘ Espaço"),
            info("Trocar de app", "⌘ Tab"),
            info("Mission Control · App Exposé", "⌃ ↑ · ⌃ ↓"),
            info("Capturas", "⇧⌘3 · ⇧⌘4 · ⇧⌘5"),
            info("Emojis", "⌃⌘ Espaço"),
            info("Bloquear", "⌃⌘ Q"),
        ]),
    ]
}

fn mouse(ctx: &Ctx) -> Vec<Group> {
    vec![group("Mouse e Trackpad", vec![
        sw("swap_vert", BLUE, "Rolagem natural", "O conteúdo acompanha o dedo, como no Mac", ctx.on("natural_scroll"), toggle(ctx, "natural_scroll")),
        sw("mouse", GRAY, "Aceleração do ponteiro", if ctx.nested { "Na sessão nativa" } else { "Desligada é melhor para jogos" }, ctx.on("pointer_accel"), toggle(ctx, "pointer_accel")),
        Row::Slider { icon: "speed".into(), color: GRAY, label: "Velocidade do ponteiro".into(), value: (ctx.get("pointer_speed").parse::<f32>().unwrap_or(0.0) + 1.0) / 2.0, template: "set:pointer_speed:{v}".into(), root: false },
    ])]
}

// ------------------------------------------------------------------ Sistema

fn datetime(ctx: &Ctx) -> Vec<Group> {
    let tz = sh("timedatectl show -p Timezone --value").trim().to_string();
    let ntp = sh("timedatectl show -p NTP --value").trim() == "yes";
    let mut rows = vec![
        sw("schedule", BLUE, "Hora automática", "Sincroniza pela internet (NTP)", ntp, Act::Run(format!("timedatectl set-ntp {}", !ntp))),
        sw("schedule", GRAY, "Relógio de 24 horas", "", ctx.get("clock24") != "false", toggle(ctx, "clock24")),
        info("Fuso horário", &tz),
        field("tz_search", "Mudar fuso", "Buscar (ex.: Sao_Paulo)", false),
    ];
    let qy = ctx.settings.get("field:tz_search").cloned().unwrap_or_default().to_lowercase();
    if qy.len() >= 2 {
        for z in lines("timedatectl list-timezones").into_iter().filter(|z| z.to_lowercase().contains(&qy)).take(6) {
            rows.push(item("public", BLUE, &z, "", "", z == tz, Act::Run(format!("timedatectl set-timezone {}", q(&z)))));
        }
    }
    vec![group("Data e Hora", rows)]
}

fn language() -> Vec<Group> {
    let cur = sh("localectl status").lines().find(|l| l.contains("LANG=")).and_then(|l| l.split("LANG=").nth(1)).unwrap_or("").trim().to_string();
    let langs = [("pt_BR.UTF-8", "Português (Brasil)"), ("en_US.UTF-8", "English (US)"), ("es_ES.UTF-8", "Español"), ("fr_FR.UTF-8", "Français"), ("de_DE.UTF-8", "Deutsch")];
    let mut rows = vec![note("Idioma dos apps (o shell do Obelisk é em português). Gera o locale e vale a partir da próxima sessão.")];
    for (code, name) in langs {
        let cmd = format!("sed -i 's/^#{c} UTF-8/{c} UTF-8/' /etc/locale.gen && locale-gen && localectl set-locale LANG={c}", c = code);
        rows.push(item("language", BLUE, name, code, "", cur == code, Act::Root(cmd)));
    }
    vec![group("Idioma", rows)]
}

fn defaults(ctx: &Ctx) -> Vec<Group> {
    let kinds = [
        ("Navegador", "x-scheme-handler/https", "public"),
        ("Pastas", "inode/directory", "folder"),
        ("Texto", "text/plain", "description"),
        ("PDF", "application/pdf", "picture_as_pdf"),
        ("Imagens", "image/png", "image"),
        ("Vídeos", "video/mp4", "movie"),
        ("Música", "audio/mpeg", "music_note"),
        ("Arquivos compactados", "application/zip", "folder_zip"),
    ];
    let apps = crate::apps::load();
    let mut rows = Vec::new();
    for (label, mime, icon) in kinds {
        let cur = sh(&format!("xdg-mime query default {mime}")).trim().trim_end_matches(".desktop").to_string();
        let name = apps.iter().find(|a| a.id == cur).map(|a| a.name.clone()).unwrap_or(cur);
        let key = format!("mime:{mime}");
        rows.push(item(icon, GRAY, label, "", &name, false, Act::Expand(key.clone())));
        if ctx.expanded.as_deref() == Some(&key) {
            let cands = crate::apps::handlers_for(mime);
            for a in cands.iter().take(8) {
                let mut cmd = format!("xdg-mime default {}.desktop {mime}", a.id);
                if mime == "x-scheme-handler/https" {
                    cmd += &format!(" && xdg-mime default {}.desktop x-scheme-handler/http text/html", a.id);
                }
                if mime == "image/png" {
                    cmd += &format!(" && xdg-mime default {}.desktop image/jpeg image/webp image/gif", a.id);
                }
                rows.push(item("subdirectory_arrow_right", GRAY, &a.name, "", "", a.name == name, Act::Run(cmd)));
            }
        }
    }
    vec![group("Abrir com", rows)]
}

fn printers() -> Vec<Group> {
    if !has("lpstat") {
        return vec![group("", vec![note("O CUPS não está instalado (pacotes cups e system-config-printer).")])];
    }
    let default = sh("lpstat -d 2>/dev/null");
    let mut rows = Vec::new();
    for l in lines("lpstat -p 2>/dev/null") {
        let name = l.split_whitespace().nth(1).unwrap_or("").to_string();
        let idle = l.contains("idle") || l.contains("inativa");
        let jobs = lines(&format!("lpstat -o {name} 2>/dev/null")).len();
        rows.push(item("print", GRAY, &name, if idle { "Pronta" } else { "Ocupada" }, &format!("{jobs} na fila"), default.contains(&name), Act::Run(format!("lpoptions -d {}", q(&name)))));
    }
    if rows.is_empty() {
        rows.push(note("Nenhuma impressora."));
    }
    rows.push(button("Adicionar impressora", "Adicionar…", Act::Run("system-config-printer".into())));
    vec![group("Impressoras", rows)]
}

fn storage() -> Vec<Group> {
    let df = sh("df -h --output=size,used,avail,pcent / | tail -1");
    let f: Vec<&str> = df.split_whitespace().collect();
    let mut rows = vec![info("Disco do sistema", &if f.len() == 4 { format!("{} usados de {} ({} livres)", f[1], f[0], f[2]) } else { String::new() })];
    let du = |p: &str| sh(&format!("du -sh {p} 2>/dev/null | cut -f1")).trim().to_string();
    rows.push(info("Jogos (Steam)", &du("~/.local/share/Steam/steamapps")));
    rows.push(info("Cache dos apps", &du("~/.cache")));
    rows.push(info("Pacotes baixados", &du("/var/cache/pacman/pkg")));
    rows.push(info("Lixo", &du("~/.local/share/Trash")));
    let clean = vec![
        button("Esvaziar o Lixo", "Esvaziar", Act::Run("gio trash --empty".into())),
        button("Limpar miniaturas e cache dos apps", "Limpar", Act::Run("rm -rf ~/.cache/thumbnails ~/.cache/mesa_shader_cache_db".into())),
        button("Remover pacotes baixados antigos", "Limpar", Act::Root("paccache -rk1 2>/dev/null || pacman -Sc --noconfirm".into())),
        button("Remover runtimes do Flatpak sem uso", "Limpar", Act::Run("flatpak uninstall --unused -y".into())),
        button("Reduzir registros do sistema", "Limpar", Act::Root("journalctl --vacuum-time=2weeks".into())),
    ];
    vec![group("Uso", rows), group("Liberar espaço", clean)]
}

fn about() -> Vec<Group> {
    let cpu = std::fs::read_to_string("/proc/cpuinfo").ok().and_then(|s| s.lines().find(|l| l.starts_with("model name")).map(|l| l.split(':').nth(1).unwrap_or("").trim().to_string())).unwrap_or_default();
    let mem = std::fs::read_to_string("/proc/meminfo").ok().and_then(|s| s.lines().next().and_then(|l| l.split_whitespace().nth(1)).and_then(|v| v.parse::<f64>().ok())).map(|k| format!("{:.0} GB", k / 1024.0 / 1024.0)).unwrap_or_default();
    let gpu = sh("lspci 2>/dev/null | grep -iE 'vga|3d|display' | head -1 | cut -d: -f3").trim().to_string();
    let kernel = sh("uname -r").trim().to_string();
    let host = std::fs::read_to_string("/etc/hostname").unwrap_or_default().trim().to_string();
    let disk = sh("df -h --output=size / | tail -1").trim().to_string();
    vec![group("", vec![info("Nome", &host), info("Processador", &cpu), info("Gráficos", &gpu), info("Memória", &mem), info("Disco", &disk), info("Kernel", &kernel)])]
}

fn users() -> Vec<Group> {
    let user = crate::lock::current_user();
    let gecos = sh(&format!("getent passwd {user} | cut -d: -f5")).split(',').next().unwrap_or("").trim().to_string();
    vec![
        group("Conta", vec![info("Usuário", &user), info("Nome completo", &gecos), field("full_name", "Novo nome", "Nome e sobrenome", false), Row::Button { label: "Mudar o nome".into(), button: "Salvar".into(), primary: true, act: Act::Submit { template: format!("usermod -c {{full_name}} {}", q(&user)), root: true } }]),
        group("Foto", vec![field("avatar_path", "Imagem", "~/Imagens/foto.jpg", false), Row::Button { label: "Usar como foto da conta".into(), button: "Usar".into(), primary: false, act: Act::Submit { template: "cp $(eval echo {avatar_path}) ~/.face".into(), root: false } }]),
        group("Senha", vec![button("Mudar a senha (abre o terminal)", "Mudar…", Act::Run("foot -e passwd".into()))]),
    ]
}
