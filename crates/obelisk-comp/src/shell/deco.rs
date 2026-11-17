//! Decoração de janela no servidor: barra de título com "semáforo" à esquerda,
//! título centralizado e sombra suave.

use super::layer::Layer;
use crate::ui::canvas::Canvas;
use crate::ui::text::Weight;
use crate::ui::theme::{metrics, Theme};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Button {
    Close,
    Minimize,
    Zoom,
}

pub const BUTTONS: [Button; 3] = [Button::Close, Button::Minimize, Button::Zoom];

/// Retângulo (x, y, w, h) do botão `i` relativo ao canto superior esquerdo da barra de título.
pub fn button_rect(i: usize) -> (f32, f32, f32, f32) {
    let d = metrics::TRAFFIC_D;
    let x = 13.0 + i as f32 * (d + metrics::TRAFFIC_GAP);
    let y = (metrics::TITLEBAR_H as f32 - d) / 2.0;
    (x, y, d, d)
}

pub fn button_at(x: f64, y: f64) -> Option<usize> {
    (0..3).find(|&i| {
        let (bx, by, bw, bh) = button_rect(i);
        // área de clique levemente maior que o desenho
        x >= (bx - 3.0) as f64 && x <= (bx + bw + 3.0) as f64 && y >= (by - 4.0) as f64 && y <= (by + bh + 4.0) as f64
    })
}

#[derive(Default)]
pub struct Deco {
    pub title_layer: Layer,
    pub shadow_layer: Layer,
    title_key: Option<(i32, bool, Option<usize>, String, bool, bool)>,
    shadow_key: Option<(i32, i32, bool, i32)>,
}

impl Deco {
    #[allow(clippy::too_many_arguments)]
    pub fn update_title(
        &mut self,
        theme: &Theme,
        width: i32,
        scale: i32,
        title: &str,
        focused: bool,
        hover_group: bool,
        hover_btn: Option<usize>,
        maximized: bool,
    ) {
        let key = (width, focused, hover_btn, title.to_string(), hover_group, maximized);
        if self.title_key.as_ref() == Some(&key) && self.title_layer.is_set() {
            return;
        }
        self.title_key = Some(key);
        let h = metrics::TITLEBAR_H;
        let mut c = Canvas::new(width, h, scale as f32).unwrap();
        let bg = if focused { theme.titlebar_bg } else { theme.titlebar_bg_inactive };
        let r = if maximized { 0.0 } else { metrics::WINDOW_RADIUS };
        c.fill_rrect_corners(0.0, 0.0, width as f32, h as f32, [r, r, 0.0, 0.0], bg);
        // linha separadora inferior
        c.fill_rrect(0.0, h as f32 - 1.0, width as f32, 1.0, 0.0, theme.separator);

        for (i, b) in BUTTONS.iter().enumerate() {
            let (x, y, d, _) = button_rect(i);
            let color = if focused || hover_group {
                match b {
                    Button::Close => theme.tl_close,
                    Button::Minimize => theme.tl_min,
                    Button::Zoom => theme.tl_max,
                }
            } else {
                theme.tl_inactive
            };
            c.fill_circle(x + d / 2.0, y + d / 2.0, d / 2.0, color);
            c.stroke_circle_outline(x + d / 2.0, y + d / 2.0, d / 2.0);
            // glifos aparecem ao passar o mouse sobre o grupo
            if hover_group {
                let glyph = crate::ui::text_glyph::glyph_color();
                let (cx, cy) = (x + d / 2.0, y + d / 2.0);
                match b {
                    Button::Close => crate::ui::text_glyph::cross(&mut c, cx, cy, 2.6, glyph),
                    Button::Minimize => crate::ui::text_glyph::minus(&mut c, cx, cy, 3.0, glyph),
                    Button::Zoom => crate::ui::text_glyph::plus(&mut c, cx, cy, 3.0, glyph),
                }
            }
            if hover_btn == Some(i) {
                c.fill_circle(cx_of(x, d), cy_of(y, d), d / 2.0, crate::ui::theme::Rgba(0, 0, 0, 30));
            }
        }

        // título centralizado (com reticências), reservando o espaço do semáforo
        let color = if focused { theme.titlebar_text } else { theme.titlebar_text_inactive };
        let avail = width as f32 - 2.0 * 84.0;
        if avail > 20.0 && !title.is_empty() {
            let t = c.ellipsized(title, 13.0, Weight::Semibold, avail);
            let tw = c.measure(&t, 13.0, Weight::Semibold);
            c.text_vcenter(&t, (width as f32 - tw) / 2.0, 0.0, h as f32, 13.0, Weight::Semibold, color);
        }
        self.title_layer.set(&c);
    }

    /// Sombra ao redor do quadro (barra de título + conteúdo).
    pub fn update_shadow(&mut self, frame_w: i32, frame_h: i32, scale: i32, focused: bool, maximized: bool) {
        let key = (frame_w, frame_h, focused, scale);
        if maximized {
            self.shadow_key = None;
            self.shadow_layer = Layer::default();
            return;
        }
        if self.shadow_key == Some(key) && self.shadow_layer.is_set() {
            return;
        }
        self.shadow_key = Some(key);
        let pad = metrics::SHADOW_PAD;
        let mut c = Canvas::new(frame_w + 2 * pad, frame_h + 2 * pad, scale as f32).unwrap();
        let (spread, alpha) = if focused { (30.0, 110u8) } else { (20.0, 60u8) };
        // deslocada um pouco para baixo (luz vinda de cima)
        c.shadow_rrect(pad as f32, pad as f32 + 6.0, frame_w as f32, frame_h as f32, metrics::WINDOW_RADIUS, spread, alpha);
        // recorta o interior: a janela cobre; mantemos só o contorno de 1px
        self.shadow_layer.set(&c);
    }
}

fn cx_of(x: f32, d: f32) -> f32 {
    x + d / 2.0
}
fn cy_of(y: f32, d: f32) -> f32 {
    y + d / 2.0
}
