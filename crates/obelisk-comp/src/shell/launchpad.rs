//! Launchpad (docs/03-especificacao-kyro.md §7): tela inteira sobre o fundo desfocado,
//! busca no topo, categorias e grade de apps (ícone 84 em células 150×140).

use std::time::Instant;

use resvg::tiny_skia::Pixmap;

use super::layer::Layer;
use crate::apps::App;
use crate::ui::canvas::Canvas;
use crate::ui::icons::IconStore;
use crate::ui::text::Weight;
use crate::ui::theme::Rgba;

pub const CATEGORIES: [(&str, &str, &[&str]); 7] = [
    ("Todos", "apps", &[]),
    ("Jogos", "sports_esports", &["Game"]),
    ("Criatividade", "edit", &["Graphics", "AudioVideo", "Audio", "Video"]),
    ("Produtividade", "description", &["Office", "TextEditor", "Development"]),
    ("Internet", "public", &["Network", "WebBrowser", "Email", "Chat"]),
    ("Entretenimento", "music_note", &["Player", "Music", "TV"]),
    ("Utilitários", "settings", &["Utility", "System", "Settings", "FileManager"]),
];

const CELL_W: f32 = 150.0;
const CELL_H: f32 = 140.0;
const ICON: f32 = 84.0;

#[derive(Default)]
pub struct Launchpad {
    pub open: bool,
    pub query: String,
    pub category: usize,
    pub hover: Option<usize>,
    pub scroll: f32,
    pub opened: Option<Instant>,
    pub layer: Layer,
    visible: Vec<usize>,
    /// (x, y) da célula i em px lógicos (tela)
    cells: Vec<(f32, f32)>,
    key: Option<String>,
}

impl Launchpad {
    pub fn toggle(&mut self) {
        self.open = !self.open;
        self.query.clear();
        self.category = 0;
        self.scroll = 0.0;
        self.hover = None;
        self.key = None;
        self.opened = self.open.then(Instant::now);
    }

    pub fn close(&mut self) {
        if self.open {
            self.toggle();
        }
    }

    fn filter(&mut self, apps: &[App]) {
        let q = self.query.trim().to_lowercase();
        let cats = CATEGORIES[self.category].2;
        self.visible = apps
            .iter()
            .enumerate()
            // como no macOS, sem variantes auxiliares do mesmo app ("Foot Client", "Foot Server"):
            // esconde a entrada cujo nome começa com o de outro app com o mesmo ícone
            .filter(|(_, a)| !apps.iter().any(|o| o.icon == a.icon && o.name.len() < a.name.len() && a.name.starts_with(&format!("{} ", o.name))))
            .filter(|(_, a)| {
                if !q.is_empty() {
                    return crate::apps::score(a, &q).is_some();
                }
                cats.is_empty() || cats.iter().any(|c| a.categories.split(';').any(|x| x == *c))
            })
            .map(|(i, _)| i)
            .collect();
    }

    /// App sob (x, y) na tela.
    pub fn app_at(&self, x: f64, y: f64) -> Option<usize> {
        self.cells.iter().position(|(cx, cy)| {
            (x as f32) >= cx + 10.0 && (x as f32) < cx + CELL_W - 10.0 && (y as f32) >= cy + 10.0 && (y as f32) < cy + CELL_H - 10.0
        })
        .map(|i| self.visible[i])
    }

    /// Categoria sob (x, y).
    pub fn category_at(&self, x: f64, y: f64, screen_w: f32) -> Option<usize> {
        let (xs, y0) = Self::pill_layout(screen_w);
        xs.iter().position(|(px, w)| (x as f32) >= *px && (x as f32) < px + w && (y as f32) >= y0 && (y as f32) < y0 + 30.0)
    }

    fn pill_layout(screen_w: f32) -> (Vec<(f32, f32)>, f32) {
        let widths: Vec<f32> = CATEGORIES
            .iter()
            .map(|(l, _, _)| crate::ui::text::measure(l, 13.0, Weight::Semibold) + 15.0 + 6.0 + 26.0)
            .collect();
        let total: f32 = widths.iter().sum::<f32>() + 8.0 * (widths.len() as f32 - 1.0);
        let mut x = (screen_w - total) / 2.0;
        let mut out = Vec::new();
        for w in widths {
            out.push((x, w));
            x += w + 8.0;
        }
        (out, 56.0 + 32.0 + 20.0)
    }

    pub fn first(&self) -> Option<usize> {
        self.visible.first().copied()
    }

    pub fn update(&mut self, apps: &[App], icons: &mut IconStore, sw: i32, sh: i32, scale: i32, backdrop: Option<&Pixmap>) {
        self.filter(apps);
        let key = format!("{}|{}|{:?}|{}|{}|{}|{}", self.query, self.category, self.hover, self.scroll, sw, sh, self.visible.len());
        if self.key.as_deref() == Some(key.as_str()) && self.layer.is_set() {
            return;
        }
        self.key = Some(key);
        let (w, h) = (sw as f32, sh as f32);
        let mut c = Canvas::new(sw, sh, scale as f32).unwrap();
        if let Some(bd) = backdrop {
            c.fill_rrect_backdrop(0.0, 0.0, w, h, 0.0, bd, (0.0, 0.0), (w, h));
        }
        c.fill_rrect(0.0, 0.0, w, h, 0.0, Rgba(5, 5, 13, 90));

        // busca
        let (bw, bh) = (260.0, 32.0);
        let bx = (w - bw) / 2.0;
        c.fill_rrect(bx, 56.0, bw, bh, 9.0, Rgba(255, 255, 255, 31));
        c.stroke_rrect(bx, 56.0, bw, bh, 9.0, 1.0, Rgba(255, 255, 255, 31));
        if self.query.is_empty() {
            let tw = c.measure("Buscar", 13.0, Weight::Medium) + 22.0;
            let tx = (w - tw) / 2.0;
            c.symbol("search", tx + 8.0, 56.0 + bh / 2.0, 16.0, Rgba(255, 255, 255, 140));
            c.text_vcenter("Buscar", tx + 22.0, 56.0, bh, 13.0, Weight::Medium, Rgba(255, 255, 255, 140));
        } else {
            let t = c.ellipsized(&self.query, 14.0, Weight::Regular, bw - 24.0);
            let tw = c.measure(&t, 14.0, Weight::Regular);
            c.text_vcenter(&t, (w - tw) / 2.0, 56.0, bh, 14.0, Weight::Regular, Rgba(255, 255, 255, 217));
        }

        // categorias
        let (pills, py) = Self::pill_layout(w);
        let fade = if self.query.is_empty() { 1.0 } else { 0.45 };
        for (i, ((px, pw), (label, sym, _))) in pills.iter().zip(CATEGORIES.iter()).enumerate() {
            let sel = i == self.category;
            let a = |v: u8| (v as f32 * fade) as u8;
            let (bg, fgc) = if sel { (Rgba(255, 255, 255, a(230)), Rgba(0x1c, 0x1c, 0x1e, a(255))) } else { (Rgba(255, 255, 255, a(31)), Rgba(255, 255, 255, a(230))) };
            c.fill_rrect(*px, py, *pw, 30.0, 15.0, bg);
            if !sel {
                c.stroke_rrect(*px, py, *pw, 30.0, 15.0, 1.0, Rgba(255, 255, 255, a(26)));
            }
            c.symbol(sym, px + 13.0 + 7.5, py + 15.0, 15.0, fgc);
            c.text_vcenter(label, px + 13.0 + 15.0 + 6.0, py, 30.0, 13.0, Weight::Semibold, fgc);
        }

        // grade
        let gy = py + 30.0 + 32.0;
        let cols = (((w - 120.0) / CELL_W).floor() as usize).clamp(3, 7);
        let gx = (w - cols as f32 * CELL_W) / 2.0;
        self.cells.clear();
        if self.visible.is_empty() {
            let t = "Nenhum resultado";
            let tw = c.measure(t, 18.0, Weight::Regular);
            c.text(t, (w - tw) / 2.0, gy + 80.0, 18.0, Weight::Regular, Rgba(255, 255, 255, 140));
        }
        for (n, &ai) in self.visible.iter().enumerate() {
            let (col, row) = (n % cols, n / cols);
            let x = gx + col as f32 * CELL_W;
            let y = gy + row as f32 * CELL_H - self.scroll;
            self.cells.push((x, y));
            if y + CELL_H < gy || y > h - 40.0 {
                continue;
            }
            let app = &apps[ai];
            let hovered = self.hover == Some(ai);
            let s = if hovered { ICON * 1.05 } else { ICON };
            let pm = icons.get_theme(&app.icon, &app.name).clone();
            c.blit(&pm, x + (CELL_W - s) / 2.0, y + 10.0 + (ICON - s) / 2.0, s, s, 1.0);
            let label = c.ellipsized(&app.name, 12.0, Weight::Medium, 130.0);
            let lw = c.measure(&label, 12.0, Weight::Medium);
            let lx = x + (CELL_W - lw) / 2.0;
            let ly = y + 10.0 + ICON + 8.0;
            c.text(&label, lx, ly + 1.0, 12.0, Weight::Medium, Rgba(0, 0, 0, 90));
            c.text(&label, lx, ly, 12.0, Weight::Medium, Rgba(255, 255, 255, 255));
        }
        self.layer.set(&c);
    }

    /// Opacidade e escala da abertura (220 ms / 1,08→1 em 320 ms).
    pub fn intro(&self) -> (f32, f32) {
        let t = self.opened.map(|o| o.elapsed().as_secs_f32()).unwrap_or(1.0);
        let alpha = (t / 0.22).min(1.0);
        let p = (t / 0.32).min(1.0);
        let ease = 1.0 - (1.0 - p).powi(3);
        (alpha, 1.08 - 0.08 * ease)
    }
}
