//! Mission Control (docs/03-especificacao-kyro.md §8). O fundo (véu, barra de mesas e
//! legendas) é desenhado aqui; as janelas são os próprios buffers dos apps, reescalados
//! na GPU pelo render (miniaturas reais e ao vivo).

use std::time::Instant;

use resvg::tiny_skia::Pixmap;

use super::layer::Layer;
use crate::ui::canvas::Canvas;
use crate::ui::icons::IconStore;
use crate::ui::text::Weight;
use crate::ui::theme::{Rgba, Theme};

/// Janela no Mission Control: geometria real (tela) e destino na grade.
#[derive(Clone, Debug)]
pub struct McWindow {
    pub real: (f32, f32, f32, f32),
    pub slot: (f32, f32, f32, f32),
    pub title: String,
    pub icon: String,
}

#[derive(Default)]
pub struct MissionControl {
    pub open: bool,
    pub opened: Option<Instant>,
    pub hover: Option<usize>,
    pub windows: Vec<McWindow>,
    pub layer: Layer,
    /// (índice da mesa ou `None` = "+", retângulo na tela)
    pub spaces: Vec<(Option<usize>, (f32, f32, f32, f32))>,
    key: Option<String>,
}

const TOP: f32 = 170.0;
const SIDE: f32 = 60.0;

/// Grade do kyro: cols = ceil(√(n·aw/ah / 1,5)), escala ≤ 1, última linha centralizada.
pub fn layout(sizes: &[(f32, f32)], sw: f32, sh: f32) -> Vec<(f32, f32, f32, f32)> {
    let n = sizes.len();
    if n == 0 {
        return vec![];
    }
    let (aw, ah) = (sw - 2.0 * SIDE, sh - TOP - 60.0);
    let cols = ((n as f32 * (aw / ah) / 1.5).sqrt().ceil() as usize).max(1);
    let rows = n.div_ceil(cols);
    let (cw, ch) = (aw / cols as f32, ah / rows as f32);
    sizes
        .iter()
        .enumerate()
        .map(|(i, (w, h))| {
            let (row, col) = (i / cols, i % cols);
            let in_row = if row == rows - 1 { n - row * cols } else { cols };
            let row_off = (aw - in_row as f32 * cw) / 2.0;
            let s = ((cw - 40.0) / w).min((ch - 50.0) / h).min(1.0);
            let x = SIDE + row_off + col as f32 * cw + (cw - w * s) / 2.0;
            let y = TOP + row as f32 * ch + (ch - 30.0 - h * s) / 2.0;
            (x, y, w * s, h * s)
        })
        .collect()
}

impl MissionControl {
    pub fn toggle(&mut self) {
        self.open = !self.open;
        self.opened = self.open.then(Instant::now);
        self.hover = None;
        self.key = None;
    }

    /// Progresso da animação 0→1 (340 ms, OutCubic).
    pub fn progress(&self) -> f32 {
        let t = self.opened.map(|o| o.elapsed().as_secs_f32() / 0.34).unwrap_or(1.0).min(1.0);
        1.0 - (1.0 - t).powi(3)
    }

    /// Retângulo atual (interpolado) da janela i.
    pub fn rect(&self, i: usize) -> (f32, f32, f32, f32) {
        let p = self.progress();
        let w = &self.windows[i];
        let l = |a: f32, b: f32| a + (b - a) * p;
        (l(w.real.0, w.slot.0), l(w.real.1, w.slot.1), l(w.real.2, w.slot.2), l(w.real.3, w.slot.3))
    }

    pub fn window_at(&self, x: f64, y: f64) -> Option<usize> {
        (0..self.windows.len()).rev().find(|&i| {
            let (rx, ry, rw, rh) = self.rect(i);
            (x as f32) >= rx && (x as f32) < rx + rw && (y as f32) >= ry && (y as f32) < ry + rh + 30.0
        })
    }

    pub fn space_at(&self, x: f64, y: f64) -> Option<Option<usize>> {
        self.spaces
            .iter()
            .find(|(_, r)| (x as f32) >= r.0 && (x as f32) < r.0 + r.2 && (y as f32) >= r.1 && (y as f32) < r.1 + r.3)
            .map(|(s, _)| *s)
    }

    /// `ws_windows[i]` = contornos (x,y,w,h em tela) das janelas da mesa i.
    #[allow(clippy::too_many_arguments)]
    pub fn update(
        &mut self,
        theme: &Theme,
        icons: &mut IconStore,
        sw: i32,
        sh: i32,
        scale: i32,
        backdrop: Option<&Pixmap>,
        current: usize,
        ws_windows: &[Vec<(f32, f32, f32, f32)>],
    ) {
        let p = self.progress();
        let key = format!("{:?}|{}|{}|{}|{:.3}|{:?}|{}", self.hover, current, sw, sh, p, ws_windows, self.windows.len());
        if self.key.as_deref() == Some(key.as_str()) && self.layer.is_set() {
            return;
        }
        self.key = Some(key);
        let (w, h) = (sw as f32, sh as f32);
        let mut c = Canvas::new(sw, sh, scale as f32).unwrap();
        if let Some(bd) = backdrop {
            c.fill_rrect_backdrop(0.0, 0.0, w, h, 0.0, bd, (0.0, 0.0), (w, h));
        }
        c.fill_rrect(0.0, 0.0, w, h, 0.0, Rgba(0, 0, 0, (115.0 * p) as u8));

        // ---- barra de mesas (desce de -140 até 24)
        let bar_y = -140.0 + (24.0 + 140.0) * p;
        let n = ws_windows.len();
        let total = n as f32 * 160.0 + 60.0 + n as f32 * 16.0;
        let mut x = (w - total) / 2.0;
        self.spaces.clear();
        for (i, wins) in ws_windows.iter().enumerate() {
            let cur = i == current;
            if let Some(bd) = backdrop {
                c.fill_rrect_backdrop(x, bar_y, 160.0, 100.0, 10.0, bd, (0.0, 0.0), (w, h));
            }
            for r in wins {
                let (ox, oy, ow, oh) = (x + r.0 / w * 160.0, bar_y + r.1 / h * 100.0, r.2 / w * 160.0, r.3 / h * 100.0);
                c.fill_rrect(ox, oy, ow, oh, 3.0, Rgba(51, 51, 56, 230));
                c.stroke_rrect(ox, oy, ow, oh, 3.0, 1.0, Rgba(255, 255, 255, 64));
            }
            if cur {
                c.stroke_rrect(x, bar_y, 160.0, 100.0, 10.0, 3.0, theme.accent);
            } else {
                c.stroke_rrect(x, bar_y, 160.0, 100.0, 10.0, 1.0, Rgba(255, 255, 255, 64));
            }
            let label = format!("Mesa {}", i + 1);
            let lw = c.measure(&label, 12.0, Weight::Regular);
            c.text(&label, x + (160.0 - lw) / 2.0, bar_y + 106.0, 12.0, Weight::Regular, Rgba(255, 255, 255, if cur { 255 } else { 140 }));
            self.spaces.push((Some(i), (x, bar_y, 160.0, 100.0)));
            x += 176.0;
        }
        c.fill_rrect(x, bar_y, 60.0, 100.0, 10.0, Rgba(255, 255, 255, 26));
        c.symbol("add", x + 30.0, bar_y + 50.0, 28.0, Rgba(255, 255, 255, 220));
        self.spaces.push((None, (x, bar_y, 60.0, 100.0)));

        // ---- molduras e legendas das janelas (o conteúdo vem do render)
        if self.windows.is_empty() {
            let t = "Nenhuma janela nesta mesa";
            let tw = c.measure(t, 18.0, Weight::Regular);
            c.text(t, (w - tw) / 2.0, h / 2.0, 18.0, Weight::Regular, Rgba(255, 255, 255, 140));
        }
        for i in 0..self.windows.len() {
            let (rx, ry, rw, rh) = self.rect(i);
            c.shadow_rrect_add(rx, ry + 6.0, rw, rh, 10.0, 19.0, (128.0 * p) as u8);
            if self.hover == Some(i) {
                c.stroke_rrect(rx - 3.0, ry - 3.0, rw + 6.0, rh + 6.0, 12.0, 3.0, theme.accent);
            }
            let win = &self.windows[i];
            let title = c.ellipsized(&win.title, 12.0, Weight::Regular, (rw - 44.0).max(40.0));
            let tw = c.measure(&title, 12.0, Weight::Regular) + 16.0;
            let total = 22.0 + 6.0 + tw;
            let cx = rx + (rw - total) / 2.0;
            let cy = ry + rh + 8.0;
            let a = (255.0 * p) as u8;
            let pm = icons.get_theme(&win.icon, &win.title).clone();
            c.blit(&pm, cx, cy, 22.0, 22.0, p);
            let bg = if self.hover == Some(i) { theme.accent } else { Rgba(0, 0, 0, 128) };
            c.fill_rrect(cx + 28.0, cy, tw, 22.0, 6.0, Rgba(bg.0, bg.1, bg.2, (bg.3 as f32 * p) as u8));
            c.text_vcenter(&title, cx + 36.0, cy, 22.0, 12.0, Weight::Regular, Rgba(255, 255, 255, a));
        }
        self.layer.set(&c);
    }

    pub fn animating(&self) -> bool {
        self.open && self.progress() < 1.0
    }
}
