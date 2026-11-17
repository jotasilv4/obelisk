//! Barra de menus no estilo macOS 26 (Tahoe): transparente, 32 px, texto branco ou preto
//! conforme o brilho do papel de parede, com sombra suave no texto claro.
//! Medidas: docs/03-especificacao-kyro.md §3.

use super::layer::Layer;
use crate::services::SysStatus;
use crate::ui::canvas::Canvas;
use crate::ui::text::Weight;
use crate::ui::text_glyph;
use crate::ui::theme::{metrics, Rgba, Theme};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BarItem {
    Logo,
    App,
    File,
    Edit,
    View,
    Window,
    Go,
    Help,
    Workspace(usize),
    Bluetooth,
    Sound,
    Battery,
    Network,
    ControlCenter,
    Clock,
    Recording,
    Tray(usize),
}

impl BarItem {
    /// Itens que abrem menu suspenso (o resto abre painel ou age direto).
    pub fn is_menu(self) -> bool {
        matches!(self, BarItem::Logo | BarItem::App | BarItem::File | BarItem::Edit | BarItem::View | BarItem::Window | BarItem::Go | BarItem::Help)
    }
}

#[derive(Default)]
pub struct MenuBar {
    pub layer: Layer,
    key: Option<String>,
    /// (item, x0, x1) em px lógicos, para hit-test e posição dos menus.
    pub hits: Vec<(BarItem, f32, f32)>,
}

pub struct MenuBarInfo<'a> {
    pub app_name: &'a str,
    pub has_window: bool,
    pub workspace: usize,
    pub workspaces: usize,
    pub status: &'a SysStatus,
    pub open: Option<BarItem>,
    pub luminance: f32,
    /// segundos de gravação em curso
    pub recording: Option<u64>,
    /// ícones da bandeja (apps)
    pub tray: Vec<Option<resvg::tiny_skia::Pixmap>>,
}

pub fn clock_string() -> String {
    use chrono::{Datelike, Timelike};
    const DIAS: [&str; 7] = ["Seg", "Ter", "Qua", "Qui", "Sex", "Sáb", "Dom"];
    const MESES: [&str; 12] = ["jan", "fev", "mar", "abr", "mai", "jun", "jul", "ago", "set", "out", "nov", "dez"];
    let n = chrono::Local::now();
    format!(
        "{} {} de {}  {:02}:{:02}",
        DIAS[n.weekday().num_days_from_monday() as usize],
        n.day(),
        MESES[n.month0() as usize],
        n.hour(),
        n.minute()
    )
}

/// (porcentagem, carregando) da primeira bateria encontrada.
pub fn battery() -> Option<(u8, bool)> {
    for e in std::fs::read_dir("/sys/class/power_supply").ok()?.flatten() {
        let p = e.path();
        if std::fs::read_to_string(p.join("type")).ok()?.trim() != "Battery" {
            continue;
        }
        let cap: u8 = std::fs::read_to_string(p.join("capacity")).ok()?.trim().parse().ok()?;
        let status = std::fs::read_to_string(p.join("status")).unwrap_or_default();
        return Some((cap, status.trim() == "Charging"));
    }
    None
}

pub fn volume_symbol(s: &SysStatus) -> &'static str {
    match s.volume {
        _ if s.muted => "volume_off",
        Some(v) if v <= 0.001 => "volume_off",
        Some(v) if v < 0.34 => "volume_mute",
        Some(v) if v < 0.67 => "volume_down",
        _ => "volume_up",
    }
}

pub fn network_symbol(s: &SysStatus) -> &'static str {
    if s.wired && s.ssid.is_none() {
        return "settings_ethernet";
    }
    if s.wifi_on && s.ssid.is_some() {
        match s.signal {
            67.. => "wifi",
            34..=66 => "wifi_2_bar",
            _ => "wifi_1_bar",
        }
    } else {
        "signal_wifi_off"
    }
}

const H: f32 = metrics::MENUBAR_H as f32;
const STATUS_W: f32 = 26.0;

impl MenuBar {
    pub fn update(&mut self, _theme: &Theme, width: i32, scale: i32, clock: &str, info: &MenuBarInfo) {
        let key = format!(
            "{width}|{scale}|{}|{}|{}|{}|{clock}|{:?}|{:.2}|{:?}|{:?}",
            info.app_name, info.has_window, info.workspace, info.workspaces, info.open, info.luminance, info.status, info.recording
        ) + &info.tray.iter().map(|t| t.as_ref().map(|p| p.data().len() as u32 ^ p.data().iter().take(64).map(|b| *b as u32).sum::<u32>()).unwrap_or(0).to_string()).collect::<Vec<_>>().join(",");
        if self.key.as_deref() == Some(key.as_str()) && self.layer.is_set() {
            return;
        }
        self.key = Some(key);

        let (fg, dim, on_light) = Theme::bar_text(info.luminance);
        let highlight = if on_light { Rgba(0, 0, 0, 31) } else { Rgba(255, 255, 255, 56) };
        let mut c = Canvas::new(width, metrics::MENUBAR_H, scale as f32).unwrap();

        // 1ª passada: sombra (só para texto claro), desfocada; 2ª passada: conteúdo.
        if !on_light {
            let shadow = Rgba(0, 0, 0, 115);
            self.draw_content(&mut c, width as f32, clock, info, shadow, shadow, None, 1.0);
            crate::ui::canvas::blur_alpha_pub(&mut c.pm, (2.0 * scale as f32) as usize);
        }
        self.hits.clear();
        self.draw_content(&mut c, width as f32, clock, info, fg, dim, Some(highlight), 0.0);
        self.layer.set(&c);
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_content(
        &mut self,
        c: &mut Canvas,
        width: f32,
        clock: &str,
        info: &MenuBarInfo,
        fg: Rgba,
        dim: Rgba,
        highlight: Option<Rgba>,
        dy: f32,
    ) {
        let record = highlight.is_some();
        let mut hits: Vec<(BarItem, f32, f32)> = Vec::new();
        let pill = |c: &mut Canvas, item: BarItem, x0: f32, w: f32| {
            if let Some(h) = highlight {
                if info.open == Some(item) {
                    c.fill_rrect(x0, 4.0, w, H - 8.0, 12.0, h);
                }
            }
        };

        // ---- esquerda: logo, nome do app (negrito), Janela, Ir, mesas
        let mut x = 10.0;
        pill(c, BarItem::Logo, x, 34.0);
        text_glyph::obelisk_logo(c, x + 17.0, 16.0 + dy, 15.0, fg);
        hits.push((BarItem::Logo, x, x + 34.0));
        x += 34.0;

        let name: String = if info.app_name.chars().count() > 32 {
            info.app_name.chars().take(31).collect::<String>() + "…"
        } else {
            info.app_name.to_string()
        };
        let w = c.measure(&name, 13.0, Weight::Bold) + 20.0;
        pill(c, BarItem::App, x, w);
        c.text_vcenter(&name, x + 10.0, dy, H, 13.0, Weight::Bold, fg);
        hits.push((BarItem::App, x, x + w));
        x += w;

        let mut titles = Vec::new();
        // ordem do macOS: App · Arquivo · Editar · Visualizar · Ir · Janela · Ajuda
        if info.has_window {
            titles.push((BarItem::File, "Arquivo"));
            titles.push((BarItem::Edit, "Editar"));
            titles.push((BarItem::View, "Visualizar"));
        }
        titles.push((BarItem::Go, "Ir"));
        if info.has_window {
            titles.push((BarItem::Window, "Janela"));
        }
        titles.push((BarItem::Help, "Ajuda"));
        // o notch fica no meio: como no MacBook, o que não cabe antes dele pula para depois
        let notch_l = width as f32 / 2.0 - super::notch::CLOSED_W / 2.0 - 4.0;
        let notch_r = width as f32 / 2.0 + super::notch::CLOSED_W / 2.0 + 4.0;
        let past_notch = |x: f32, w: f32| if x < notch_r && x + w > notch_l { notch_r } else { x };
        // sem espaço antes dos ícones de status, os últimos menus somem (como no macOS)
        let status_w = 330.0 + info.tray.len() as f32 * 26.0;
        for (item, label) in titles {
            let w = c.measure(label, 13.0, Weight::Medium) + 20.0;
            x = past_notch(x, w);
            if x + w > width as f32 - status_w {
                break;
            }
            pill(c, item, x, w);
            c.text_vcenter(label, x + 10.0, dy, H, 13.0, Weight::Medium, fg);
            hits.push((item, x, x + w));
            x += w;
        }

        // pontos das mesas: atual vira pílula 18×6
        x += 10.0;
        x = past_notch(x, info.workspaces as f32 * 11.0 + 12.0);
        // sem espaço antes dos ícones de status (tela estreita): as mesas ficam só no menu Ir
        let room = x + info.workspaces as f32 * 11.0 + 12.0 <= width as f32 - status_w;
        for i in (0..info.workspaces).filter(|_| room) {
            let current = i == info.workspace;
            let dw = if current { 18.0 } else { 6.0 };
            c.fill_rrect(x, H / 2.0 - 3.0 + dy, dw, 6.0, 3.0, if current { fg } else { dim });
            hits.push((BarItem::Workspace(i), x - 3.0, x + dw + 3.0));
            x += dw + 5.0;
        }

        // ---- direita (da borda para dentro): relógio, CC, rede, bateria, som, bluetooth
        let mut rx = width - 4.0;
        let cw = c.measure(clock, 13.0, Weight::Medium) + 12.0;
        rx -= cw;
        pill(c, BarItem::Clock, rx + 1.0, cw - 2.0);
        c.text_vcenter(clock, rx + 6.0, dy, H, 13.0, Weight::Medium, fg);
        hits.push((BarItem::Clock, rx, rx + cw));

        rx -= STATUS_W;
        pill(c, BarItem::ControlCenter, rx + 1.0, STATUS_W - 2.0);
        cc_glyph(c, rx + (STATUS_W - 15.0) / 2.0, (H - 15.5) / 2.0 + dy, fg);
        hits.push((BarItem::ControlCenter, rx, rx + STATUS_W));

        let st = info.status;
        if st.wifi_available || st.wired {
            rx -= STATUS_W;
            pill(c, BarItem::Network, rx + 1.0, STATUS_W - 2.0);
            c.symbol(network_symbol(st), rx + STATUS_W / 2.0, H / 2.0 + dy, 17.0, fg);
            hits.push((BarItem::Network, rx, rx + STATUS_W));
        }
        if let Some((pct, charging)) = st.battery {
            let label = format!("{pct}%");
            let lw = c.measure(&label, 12.0, Weight::Medium);
            let w = lw + 6.0 + 27.0 + 12.0;
            rx -= w;
            pill(c, BarItem::Battery, rx + 1.0, w - 2.0);
            c.text_vcenter(&label, rx + 6.0, dy, H, 12.0, Weight::Medium, fg);
            battery_glyph(c, rx + 6.0 + lw + 6.0, (H - 13.0) / 2.0 + dy, pct, charging, fg, dim);
            hits.push((BarItem::Battery, rx, rx + w));
        }
        if st.volume.is_some() {
            rx -= STATUS_W;
            pill(c, BarItem::Sound, rx + 1.0, STATUS_W - 2.0);
            c.symbol(volume_symbol(st), rx + STATUS_W / 2.0, H / 2.0 + dy, 17.0, fg);
            hits.push((BarItem::Sound, rx, rx + STATUS_W));
        }
        // bandeja de apps (16 px, mais à esquerda dos ícones de status)
        for (i, icon) in info.tray.iter().enumerate().rev() {
            rx -= STATUS_W;
            pill(c, BarItem::Tray(i), rx + 1.0, STATUS_W - 2.0);
            if let Some(pm) = icon {
                if highlight.is_some() {
                    c.blit(pm, rx + (STATUS_W - 16.0) / 2.0, (H - 16.0) / 2.0, 16.0, 16.0, 1.0);
                }
            } else {
                c.fill_circle(rx + STATUS_W / 2.0, H / 2.0 + dy, 5.0, fg);
            }
            hits.push((BarItem::Tray(i), rx, rx + STATUS_W));
        }
        if let Some(secs) = info.recording {
            let t = format!("{}:{:02}", secs / 60, secs % 60);
            let tw = c.measure(&t, 12.0, Weight::Medium);
            let w = 17.0 + 5.0 + tw + 14.0;
            rx -= w;
            c.symbol("stop_circle", rx + 7.0 + 8.5, H / 2.0 + dy, 17.0, Rgba::rgb(0xff, 0x42, 0x45));
            c.text_vcenter(&t, rx + 7.0 + 17.0 + 5.0, dy, H, 12.0, Weight::Medium, fg);
            hits.push((BarItem::Recording, rx, rx + w));
        }
        if st.bt_available {
            rx -= STATUS_W;
            pill(c, BarItem::Bluetooth, rx + 1.0, STATUS_W - 2.0);
            let sym = if st.bt_on { "bluetooth" } else { "bluetooth_disabled" };
            c.symbol(sym, rx + STATUS_W / 2.0, H / 2.0 + dy, 17.0, fg);
            hits.push((BarItem::Bluetooth, rx, rx + STATUS_W));
        }

        if record {
            self.hits = hits;
        }
    }

    pub fn item_at(&self, x: f64) -> Option<BarItem> {
        self.hits.iter().find(|(_, a, b)| x as f32 >= *a && (x as f32) < *b).map(|(i, _, _)| *i)
    }

    pub fn item_x(&self, item: BarItem) -> Option<(f32, f32)> {
        self.hits.iter().find(|(i, _, _)| *i == item).map(|(_, a, b)| (*a, *b))
    }
}

/// Ícone da Central de Controle: duas cápsulas com botões (como no macOS).
fn cc_glyph(c: &mut Canvas, x: f32, y: f32, fg: Rgba) {
    for (row, knob_right) in [(0.0, true), (9.0, false)] {
        c.stroke_rrect(x, y + row, 15.0, 6.5, 3.25, 1.3, fg);
        let kx = if knob_right { x + 15.0 - 4.5 - 1.0 } else { x + 1.0 };
        c.fill_circle(kx + 2.25, y + row + 3.25, 2.25, fg);
    }
}

/// Bateria 27×13 (corpo 24×13, ponta à direita), preenchimento pelo nível.
fn battery_glyph(c: &mut Canvas, x: f32, y: f32, pct: u8, charging: bool, fg: Rgba, dim: Rgba) {
    c.stroke_rrect(x, y, 24.0, 13.0, 4.0, 1.0, dim);
    let level = pct as f32 / 100.0;
    let fill = if charging {
        Rgba::rgb(0x30, 0xd1, 0x58)
    } else if pct < 20 {
        Rgba::rgb(0xff, 0x42, 0x45)
    } else {
        fg
    };
    c.fill_rrect(x + 2.0, y + 2.0, (20.0 * level).max(2.0), 9.0, 2.0, fill);
    c.fill_rrect(x + 25.0, y + 4.5, 1.5, 4.0, 1.0, dim);
    if charging {
        c.symbol("bolt", x + 12.0, y + 6.5, 12.0, Rgba(255, 255, 255, 255));
    }
}
