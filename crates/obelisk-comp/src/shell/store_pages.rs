//! App Store do Obelisk (como a App Store do macOS): apps do Flathub instalados para o usuário,
//! sem senha. Catálogo guardado em ~/.cache/obelisk/flathub.tsv (renovado a cada dia); ícones do
//! appstream do Flathub. Instalar/atualizar/remover são tarefas com progresso (Act::Job).

use super::settings_model::*;

const BLUE: (u8, u8, u8) = (0x30, 0x7c, 0xf6);
const GREEN: (u8, u8, u8) = (0x30, 0xd1, 0x58);
const ORANGE: (u8, u8, u8) = (0xff, 0x92, 0x30);
const PURPLE: (u8, u8, u8) = (0xa5, 0x5e, 0xea);
const PINK: (u8, u8, u8) = (0xff, 0x37, 0x8f);
const TEAL: (u8, u8, u8) = (0x00, 0xb8, 0xd4);
const GRAY: (u8, u8, u8) = (0x8e, 0x8e, 0x93);

pub const STORE_PAGES: [(&str, &str, &str, (u8, u8, u8), &str, &str); 9] = [
    ("s:descobrir", "Descobrir", "star", BLUE, "", "destaques novidades"),
    ("s:jogos", "Jogos", "sports_esports", GREEN, "", "games emuladores steam"),
    ("s:criar", "Criatividade", "brush", PINK, "", "foto video desenho musica"),
    ("s:trabalho", "Produtividade", "work", ORANGE, "", "escritorio documentos notas"),
    ("s:internet", "Internet", "public", TEAL, "", "navegador chat mensagens"),
    ("s:diversao", "Entretenimento", "movie", PURPLE, "", "musica video filmes"),
    ("s:util", "Utilitários", "build", GRAY, "", "ferramentas sistema"),
    ("s:instalados", "Instalados", "download_done", BLUE, "", "meus apps remover"),
    ("s:atualizacoes", "Atualizações", "update", GREEN, "", "atualizar"),
];

pub const STORE_SECTIONS: [(&str, &[&str]); 2] = [
    ("", &["s:descobrir", "s:jogos", "s:criar", "s:trabalho", "s:internet", "s:diversao", "s:util"]),
    ("Seus apps", &["s:instalados", "s:atualizacoes"]),
];

const GAMES: &[&str] = &[
    "com.heroicgameslauncher.hgl", "net.lutris.Lutris", "com.usebottles.bottles", "net.davidotek.pupgui2",
    "org.prismlauncher.PrismLauncher", "org.libretro.RetroArch", "net.pcsx2.PCSX2", "org.DolphinEmu.dolphin-emu",
    "net.rpcs3.RPCS3", "io.itch.itch", "com.github.Matoking.protontricks", "io.github.ryubing.Ryujinx",
];
const CREATE: &[&str] = &[
    "org.gimp.GIMP", "org.kde.krita", "org.inkscape.Inkscape", "org.blender.Blender", "org.kde.kdenlive",
    "com.obsproject.Studio", "org.audacityteam.Audacity", "fr.handbrake.ghb", "org.darktable.Darktable", "com.github.PintaProject.Pinta",
];
const WORK: &[&str] = &[
    "org.libreoffice.LibreOffice", "org.onlyoffice.desktopeditors", "md.obsidian.Obsidian", "org.mozilla.Thunderbird",
    "com.github.xournalpp.xournalpp", "org.gnome.Calendar", "net.cozic.joplin_desktop", "org.zotero.Zotero",
];
const NET: &[&str] = &[
    "com.discordapp.Discord", "com.google.Chrome", "com.brave.Browser", "org.telegram.desktop", "com.slack.Slack",
    "org.qbittorrent.qBittorrent", "org.localsend.localsend_app", "com.rustdesk.RustDesk", "us.zoom.Zoom",
];
const FUN: &[&str] = &[
    "com.spotify.Client", "org.videolan.VLC", "com.stremio.Stremio", "io.mpv.Mpv", "com.github.iwalton3.jellyfin-media-player",
    "io.freetubeapp.FreeTube", "org.kde.elisa", "com.plexamp.Plexamp",
];
const UTIL: &[&str] = &[
    "com.github.tchx84.Flatseal", "io.missioncenter.MissionCenter", "net.nokyan.Resources", "org.gnome.Boxes",
    "it.mijorus.gearlever", "com.github.wwmm.easyeffects", "io.github.ilya_zlobintsev.LACT", "org.gnome.DejaDup",
];

fn home() -> std::path::PathBuf {
    std::path::PathBuf::from(std::env::var("HOME").unwrap_or_default())
}

/// Flathub como remoto do usuário (instalar sem senha).
fn ensure_flathub() {
    if !sh("flatpak remotes --user --columns=name").lines().any(|l| l.trim() == "flathub") {
        sh_t("flatpak remote-add --user --if-not-exists flathub https://dl.flathub.org/repo/flathub.flatpakrepo", 30_000);
    }
}

/// Catálogo (id, nome, descrição), do cache ou do Flathub.
fn catalog() -> Vec<(String, String, String)> {
    let cache = home().join(".cache/obelisk/flathub.tsv");
    let fresh = std::fs::metadata(&cache).and_then(|m| m.modified()).ok().and_then(|t| t.elapsed().ok()).is_some_and(|e| e.as_secs() < 86_400);
    let text = if fresh {
        std::fs::read_to_string(&cache).unwrap_or_default()
    } else {
        ensure_flathub();
        // primeiro o appstream (nomes, descrições e ícones), depois a lista
        sh_t("flatpak update --user --appstream flathub", 90_000);
        let t = sh_t("flatpak remote-ls --user flathub --app --columns=application,name,description", 60_000);
        if !t.trim().is_empty() {
            let _ = std::fs::create_dir_all(cache.parent().unwrap());
            let _ = std::fs::write(&cache, &t);
            t
        } else {
            std::fs::read_to_string(&cache).unwrap_or_default()
        }
    };
    text.lines()
        .filter_map(|l| {
            let mut f = l.split('\t');
            Some((f.next()?.to_string(), f.next()?.to_string(), f.next().unwrap_or("").to_string()))
        })
        .collect()
}

fn installed() -> Vec<(String, String, String)> {
    sh("flatpak list --app --columns=application,name,version")
        .lines()
        .filter_map(|l| {
            let mut f = l.split('\t');
            Some((f.next()?.to_string(), f.next()?.to_string(), f.next().unwrap_or("").to_string()))
        })
        .collect()
}

/// Ícone do app no appstream do Flathub (usuário ou sistema).
pub fn icon_path(id: &str) -> String {
    for base in [home().join(".local/share/flatpak"), "/var/lib/flatpak".into()] {
        for size in ["128x128", "64x64"] {
            let p = base.join(format!("appstream/flathub/x86_64/active/icons/{size}/{id}.png"));
            if p.exists() {
                return p.to_string_lossy().into_owned();
            }
        }
        let p = base.join(format!("exports/share/icons/hicolor/128x128/apps/{id}.png"));
        if p.exists() {
            return p.to_string_lossy().into_owned();
        }
    }
    String::new()
}

fn app_row(id: &str, cat: &[(String, String, String)], inst: &[(String, String, String)]) -> Option<Row> {
    let (name, desc) = cat.iter().find(|c| c.0 == id).map(|c| (c.1.clone(), c.2.clone())).or_else(|| inst.iter().find(|c| c.0 == id).map(|c| (c.1.clone(), String::new())))?;
    let is_in = inst.iter().any(|i| i.0 == id);
    let (button, act) = if is_in {
        ("Abrir".to_string(), Act::Run(format!("flatpak run {}", q(id))))
    } else {
        ("Obter".to_string(), Act::Job(format!("Instalando {name}"), format!("flatpak install --user -y --noninteractive flathub {}", q(id))))
    };
    Some(Row::App { icon: icon_path(id), title: name, sub: desc, button, act })
}

fn list(title: &str, ids: &[&str], cat: &[(String, String, String)], inst: &[(String, String, String)]) -> Group {
    let rows: Vec<Row> = ids.iter().filter_map(|id| app_row(id, cat, inst)).collect();
    if rows.is_empty() {
        return group(title, vec![note("Sem conexão com o Flathub. Verifique a internet e abra a loja de novo.")]);
    }
    group(title, rows)
}

pub fn build(page: &str, ctx: &Ctx) -> Vec<Group> {
    if !has("flatpak") {
        return vec![group("", vec![note("A loja usa o Flatpak, que não está instalado (pacote flatpak).")])];
    }
    let cat = catalog();
    let inst = installed();
    match page {
        "s:descobrir" => vec![
            list("Para jogar", &["com.heroicgameslauncher.hgl", "net.lutris.Lutris", "com.discordapp.Discord", "net.davidotek.pupgui2"], &cat, &inst),
            list("Essenciais", &["com.spotify.Client", "org.videolan.VLC", "com.obsproject.Studio", "org.localsend.localsend_app", "md.obsidian.Obsidian"], &cat, &inst),
            list("Para criar", &["org.blender.Blender", "org.kde.krita", "org.gimp.GIMP", "org.kde.kdenlive"], &cat, &inst),
        ],
        "s:jogos" => vec![list("Jogos e emuladores", GAMES, &cat, &inst), group("", vec![note("A Steam já vem instalada no Obelisk (com Proton). Aqui ficam lojas e emuladores extras.")])],
        "s:criar" => vec![list("Criatividade", CREATE, &cat, &inst)],
        "s:trabalho" => vec![list("Produtividade", WORK, &cat, &inst)],
        "s:internet" => vec![list("Internet e conversas", NET, &cat, &inst)],
        "s:diversao" => vec![list("Música e vídeo", FUN, &cat, &inst)],
        "s:util" => vec![list("Utilitários", UTIL, &cat, &inst)],
        "s:instalados" => {
            let mut rows: Vec<Row> = inst
                .iter()
                .map(|(id, name, ver)| Row::App {
                    icon: icon_path(id),
                    title: name.clone(),
                    sub: if ver.is_empty() { id.clone() } else { format!("Versão {ver}") },
                    button: "Remover".into(),
                    act: Act::Job(format!("Removendo {name}"), format!("flatpak uninstall --user -y --noninteractive {} || pkexec flatpak uninstall -y --noninteractive {}", q(id), q(id))),
                })
                .collect();
            if rows.is_empty() {
                rows.push(note("Nenhum app da loja instalado ainda."));
            }
            vec![group("Apps da loja", rows)]
        }
        "s:atualizacoes" => {
            let ups: Vec<String> = sh_t("flatpak remote-ls --updates --app --columns=application", 30_000).lines().map(str::to_string).filter(|l| !l.is_empty()).collect();
            let mut rows: Vec<Row> = ups
                .iter()
                .map(|id| {
                    let name = inst.iter().find(|i| &i.0 == id).map(|i| i.1.clone()).unwrap_or_else(|| id.clone());
                    Row::App {
                        icon: icon_path(id),
                        title: name.clone(),
                        sub: "Atualização disponível".into(),
                        button: "Atualizar".into(),
                        act: Act::Job(format!("Atualizando {name}"), format!("flatpak update -y --noninteractive {}", q(id))),
                    }
                })
                .collect();
            if rows.is_empty() {
                rows.push(note("Todos os apps estão atualizados."));
            } else {
                rows.insert(0, Row::Button { label: format!("{} atualizações", ups.len()), button: "Atualizar tudo".into(), primary: true, act: Act::Job("Atualizando os apps".into(), "flatpak update -y --noninteractive".into()) });
            }
            vec![group("Atualizações dos apps", rows), group("", vec![note("Atualizações do sistema (pacman) ficam em Ajustes › Atualizações.")])]
        }
        "s:busca" => {
            let qy = ctx.get("store_query").to_lowercase();
            let rows: Vec<Row> = cat
                .iter()
                .filter(|(id, name, desc)| name.to_lowercase().contains(&qy) || id.to_lowercase().contains(&qy) || desc.to_lowercase().contains(&qy))
                .take(30)
                .filter_map(|(id, ..)| app_row(id, &cat, &inst))
                .collect();
            if rows.is_empty() {
                vec![group("", vec![note(&format!("Nada encontrado para “{qy}”."))])]
            } else {
                vec![group(&format!("Resultados para “{qy}”"), rows)]
            }
        }
        _ => vec![],
    }
}
