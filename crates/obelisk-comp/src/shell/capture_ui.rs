//! Capturas estilo macOS: ⇧⌘4 (área, com mira e tamanho), ⇧⌘5 (barra: tela, janela, área, timer).
//! O recorte sai do framebuffer do próprio compositor.

use std::time::Instant;

use resvg::tiny_skia::Pixmap;

use super::layer::Layer;
use crate::ui::canvas::Canvas;
use crate::ui::text::Weight;
use crate::ui::theme::{Rgba, Theme};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CaptureMode {
    Screen,
    Window,
    Area,
    RecordScreen,
    RecordArea,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BarHit {
    Close,
    Mode(CaptureMode),
    Timer,
    Capture,
}

pub struct CaptureUi {
    pub active: bool,
    pub bar: bool,
    pub mode: CaptureMode,
    pub timer: u32,
    pub drag_start: Option<(f64, f64)>,
    pub cursor: (f64, f64),
    /// janela destacada no modo Janela: retângulo (tela)
    pub hover_rect: Option<(f32, f32, f32, f32)>,
    pub countdown: Option<(Instant, u32, (f32, f32, f32, f32))>,
    pub layer: Layer,
    bar_hits: Vec<(BarHit, (f32, f32, f32, f32))>,
    key: Option<String>,
}

impl Default for CaptureUi {
    fn default() -> Self {
        CaptureUi {
            active: false,
            bar: false,
            mode: CaptureMode::Area,
            timer: 0,
            drag_start: None,
            cursor: (0.0, 0.0),
            hover_rect: None,
            countdown: None,
            layer: Layer::default(),
            bar_hits: vec![],
            key: None,
        }
    }
}

impl CaptureUi {
    pub fn start_area(&mut self) {
        self.active = true;
        self.bar = false;
        self.mode = CaptureMode::Area;
        self.drag_start = None;
        self.key = None;
    }

    pub fn start_bar(&mut self) {
        self.active = true;
        self.bar = true;
        self.drag_start = None;
        self.key = None;
    }

    pub fn close(&mut self) {
        self.active = false;
        self.bar = false;
        self.drag_start = None;
        self.hover_rect = None;
    }

    /// Retângulo selecionado (x, y, w, h) a partir do arrasto.
    pub fn selection(&self) -> Option<(f32, f32, f32, f32)> {
        let (sx, sy) = self.drag_start?;
        let (cx, cy) = self.cursor;
        let (x0, y0, x1, y1) = (sx.min(cx), sy.min(cy), sx.max(cx), sy.max(cy));
        Some((x0 as f32, y0 as f32, (x1 - x0) as f32, (y1 - y0) as f32))
    }

    pub fn bar_hit(&self, x: f64, y: f64) -> Option<BarHit> {
        if !self.bar {
            return None;
        }
        let (x, y) = (x as f32, y as f32);
        self.bar_hits.iter().find(|(_, r)| x >= r.0 && x < r.0 + r.2 && y >= r.1 && y < r.1 + r.3).map(|(h, _)| *h)
    }

    pub fn countdown_left(&self) -> Option<u32> {
        let (t0, secs, _) = self.countdown?;
        let e = t0.elapsed().as_secs_f32();
        (e < secs as f32).then(|| (secs as f32 - e).ceil() as u32)
    }

    pub fn update(&mut self, theme: &Theme, sw: i32, sh: i32, scale: i32, backdrop: Option<&Pixmap>) {
        let key = format!(
            "{}|{}|{:?}|{}|{:?}|{:?}|{:?}|{:?}|{sw}|{sh}",
            self.active, self.bar, self.mode, self.timer, self.selection().map(|r| (r.0 as i32, r.1 as i32, r.2 as i32, r.3 as i32)),
            self.hover_rect.map(|r| r.0 as i32), self.countdown_left(), (self.cursor.0 as i32 / 4, self.cursor.1 as i32 / 4)
        );
        if self.key.as_deref() == Some(key.as_str()) && self.layer.is_set() {
            return;
        }
        self.key = Some(key);
        let (w, h) = (sw as f32, sh as f32);
        let mut c = Canvas::new(sw, sh, scale as f32).unwrap();
        self.bar_hits.clear();
        let white = |o: f32| Rgba(255, 255, 255, (255.0 * o) as u8);

        if let Some(left) = self.countdown_left() {
            // pílula da contagem no topo (não toma o teclado)
            let t = format!("{left}");
            c.fill_rrect(w / 2.0 - 30.0, 40.0, 60.0, 36.0, 18.0, Rgba(0, 0, 0, 170));
            let tw = c.measure(&t, 18.0, Weight::Bold);
            c.text_vcenter(&t, w / 2.0 - tw / 2.0, 40.0, 36.0, 18.0, Weight::Bold, white(1.0));
            self.layer.set(&c);
            return;
        }

        match (self.mode, self.selection()) {
            (CaptureMode::Area | CaptureMode::RecordArea, Some((x, y, sw_, sh_))) => {
                // escurece fora da seleção
                let dim = Rgba(0, 0, 0, 90);
                c.fill_rrect(0.0, 0.0, w, y, 0.0, dim);
                c.fill_rrect(0.0, y + sh_, w, h - y - sh_, 0.0, dim);
                c.fill_rrect(0.0, y, x, sh_, 0.0, dim);
                c.fill_rrect(x + sw_, y, w - x - sw_, sh_, 0.0, dim);
                c.stroke_rrect(x, y, sw_.max(1.0), sh_.max(1.0), 0.0, 1.0, white(0.9));
                let label = format!("{} × {}", (sw_ * scale as f32) as i32, (sh_ * scale as f32) as i32);
                let lw = c.measure(&label, 11.0, Weight::Semibold) + 12.0;
                c.fill_rrect(x + sw_ + 6.0, y + sh_ + 6.0, lw, 20.0, 6.0, Rgba(0, 0, 0, 160));
                c.text_vcenter(&label, x + sw_ + 12.0, y + sh_ + 6.0, 20.0, 11.0, Weight::Semibold, white(1.0));
            }
            (CaptureMode::Area | CaptureMode::RecordArea, None) => {
                // mira com coordenadas (como no macOS)
                let (cx, cy) = (self.cursor.0 as f32, self.cursor.1 as f32);
                c.fill_rrect(cx - 10.0, cy - 0.5, 20.0, 1.0, 0.0, white(0.95));
                c.fill_rrect(cx - 0.5, cy - 10.0, 1.0, 20.0, 0.0, white(0.95));
                let label = format!("{}  {}", (cx * scale as f32) as i32, (cy * scale as f32) as i32);
                c.text(&label, cx + 8.0, cy + 6.0, 11.0, Weight::Semibold, white(0.95));
            }
            (CaptureMode::Window, _) => {
                c.fill_rrect(0.0, 0.0, w, h, 0.0, Rgba(0, 0, 0, 40));
                if let Some((x, y, rw, rh)) = self.hover_rect {
                    c.fill_rrect(x, y, rw, rh, 12.0, Rgba(0x00, 0x91, 0xff, 70));
                }
            }
            (CaptureMode::Screen | CaptureMode::RecordScreen, _) => {}
        }

        if self.bar {
            // barra ⇧⌘5: fechar | tela, janela, área | timer | Capturar
            let modes = [
                (CaptureMode::Screen, "screenshot_monitor", "Tela"),
                (CaptureMode::Window, "web_asset", "Janela"),
                (CaptureMode::Area, "screenshot_region", "Área"),
                (CaptureMode::RecordScreen, "screen_record", "Gravar tela"),
                (CaptureMode::RecordArea, "select", "Gravar área"),
            ];
            let bw = 42.0 * 6.0 + 5.0 * 4.0 + 1.0 + 8.0 + 92.0 + 8.0 + 92.0 + 34.0;
            let (bx, by) = ((w - bw) / 2.0, h - 110.0 - 52.0);
            c.shadow_rrect_add(bx, by + 6.0, bw, 52.0, 14.0, 26.0, 128);
            if let Some(bd) = backdrop {
                c.fill_rrect_backdrop(bx, by, bw, 52.0, 14.0, bd, (0.0, 0.0), (w, h));
            }
            c.fill_rrect(bx, by, bw, 52.0, 14.0, theme.menu_bg);
            c.stroke_rrect(bx, by, bw, 52.0, 14.0, 1.0, theme.fg(0.12));
            let mut x = bx + 12.0;
            let y = by + 8.0;
            c.symbol("cancel", x + 21.0, y + 18.0, 22.0, theme.secondary);
            self.bar_hits.push((BarHit::Close, (x, y, 42.0, 36.0)));
            x += 46.0;
            c.fill_rrect(x, y + 5.0, 1.0, 26.0, 0.0, theme.fg(0.16));
            x += 5.0;
            for (i, (m, sym, _label)) in modes.into_iter().enumerate() {
                if i == 3 {
                    c.fill_rrect(x, y + 5.0, 1.0, 26.0, 0.0, theme.fg(0.16));
                    x += 5.0;
                }
                if m == self.mode {
                    c.fill_rrect(x, y, 42.0, 36.0, 8.0, theme.fg(0.20));
                }
                c.symbol(sym, x + 21.0, y + 18.0, 22.0, theme.label);
                if m == CaptureMode::RecordArea {
                    c.fill_circle(x + 31.0, y + 9.0, 3.5, theme.red);
                }
                self.bar_hits.push((BarHit::Mode(m), (x, y, 42.0, 36.0)));
                x += 46.0;
            }
            x += 4.0;
            let timer = if self.timer == 0 { "Sem timer".to_string() } else { format!("Timer {} s", self.timer) };
            c.fill_rrect(x, y, 92.0, 36.0, 8.0, theme.fg(0.10));
            let tw = c.measure(&timer, 12.0, Weight::Semibold);
            c.text_vcenter(&timer, x + (92.0 - tw) / 2.0, y, 36.0, 12.0, Weight::Semibold, theme.label);
            self.bar_hits.push((BarHit::Timer, (x, y, 92.0, 36.0)));
            x += 100.0;
            c.fill_rrect(x, y + 2.0, 92.0, 32.0, 8.0, theme.accent);
            let action = if matches!(self.mode, CaptureMode::RecordScreen | CaptureMode::RecordArea) { "Gravar" } else { "Capturar" };
            let cw = c.measure(action, 13.0, Weight::Semibold);
            c.text_vcenter(action, x + (92.0 - cw) / 2.0, y + 2.0, 32.0, 13.0, Weight::Semibold, theme.text_on_accent);
            self.bar_hits.push((BarHit::Capture, (x, y + 2.0, 92.0, 32.0)));
        }
        self.layer.set(&c);
    }
}
