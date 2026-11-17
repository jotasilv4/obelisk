//! Central de Controle (docs/03-especificacao-kyro.md §10): 320 px, topo 35, 6 px da borda
//! direita, raio 22, módulos de raio 18, sliders brancos estilo macOS.

use std::time::Instant;

use resvg::tiny_skia::Pixmap;

use super::layer::Layer;
use crate::services::SysStatus;
use crate::ui::canvas::Canvas;
use crate::ui::text::Weight;
use crate::ui::theme::{Rgba, Theme};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CcHit {
    Wifi,
    Bluetooth,
    Focus,
    Appearance,
    GameMode,
    Tile(usize),
    Brightness(f32),
    Volume(f32),
    Mute,
    MediaPrev,
    MediaPlay,
    MediaNext,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Slider {
    Brightness,
    Volume,
}

pub const WIDTH: f32 = 320.0;
const PAD: f32 = 12.0;
const GAP: f32 = 10.0;
const SHADOW: f32 = 28.0;
pub const TILES: [(&str, &str); 4] = [("Captura", "screenshot_region"), ("Mesas", "space_dashboard"), ("Aparência", "dark_mode"), ("Bloquear", "lock")];

#[derive(Default)]
pub struct ControlCenter {
    pub open: bool,
    pub opened: Option<Instant>,
    pub focus_on: bool,
    pub game_on: bool,
    pub dragging: Option<Slider>,
    pub layer: Layer,
    /// Canto superior esquerdo do painel (tela).
    pub pos: (f32, f32),
    pub size: (f32, f32),
    hits: Vec<(CcHit, (f32, f32, f32, f32))>,
    sliders: Vec<(Slider, (f32, f32, f32, f32))>,
    key: Option<String>,
    /// capa do álbum (arquivo, imagem)
    art: Option<(String, Pixmap)>,
}

impl ControlCenter {
    pub fn toggle(&mut self) {
        self.open = !self.open;
        self.opened = self.open.then(Instant::now);
        self.dragging = None;
        self.key = None;
    }

    pub fn contains(&self, x: f64, y: f64) -> bool {
        let (px, py) = self.pos;
        let (w, h) = self.size;
        self.open && (x as f32) >= px && (x as f32) < px + w && (y as f32) >= py && (y as f32) < py + h
    }

    pub fn hit(&self, x: f64, y: f64) -> Option<CcHit> {
        let (x, y) = (x as f32, y as f32);
        let inside = |r: &(f32, f32, f32, f32)| x >= r.0 && x < r.0 + r.2 && y >= r.1 && y < r.1 + r.3;
        for (s, r) in &self.sliders {
            if inside(r) {
                let v = self.slider_value(*s, x);
                // clicar no glifo do som (início do trilho) alterna o mudo
                if *s == Slider::Volume && x < r.0 + 26.0 {
                    return Some(CcHit::Mute);
                }
                return Some(match s {
                    Slider::Brightness => CcHit::Brightness(v),
                    Slider::Volume => CcHit::Volume(v),
                });
            }
        }
        self.hits.iter().find(|(_, r)| inside(r)).map(|(h, _)| *h)
    }

    pub fn slider_value(&self, s: Slider, x: f32) -> f32 {
        self.sliders
            .iter()
            .find(|(k, _)| *k == s)
            .map(|(_, r)| ((x - r.0 + 11.0) / r.2).clamp(0.0, 1.0))
            .unwrap_or(0.0)
    }

    pub fn buf_pos(&self) -> (i32, i32) {
        ((self.pos.0 - SHADOW) as i32, (self.pos.1 - SHADOW) as i32)
    }

    pub fn alpha(&self) -> f32 {
        self.opened.map(|o| (o.elapsed().as_secs_f32() / 0.11).min(1.0)).unwrap_or(1.0)
    }

    pub fn update(&mut self, theme: &Theme, st: &SysStatus, sw: i32, sh: i32, scale: i32, backdrop: Option<&Pixmap>) {
        let key = format!("{st:?}|{}|{}|{}|{}|{}", self.focus_on, self.game_on, theme.appearance as u8, sw, scale);
        let col_w = (WIDTH - 2.0 * PAD - GAP) / 2.0;
        let has_bright = st.brightness.is_some();
        let has_media = st.media.is_some();
        let mut h = PAD + 128.0 + GAP + 62.0 + GAP + 64.0 + PAD;
        if has_bright {
            h += 64.0 + GAP;
        }
        if has_media {
            h += 64.0 + GAP;
        }
        self.size = (WIDTH, h);
        self.pos = (sw as f32 - 6.0 - WIDTH, 35.0);
        if self.key.as_deref() == Some(key.as_str()) && self.layer.is_set() {
            return;
        }
        self.key = Some(key);
        self.hits.clear();
        self.sliders.clear();

        let mut c = Canvas::new((WIDTH + 2.0 * SHADOW) as i32, (h + 2.0 * SHADOW) as i32, scale as f32).unwrap();
        let (ox, oy) = (SHADOW, SHADOW);
        let (sx, sy) = (self.pos.0 - SHADOW, self.pos.1 - SHADOW); // origem do canvas na tela
        let to_screen = |r: (f32, f32, f32, f32)| (r.0 + sx, r.1 + sy, r.2, r.3);
        c.shadow_rrect(ox, oy + 8.0, WIDTH, h, 22.0, 22.0, 115);
        if let Some(bd) = backdrop {
            c.fill_rrect_backdrop(ox, oy, WIDTH, h, 22.0, bd, (sx, sy), (sw as f32, sh as f32));
        }
        let _ = sh;
        c.fill_rrect(ox, oy, WIDTH, h, 22.0, theme.menu_bg);
        c.stroke_rrect(ox, oy, WIDTH, h, 22.0, 1.0, theme.menu_border);

        let module = |c: &mut Canvas, x: f32, y: f32, w: f32, h: f32| {
            c.fill_rrect(x, y, w, h, 18.0, theme.fg(0.08));
            c.stroke_rrect(x, y, w, h, 18.0, 1.0, theme.fg(0.05));
        };
        let toggle = |c: &mut Canvas, x: f32, y: f32, sym: &str, title: &str, sub: &str, on: bool, color: Rgba, maxw: f32| {
            c.fill_circle(x + 16.0, y + 17.0, 16.0, if on { color } else { theme.fg(0.12) });
            c.symbol(sym, x + 16.0, y + 17.0, 18.0, if on { Rgba::rgb(255, 255, 255) } else { theme.label });
            let t = c.ellipsized(title, 13.0, Weight::Semibold, maxw - 41.0);
            c.text(&t, x + 41.0, y + 1.0, 13.0, Weight::Semibold, theme.label);
            let s = c.ellipsized(sub, 11.0, Weight::Regular, maxw - 41.0);
            c.text(&s, x + 41.0, y + 18.0, 11.0, Weight::Regular, theme.secondary);
        };

        // ---- linha 1
        let mut y = oy + PAD;
        let lx = ox + PAD;
        let rx = lx + col_w + GAP;
        module(&mut c, lx, y, col_w, 128.0);
        let wifi_sub = if !st.wifi_on {
            "Desligado".to_string()
        } else {
            st.ssid.clone().unwrap_or_else(|| if st.wired { "Cabo conectado".into() } else { "Sem conexão".into() })
        };
        toggle(&mut c, lx + 12.0, y + 14.0, if st.wifi_on { "wifi" } else { "wifi_off" }, "Wi‑Fi", &wifi_sub, st.wifi_on, theme.accent, col_w - 24.0);
        self.hits.push((CcHit::Wifi, to_screen((lx, y, col_w, 64.0))));
        let bt_sub = if !st.bt_available { "Indisponível" } else if st.bt_on { "Ligado" } else { "Desligado" };
        toggle(&mut c, lx + 12.0, y + 14.0 + 34.0 + 16.0, "bluetooth", "Bluetooth", bt_sub, st.bt_on, theme.accent, col_w - 24.0);
        self.hits.push((CcHit::Bluetooth, to_screen((lx, y + 64.0, col_w, 64.0))));

        module(&mut c, rx, y, col_w, 59.0);
        toggle(&mut c, rx + 12.0, y + 12.0, "bedtime", "Foco", if self.focus_on { "Não Perturbe" } else { "Desligado" }, self.focus_on, theme.purple, col_w - 24.0);
        self.hits.push((CcHit::Focus, to_screen((rx, y, col_w, 59.0))));
        module(&mut c, rx, y + 69.0, col_w, 59.0);
        let gsub = if self.game_on { "Ativo" } else { "Desligado" };
        toggle(&mut c, rx + 12.0, y + 69.0 + 12.0, "sports_esports", "Modo Jogo", gsub, self.game_on, theme.green, col_w - 24.0);
        self.hits.push((CcHit::GameMode, to_screen((rx, y + 69.0, col_w, 59.0))));
        y += 128.0 + GAP;

        // ---- tiles rápidos
        let tw = (WIDTH - 2.0 * PAD - 3.0 * GAP) / 4.0;
        for (i, (label, sym)) in TILES.iter().enumerate() {
            let tx = lx + i as f32 * (tw + GAP);
            let enabled = true;
            let a = if enabled { 1.0 } else { 0.4 };
            let fade = |c: Rgba| Rgba(c.0, c.1, c.2, (c.3 as f32 * a) as u8);
            c.fill_rrect(tx, y, tw, 62.0, 18.0, fade(theme.fg(0.08)));
            c.symbol(sym, tx + tw / 2.0, y + 23.0, 20.0, fade(theme.label));
            let lw = c.measure(label, 11.0, Weight::Medium);
            c.text(label, tx + (tw - lw) / 2.0, y + 38.0, 11.0, Weight::Medium, fade(theme.label));
            if enabled {
                self.hits.push((CcHit::Tile(i), to_screen((tx, y, tw, 62.0))));
            }
        }
        y += 62.0 + GAP;

        // ---- sliders
        let mut slider = |c: &mut Canvas, y: f32, title: &str, sym: &str, value: f32, kind: Slider| {
            module(c, lx, y, WIDTH - 2.0 * PAD, 64.0);
            c.text(title, lx + 14.0, y + 9.0, 13.0, Weight::Semibold, theme.label);
            let (trx, try_, trw) = (lx + 12.0, y + 64.0 - 12.0 - 22.0, WIDTH - 2.0 * PAD - 24.0);
            c.fill_rrect(trx, try_, trw, 22.0, 11.0, theme.fg(0.14));
            let fw = (trw * value).max(22.0);
            c.fill_rrect(trx, try_, fw, 22.0, 11.0, Rgba::rgb(255, 255, 255));
            c.symbol(sym, trx + 4.0 + 7.5, try_ + 11.0, 15.0, Rgba::rgb(0x7a, 0x7a, 0x7e));
            self.sliders.push((kind, to_screen((trx, try_ - 6.0, trw, 34.0))));
        };
        if let Some(b) = st.brightness {
            slider(&mut c, y, "Tela", "light_mode", b, Slider::Brightness);
            y += 64.0 + GAP;
        }
        let vol = if st.muted { 0.0 } else { st.volume.unwrap_or(0.0) };
        slider(&mut c, y, "Som", crate::shell::menubar::volume_symbol(st), vol.min(1.0), Slider::Volume);
        y += 64.0 + GAP;

        // ---- tocando agora
        if let Some(m) = &st.media {
            module(&mut c, lx, y, WIDTH - 2.0 * PAD, 64.0);
            if self.art.as_ref().map(|a| a.0.as_str()) != Some(m.art.as_str()) {
                self.art = (!m.art.is_empty()).then(|| crate::settings_ctl::load_image(&m.art, 128).map(|pm| (m.art.clone(), pm))).flatten();
            }
            match &self.art {
                Some((_, pm)) => {
                    c.shadow_rrect_add(lx + 10.0, y + 11.0, 44.0, 44.0, 8.0, 6.0, 60);
                    c.blit_rounded(pm, lx + 10.0, y + 10.0, 44.0, 44.0, 8.0, 1.0);
                }
                None => {
                    c.fill_rrect(lx + 10.0, y + 10.0, 44.0, 44.0, 8.0, theme.fg(0.12));
                    c.symbol("music_note", lx + 32.0, y + 32.0, 22.0, theme.secondary);
                }
            }
            let tmax = WIDTH - 2.0 * PAD - 64.0 - 90.0;
            let t = c.ellipsized(&m.title, 13.0, Weight::Semibold, tmax);
            c.text(&t, lx + 64.0, y + 13.0, 13.0, Weight::Semibold, theme.label);
            let a = c.ellipsized(&m.artist, 12.0, Weight::Regular, tmax);
            c.text(&a, lx + 64.0, y + 32.0, 12.0, Weight::Regular, theme.secondary);
            let bx = lx + WIDTH - 2.0 * PAD - 90.0;
            for (i, (sym, hit)) in [("skip_previous", CcHit::MediaPrev), (if m.playing { "pause" } else { "play_arrow" }, CcHit::MediaPlay), ("skip_next", CcHit::MediaNext)].iter().enumerate() {
                let cx = bx + 15.0 + i as f32 * 28.0;
                c.symbol(sym, cx, y + 32.0, if i == 1 { 24.0 } else { 22.0 }, theme.label);
                self.hits.push((*hit, to_screen((cx - 14.0, y + 18.0, 28.0, 28.0))));
            }
        }
        self.layer.set(&c);
    }
}
