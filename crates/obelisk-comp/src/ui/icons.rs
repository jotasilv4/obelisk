//! Rasterização e cache de ícones SVG (via `resvg`) com *fallback* gerado.

use std::collections::HashMap;
use std::path::Path;

use resvg::tiny_skia::{Pixmap, Transform};
use resvg::usvg;

use super::canvas::Canvas;
use super::text::Weight;
use super::theme::Rgba;

/// Resolução base de rasterização; a dock reduz a partir daqui.
pub const BASE_RES: u32 = 192;

pub fn render_svg(path: &Path, size: u32) -> Option<Pixmap> {
    let data = std::fs::read(path).ok()?;
    let tree = usvg::Tree::from_data(&data, &usvg::Options::default()).ok()?;
    let mut pm = Pixmap::new(size, size)?;
    let ts = tree.size();
    let s = (size as f32 / ts.width()).min(size as f32 / ts.height());
    let dx = (size as f32 - ts.width() * s) / 2.0;
    let dy = (size as f32 - ts.height() * s) / 2.0;
    resvg::render(&tree, Transform::from_row(s, 0.0, 0.0, s, dx, dy), &mut pm.as_mut());
    Some(pm)
}

pub fn load_png(path: &Path) -> Option<Pixmap> {
    Pixmap::load_png(path).ok()
}

/// Ícone genérico: squircle colorido com a inicial (usado quando falta o SVG).
pub fn fallback(label: &str, size: u32) -> Pixmap {
    let mut c = Canvas::new(size as i32, size as i32, 1.0).unwrap();
    let hue = label.bytes().fold(7u32, |a, b| a.wrapping_mul(31).wrapping_add(b as u32)) % 360;
    let (r, g, b) = hsl(hue as f32, 0.55, 0.52);
    let sz = size as f32;
    c.fill_rrect_vgradient(
        0.0, 0.0, sz, sz, sz * 0.225,
        Rgba(r.saturating_add(30), g.saturating_add(30), b.saturating_add(30), 255),
        Rgba(r, g, b, 255),
    );
    let initial: String = label.chars().next().map(|c| c.to_uppercase().collect()).unwrap_or_else(|| "?".into());
    let fs = sz * 0.5;
    let w = c.measure(&initial, fs, Weight::Semibold);
    c.text_vcenter(&initial, (sz - w) / 2.0, 0.0, sz, fs, Weight::Semibold, Rgba(255, 255, 255, 235));
    c.pm
}

fn hsl(h: f32, s: f32, l: f32) -> (u8, u8, u8) {
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = l - c / 2.0;
    let (r, g, b) = match h as u32 / 60 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    (((r + m) * 255.0) as u8, ((g + m) * 255.0) as u8, ((b + m) * 255.0) as u8)
}

#[derive(Default)]
pub struct IconStore {
    cache: HashMap<String, Pixmap>,
}

impl IconStore {
    /// Ícone do Obelisk por nome (`files`, `terminal`...). Sempre devolve algo.
    pub fn get(&mut self, name: &str) -> &Pixmap {
        self.cache.entry(name.to_string()).or_insert_with(|| {
            crate::assets::icon_svg(name)
                .and_then(|p| render_svg(&p, BASE_RES))
                .unwrap_or_else(|| fallback(name, BASE_RES))
        })
    }

    /// Ícone de um arquivo .desktop (`Icon=`): caminho absoluto ou nome no tema hicolor/pixmaps.
    pub fn get_theme(&mut self, icon: &str, label: &str) -> &Pixmap {
        let key = format!("theme:{icon}");
        self.cache.entry(key).or_insert_with(|| {
            find_theme_icon(icon)
                .and_then(|p| {
                    if p.extension().is_some_and(|e| e == "svg") {
                        render_svg(&p, BASE_RES)
                    } else {
                        load_png(&p)
                    }
                })
                .unwrap_or_else(|| fallback(label, BASE_RES))
        })
    }
}

fn find_theme_icon(icon: &str) -> Option<std::path::PathBuf> {
    use std::path::PathBuf;
    if icon.starts_with('/') {
        let p = PathBuf::from(icon);
        return p.is_file().then_some(p);
    }
    let mut roots: Vec<PathBuf> = vec![PathBuf::from("/usr/share/icons")];
    if let Some(h) = std::env::var_os("HOME") {
        roots.push(PathBuf::from(h).join(".local/share/icons"));
    }
    // WhiteSur (GPL-3.0) é o tema de ícones estilo macOS que o kyro também usa
    let themes = ["WhiteSur-dark", "WhiteSur", "Papirus", "Adwaita", "hicolor"];
    let sizes = ["scalable", "512x512", "256x256", "128x128", "96x96", "64x64", "48x48", "48", "64", "128"];
    let cats = ["apps", "applications", "places", "status"];
    for root in &roots {
        for t in themes {
            for s in sizes {
                for c in cats {
                    for ext in ["svg", "png"] {
                        let p = root.join(t).join(s).join(c).join(format!("{icon}.{ext}"));
                        if p.is_file() {
                            return Some(p);
                        }
                        // layout "apps/<tamanho>"
                        let p = root.join(t).join(c).join(s).join(format!("{icon}.{ext}"));
                        if p.is_file() {
                            return Some(p);
                        }
                    }
                }
            }
        }
    }
    let p = PathBuf::from("/usr/share/pixmaps").join(format!("{icon}.png"));
    p.is_file().then_some(p)
}
