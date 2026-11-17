//! Menus dos ícones da barra, como no macOS: Som (volume e saídas), Wi‑Fi/Rede (ligar, rede atual,
//! outras redes) e Bluetooth (ligar e aparelhos). Os dados vêm de uma thread (pactl, nmcli,
//! bluetoothctl) para o clique abrir na hora.

use resvg::tiny_skia::Pixmap;

use super::layer::Layer;
use crate::ui::canvas::Canvas;
use crate::ui::text::Weight;
use crate::ui::theme::{metrics, Rgba, Theme};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum StatusKind {
    Sound,
    Network,
    Bluetooth,
}

#[derive(Clone, Debug, PartialEq)]
pub enum StatusHit {
    Slider,
    Sink(usize),
    WifiToggle,
    Net(usize),
    BtToggle,
    Dev(usize),
    Settings,
}

/// Dados lidos do sistema para o menu.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct StatusData {
    /// saídas de som: (nome, descrição, padrão)
    pub sinks: Vec<(String, String, bool)>,
    /// redes Wi‑Fi: (ssid, sinal 0–100, protegida, conectada, salva)
    pub nets: Vec<(String, u32, bool, bool, bool)>,
    /// conexão por cabo ativa (nome)
    pub wired: Option<String>,
    /// aparelhos Bluetooth: (mac, nome, conectado, ícone, bateria)
    pub devices: Vec<(String, String, bool, String, String)>,
    pub loaded: bool,
}

const W: f32 = 300.0;
const ROW: f32 = 30.0;
const SHADOW: f32 = 30.0;

#[derive(Default)]
pub struct StatusMenu {
    pub open: Option<StatusKind>,
    pub data: StatusData,
    pub layer: Layer,
    pub pos: (f32, f32),
    pub size: (f32, f32),
    /// trilho do volume na tela (x, largura)
    pub slider: (f32, f32),
    pub dragging: bool,
    hits: Vec<(StatusHit, (f32, f32, f32, f32))>,
    key: Option<String>,
}

/// Lê o sistema para o menu (roda numa thread).
pub fn load(kind: StatusKind) -> StatusData {
    use crate::shell::settings_model::{sh, sh_t};
    let mut d = StatusData { loaded: true, ..Default::default() };
    match kind {
        StatusKind::Sound => {
            let def = sh("pactl get-default-sink").trim().to_string();
            let mut name = String::new();
            for l in sh("pactl list sinks").lines() {
                let t = l.trim();
                if let Some(n) = t.strip_prefix("Name: ") {
                    name = n.to_string();
                } else if let Some(desc) = t.strip_prefix("Description: ") {
                    d.sinks.push((name.clone(), desc.to_string(), name == def));
                }
            }
        }
        StatusKind::Network => {
            let fields = |l: &str| -> Vec<String> {
                let mut out = vec![String::new()];
                let mut esc = false;
                for c in l.chars() {
                    match (esc, c) {
                        (true, c) => {
                            out.last_mut().unwrap().push(c);
                            esc = false;
                        }
                        (false, '\\') => esc = true,
                        (false, ':') => out.push(String::new()),
                        (false, c) => out.last_mut().unwrap().push(c),
                    }
                }
                out
            };
            d.wired = sh("nmcli -t -f TYPE,STATE,CONNECTION dev").lines().map(fields).find(|f| f.len() >= 3 && f[0] == "ethernet" && f[1] == "connected").map(|f| f[2].clone());
            let known: Vec<String> = sh("nmcli -t -f NAME,TYPE con show").lines().map(fields).filter(|f| f.len() >= 2 && f[1].contains("wireless")).map(|f| f[0].clone()).collect();
            let mut seen = std::collections::HashSet::new();
            for l in sh_t("nmcli -t -f IN-USE,SSID,SIGNAL,SECURITY dev wifi list", 4000).lines() {
                let f = fields(l);
                if f.len() < 4 || f[1].is_empty() || !seen.insert(f[1].clone()) {
                    continue;
                }
                let k = known.contains(&f[1]);
                d.nets.push((f[1].clone(), f[2].parse().unwrap_or(0), !f[3].is_empty() && f[3] != "--", f[0] == "*", k));
            }
            d.nets.sort_by(|a, b| b.3.cmp(&a.3).then(b.4.cmp(&a.4)).then(b.1.cmp(&a.1)));
            d.nets.truncate(10);
        }
        StatusKind::Bluetooth => {
            for l in sh("bluetoothctl devices").lines() {
                let mut p = l.splitn(3, ' ');
                let (_, mac, name) = (p.next(), p.next().unwrap_or("").to_string(), p.next().unwrap_or("").to_string());
                let info = sh(&format!("bluetoothctl info {mac}"));
                let connected = info.contains("Connected: yes");
                if !info.contains("Paired: yes") && !connected {
                    continue;
                }
                let icon = info.lines().find_map(|l| l.trim().strip_prefix("Icon: ")).unwrap_or("").to_string();
                let bat = info.lines().find(|l| l.contains("Battery Percentage")).and_then(|l| l.rsplit('(').next()).map(|s| format!("{}%", s.trim_end_matches(')'))).unwrap_or_default();
                d.devices.push((mac, name, connected, icon, bat));
            }
            d.devices.sort_by(|a, b| b.2.cmp(&a.2));
        }
    }
    d
}

fn wifi_symbol(sig: u32) -> &'static str {
    if sig > 66 {
        "wifi"
    } else if sig > 33 {
        "wifi_2_bar"
    } else {
        "wifi_1_bar"
    }
}

fn device_symbol(icon: &str) -> &'static str {
    match icon {
        "audio-headset" | "audio-headphones" | "audio-card" => "headphones",
        "input-gaming" => "sports_esports",
        "input-keyboard" => "keyboard",
        "input-mouse" => "mouse",
        "phone" => "mobile",
        _ => "bluetooth",
    }
}

impl StatusMenu {
    pub fn contains(&self, x: f64, y: f64) -> bool {
        self.open.is_some() && (x as f32) >= self.pos.0 && (x as f32) < self.pos.0 + self.size.0 && (y as f32) >= self.pos.1 && (y as f32) < self.pos.1 + self.size.1
    }

    pub fn hit(&self, x: f64, y: f64) -> Option<StatusHit> {
        let (x, y) = (x as f32, y as f32);
        self.hits.iter().find(|(_, r)| x >= r.0 && x < r.0 + r.2 && y >= r.1 && y < r.1 + r.3).map(|(h, _)| h.clone())
    }

    pub fn slider_value(&self, x: f64) -> f32 {
        ((x as f32 - self.slider.0) / self.slider.1.max(1.0)).clamp(0.0, 1.0)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn update(&mut self, theme: &Theme, st: &crate::services::SysStatus, anchor_x: f32, sw: i32, sh: i32, scale: i32, backdrop: Option<&Pixmap>) {
        let Some(kind) = self.open else { return };
        let vol = if st.muted { 0.0 } else { st.volume.unwrap_or(0.0).min(1.0) };
        let key = format!("{kind:?}|{:?}|{vol:.2}|{}|{}|{}|{anchor_x}|{sw}|{scale}|{}", self.data, st.wifi_on, st.bt_on, theme.appearance as u8, theme.accent.0);
        // altura conforme o conteúdo
        let n = |k: usize| k.max(1) as f32 * ROW;
        let body = match kind {
            StatusKind::Sound => 40.0 + 24.0 + n(self.data.sinks.len()),
            StatusKind::Network => {
                (if self.data.wired.is_some() { ROW } else { 0.0 }) + if st.wifi_on { 8.0 + 24.0 + n(self.data.nets.len()) } else { ROW }
            }
            StatusKind::Bluetooth => 24.0 + n(self.data.devices.len()),
        };
        let h = 12.0 + 34.0 + body + 10.0 + ROW + 8.0;
        self.size = (W, h);
        self.pos = ((anchor_x - W + 40.0).clamp(6.0, sw as f32 - W - 6.0), metrics::MENUBAR_H as f32 + 4.0);
        let _ = sh;
        if self.key.as_deref() == Some(key.as_str()) && self.layer.is_set() {
            return;
        }
        self.key = Some(key);
        self.hits.clear();
        let (sx, sy) = (self.pos.0 - SHADOW, self.pos.1 - SHADOW);
        let to_screen = |r: (f32, f32, f32, f32)| (r.0 + sx, r.1 + sy, r.2, r.3);
        let mut c = Canvas::new((W + 2.0 * SHADOW) as i32, (h + 2.0 * SHADOW) as i32, scale as f32).unwrap();
        let (ox, oy) = (SHADOW, SHADOW);
        c.shadow_rrect(ox, oy + 8.0, W, h, metrics::MENU_RADIUS, 24.0, 90);
        if let Some(bd) = backdrop {
            c.fill_rrect_backdrop(ox, oy, W, h, metrics::MENU_RADIUS, bd, (sx, sy), (sw as f32, sh as f32));
        }
        c.fill_rrect(ox, oy, W, h, metrics::MENU_RADIUS, theme.menu_bg);
        c.stroke_rrect(ox, oy, W, h, metrics::MENU_RADIUS, 1.0, theme.menu_border);
        let (lx, lw) = (ox + 14.0, W - 28.0);
        let mut y = oy + 12.0;
        let switch = |c: &mut Canvas, x: f32, y: f32, on: bool| {
            c.fill_rrect(x, y, 34.0, 20.0, 10.0, if on { theme.accent } else { theme.fg(0.18) });
            c.fill_circle(if on { x + 24.0 } else { x + 10.0 }, y + 10.0, 8.5, Rgba::rgb(255, 255, 255));
        };
        let header = |c: &mut Canvas, t: &str, y: f32| {
            c.text_vcenter(t, lx, y, 24.0, 14.0, Weight::Bold, theme.label);
        };
        let section = |c: &mut Canvas, t: &str, y: f32| {
            c.text_vcenter(t, lx, y, 22.0, 11.0, Weight::Bold, theme.secondary);
        };
        let row_hl = |c: &mut Canvas, y: f32| {
            let _ = (c, y);
        };
        let _ = row_hl;
        let sep = |c: &mut Canvas, y: f32| c.fill_rrect(lx, y, lw, 1.0, 0.0, theme.separator);

        match kind {
            StatusKind::Sound => {
                header(&mut c, "Som", y);
                y += 34.0;
                // volume (arrastar ou clicar)
                let sym = if vol <= 0.001 { "volume_off" } else if vol < 0.5 { "volume_down" } else { "volume_up" };
                c.fill_rrect(lx, y, lw, 26.0, 13.0, theme.fg(0.10));
                c.fill_rrect(lx, y, (lw * vol).max(26.0), 26.0, 13.0, Rgba(255, 255, 255, 235));
                c.symbol(sym, lx + 14.0, y + 13.0, 16.0, Rgba(60, 60, 67, 255));
                self.slider = (lx + sx, lw);
                self.hits.push((StatusHit::Slider, to_screen((lx, y - 4.0, lw, 34.0))));
                y += 26.0 + 14.0;
                sep(&mut c, y - 6.0);
                section(&mut c, "Saída", y);
                y += 24.0;
                if !self.data.loaded {
                    c.text_vcenter("Carregando…", lx + 6.0, y, ROW, 13.0, Weight::Regular, theme.secondary);
                    y += ROW;
                }
                for (i, (_, desc, def)) in self.data.sinks.iter().enumerate() {
                    c.fill_circle(lx + 13.0, y + ROW / 2.0, 12.0, if *def { theme.accent } else { theme.fg(0.10) });
                    c.symbol("speaker", lx + 13.0, y + ROW / 2.0, 14.0, if *def { Rgba::rgb(255, 255, 255) } else { theme.label });
                    let t = c.ellipsized(desc, 13.0, Weight::Regular, lw - 40.0);
                    c.text_vcenter(&t, lx + 32.0, y, ROW, 13.0, Weight::Regular, theme.label);
                    self.hits.push((StatusHit::Sink(i), to_screen((lx - 6.0, y, lw + 12.0, ROW))));
                    y += ROW;
                }
            }
            StatusKind::Network => {
                // só cabo e nenhuma rede sem fio por perto: o menu é de "Rede"
                let wifi_title = if self.data.loaded && self.data.nets.is_empty() && self.data.wired.is_some() { "Rede" } else { "Wi‑Fi" };
                header(&mut c, wifi_title, y);
                switch(&mut c, lx + lw - 34.0, y + 2.0, st.wifi_on);
                self.hits.push((StatusHit::WifiToggle, to_screen((lx + lw - 44.0, y - 2.0, 54.0, 28.0))));
                y += 34.0;
                if let Some(w) = &self.data.wired {
                    c.fill_circle(lx + 13.0, y + ROW / 2.0, 12.0, theme.accent);
                    c.symbol("settings_ethernet", lx + 13.0, y + ROW / 2.0, 14.0, Rgba::rgb(255, 255, 255));
                    let t = c.ellipsized(&format!("Cabo: {w}"), 13.0, Weight::Semibold, lw - 40.0);
                    c.text_vcenter(&t, lx + 32.0, y, ROW, 13.0, Weight::Semibold, theme.label);
                    y += ROW;
                }
                if st.wifi_on {
                    sep(&mut c, y + 4.0);
                    y += 8.0;
                    section(&mut c, if self.data.nets.iter().any(|n| n.3) { "Rede" } else { "Redes" }, y);
                    y += 24.0;
                    if !self.data.loaded {
                        c.text_vcenter("Procurando redes…", lx + 6.0, y, ROW, 13.0, Weight::Regular, theme.secondary);
                        y += ROW;
                    } else if self.data.nets.is_empty() {
                        c.text_vcenter("Nenhuma rede encontrada", lx + 6.0, y, ROW, 13.0, Weight::Regular, theme.secondary);
                        y += ROW;
                    }
                    for (i, (ssid, sig, secure, active, _)) in self.data.nets.iter().enumerate() {
                        c.fill_circle(lx + 13.0, y + ROW / 2.0, 12.0, if *active { theme.accent } else { theme.fg(0.10) });
                        c.symbol(wifi_symbol(*sig), lx + 13.0, y + ROW / 2.0, 14.0, if *active { Rgba::rgb(255, 255, 255) } else { theme.label });
                        let t = c.ellipsized(ssid, 13.0, if *active { Weight::Semibold } else { Weight::Regular }, lw - 60.0);
                        c.text_vcenter(&t, lx + 32.0, y, ROW, 13.0, if *active { Weight::Semibold } else { Weight::Regular }, theme.label);
                        if *secure {
                            c.symbol("lock", lx + lw - 8.0, y + ROW / 2.0, 13.0, theme.secondary);
                        }
                        self.hits.push((StatusHit::Net(i), to_screen((lx - 6.0, y, lw + 12.0, ROW))));
                        y += ROW;
                    }
                } else {
                    y += ROW;
                }
            }
            StatusKind::Bluetooth => {
                header(&mut c, "Bluetooth", y);
                switch(&mut c, lx + lw - 34.0, y + 2.0, st.bt_on);
                self.hits.push((StatusHit::BtToggle, to_screen((lx + lw - 44.0, y - 2.0, 54.0, 28.0))));
                y += 34.0;
                section(&mut c, "Aparelhos", y);
                y += 24.0;
                if !self.data.loaded {
                    c.text_vcenter("Carregando…", lx + 6.0, y, ROW, 13.0, Weight::Regular, theme.secondary);
                    y += ROW;
                } else if self.data.devices.is_empty() {
                    c.text_vcenter("Nenhum aparelho pareado", lx + 6.0, y, ROW, 13.0, Weight::Regular, theme.secondary);
                    y += ROW;
                }
                for (i, (_, name, conn, icon, bat)) in self.data.devices.iter().enumerate() {
                    c.fill_circle(lx + 13.0, y + ROW / 2.0, 12.0, if *conn { theme.accent } else { theme.fg(0.10) });
                    c.symbol(device_symbol(icon), lx + 13.0, y + ROW / 2.0, 14.0, if *conn { Rgba::rgb(255, 255, 255) } else { theme.label });
                    let t = c.ellipsized(name, 13.0, Weight::Regular, lw - 80.0);
                    c.text_vcenter(&t, lx + 32.0, y, ROW, 13.0, Weight::Regular, theme.label);
                    if !bat.is_empty() {
                        let bw = c.measure(bat, 12.0, Weight::Regular);
                        c.text_vcenter(bat, lx + lw - bw, y, ROW, 12.0, Weight::Regular, theme.secondary);
                    }
                    self.hits.push((StatusHit::Dev(i), to_screen((lx - 6.0, y, lw + 12.0, ROW))));
                    y += ROW;
                }
            }
        }
        // rodapé: atalho para os Ajustes
        sep(&mut c, y + 4.0);
        y += 10.0;
        let label = match kind {
            StatusKind::Sound => "Ajustes de Som…",
            StatusKind::Network => "Ajustes de Rede…",
            StatusKind::Bluetooth => "Ajustes de Bluetooth…",
        };
        c.text_vcenter(label, lx, y, ROW, 13.0, Weight::Regular, theme.label);
        self.hits.push((StatusHit::Settings, to_screen((lx - 6.0, y, lw + 12.0, ROW))));
        self.layer.set(&c);
    }
}
