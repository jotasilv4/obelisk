//! ⌘Tab (docs/03-especificacao-kyro.md §9): painel central, ícones 80 em células 96×96,
//! ordem de uso, soltar ⌘ confirma, Esc cancela.

use resvg::tiny_skia::Pixmap;

use super::layer::Layer;
use crate::ui::canvas::Canvas;
use crate::ui::icons::IconStore;
use crate::ui::text::Weight;
use crate::ui::theme::Theme;

pub struct SwitchItem {
    pub name: String,
    pub icon: String,
    pub minimized: bool,
}

#[derive(Default)]
pub struct Switcher {
    pub open: bool,
    pub selected: usize,
    pub items: Vec<SwitchItem>,
    pub layer: Layer,
    pub pos: (i32, i32),
    key: Option<(usize, usize, u8)>,
}

const SHADOW: f32 = 40.0;

impl Switcher {
    pub fn update(&mut self, theme: &Theme, icons: &mut IconStore, sw: i32, sh: i32, scale: i32, backdrop: Option<&Pixmap>) {
        let n = self.items.len().max(1) as f32;
        let row = n * 96.0 + (n - 1.0) * 6.0;
        let (w, h) = (row + 28.0, 150.0);
        let x = ((sw as f32 - w) / 2.0).round();
        let y = ((sh as f32 - h) / 2.0).round();
        self.pos = ((x - SHADOW) as i32, (y - SHADOW) as i32);
        let key = (self.selected, self.items.len(), theme.appearance as u8);
        if self.key == Some(key) && self.layer.is_set() {
            return;
        }
        self.key = Some(key);
        let mut c = Canvas::new((w + 2.0 * SHADOW) as i32, (h + 2.0 * SHADOW) as i32, scale as f32).unwrap();
        let (px, py) = (SHADOW, SHADOW);
        c.shadow_rrect(px, py + 10.0, w, h, 26.0, 40.0, 128);
        if let Some(bd) = backdrop {
            c.fill_rrect_backdrop(px, py, w, h, 26.0, bd, (x - SHADOW, y - SHADOW), (sw as f32, sh as f32));
        }
        c.fill_rrect(px, py, w, h, 26.0, theme.menu_bg);
        c.stroke_rrect(px, py, w, h, 26.0, 1.0, theme.menu_border);
        for (i, it) in self.items.iter().enumerate() {
            let cx = px + 14.0 + i as f32 * 102.0;
            let cy = py + 16.0;
            if i == self.selected {
                c.fill_rrect(cx, cy, 96.0, 96.0, 18.0, theme.surface_hover);
            }
            let pm = icons.get_theme(&it.icon, &it.name).clone();
            c.blit(&pm, cx + 8.0, cy + 8.0, 80.0, 80.0, 1.0);
            if it.minimized {
                c.fill_circle(cx + 48.0, cy + 96.0 - 3.0 - 2.5, 2.5, theme.secondary);
            }
        }
        if let Some(it) = self.items.get(self.selected) {
            let t = c.ellipsized(&it.name, 13.0, Weight::Semibold, w - 20.0);
            let tw = c.measure(&t, 13.0, Weight::Semibold);
            c.text(&t, px + (w - tw) / 2.0, py + h - 12.0 - 16.0, 13.0, Weight::Semibold, theme.label);
        }
        self.layer.set(&c);
    }
}
