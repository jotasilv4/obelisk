//! Texto com `fontdue`. Procura a fonte Inter (substituta aberta da SF Pro, que é
//! proprietária) via `fc-match` e cai para DejaVu/Noto se não houver.

use std::cell::RefCell;
use std::collections::HashMap;
use std::process::Command;

use fontdue::{Font, FontSettings};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Weight {
    Light,
    Regular,
    Medium,
    Semibold,
    Bold,
    /// Material Symbols Rounded (ícones no lugar dos SF Symbols)
    Symbol,
}

struct Glyph {
    xmin: i32,
    ymin: i32,
    width: usize,
    height: usize,
    advance: f32,
    coverage: Vec<u8>,
}

pub struct Fonts {
    light: Font,
    medium: Font,
    regular: Font,
    semibold: Font,
    bold: Font,
    symbol: Option<Font>,
    cache: HashMap<(char, u32, Weight), std::rc::Rc<Glyph>>,
}

/// (arquivo, índice na coleção) que o fontconfig escolhe para o padrão.
fn fc_match(pattern: &str) -> Option<(String, u32)> {
    let out = Command::new("fc-match").args(["-f", "%{file}|%{index}", pattern]).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8(out.stdout).ok()?;
    let (f, i) = s.split_once('|')?;
    (!f.is_empty()).then(|| (f.to_string(), i.trim().parse().unwrap_or(0)))
}

fn load(patterns: &[&str], fallbacks: &[&str]) -> Option<Font> {
    let mut candidates: Vec<(String, u32)> = patterns.iter().filter_map(|p| fc_match(p)).collect();
    candidates.extend(fallbacks.iter().map(|s| (s.to_string(), 0)));
    for (path, index) in candidates {
        if let Ok(bytes) = std::fs::read(&path) {
            let settings = FontSettings { collection_index: index, ..FontSettings::default() };
            if let Ok(font) = Font::from_bytes(bytes, settings) {
                tracing::debug!("fonte carregada: {path} #{index}");
                return Some(font);
            }
        }
    }
    None
}

impl Fonts {
    fn new() -> Self {
        let fallbacks = [
            "/usr/share/fonts/inter/InterVariable.ttf",
            "/usr/share/fonts/TTF/DejaVuSans.ttf",
            "/usr/share/fonts/noto/NotoSans-Regular.ttf",
            "/usr/share/fonts/liberation/LiberationSans-Regular.ttf",
        ];
        let fallbacks_bold = [
            "/usr/share/fonts/TTF/DejaVuSans-Bold.ttf",
            "/usr/share/fonts/noto/NotoSans-Bold.ttf",
            "/usr/share/fonts/liberation/LiberationSans-Bold.ttf",
        ];
        let regular = load(&["Inter:weight=80", "sans-serif"], &fallbacks)
            .expect("nenhuma fonte encontrada (instale inter-font)");
        let semibold = load(&["Inter:weight=180", "sans-serif:bold"], &fallbacks_bold)
            .unwrap_or_else(|| load(&["sans-serif"], &fallbacks).expect("fonte"));
        let bold = load(&["Inter:weight=200", "sans-serif:bold"], &fallbacks_bold)
            .unwrap_or_else(|| load(&["sans-serif"], &fallbacks).expect("fonte"));
        let symbol = load(
            &[],
            &[
                "/usr/share/fonts/TTF/MaterialSymbolsRounded[FILL,GRAD,opsz,wght].ttf",
                "/usr/share/fonts/TTF/MaterialSymbolsOutlined[FILL,GRAD,opsz,wght].ttf",
            ],
        );
        if symbol.is_none() {
            tracing::warn!("Material Symbols não encontrada (pacote ttf-material-symbols-variable): ícones viram texto");
        }
        let light = load(&["Inter:weight=50"], &fallbacks).unwrap_or_else(|| load(&["sans-serif"], &fallbacks).expect("fonte"));
        let medium = load(&["Inter:weight=100"], &fallbacks).unwrap_or_else(|| load(&["sans-serif"], &fallbacks).expect("fonte"));
        Fonts { light, medium, regular, semibold, bold, symbol, cache: HashMap::new() }
    }

    fn font(&self, w: Weight) -> &Font {
        match w {
            Weight::Light => &self.light,
            Weight::Regular => &self.regular,
            Weight::Medium => &self.medium,
            Weight::Semibold => &self.semibold,
            Weight::Bold => &self.bold,
            Weight::Symbol => self.symbol.as_ref().unwrap_or(&self.regular),
        }
    }

    fn glyph(&mut self, ch: char, px: f32, w: Weight) -> std::rc::Rc<Glyph> {
        let key = (ch, (px * 4.0) as u32, w);
        if let Some(g) = self.cache.get(&key) {
            return g.clone();
        }
        let (m, bitmap) = self.font(w).rasterize(ch, px);
        let g = std::rc::Rc::new(Glyph {
            xmin: m.xmin,
            ymin: m.ymin,
            width: m.width,
            height: m.height,
            advance: m.advance_width,
            coverage: bitmap,
        });
        self.cache.insert(key, g.clone());
        g
    }
}

thread_local! {
    static FONTS: RefCell<Option<Fonts>> = const { RefCell::new(None) };
}

fn with_fonts<R>(f: impl FnOnce(&mut Fonts) -> R) -> R {
    FONTS.with(|cell| {
        let mut b = cell.borrow_mut();
        if b.is_none() {
            *b = Some(Fonts::new());
        }
        f(b.as_mut().unwrap())
    })
}

/// Largura (em px físicos) de `text` no tamanho `px`.
pub fn measure(text: &str, px: f32, weight: Weight) -> f32 {
    with_fonts(|f| text.chars().map(|c| f.glyph(c, px, weight).advance).sum())
}

/// Altura de linha ascendente/descendente (px físicos).
pub fn line_metrics(px: f32, weight: Weight) -> (f32, f32) {
    with_fonts(|f| {
        f.font(weight)
            .horizontal_line_metrics(px)
            .map(|m| (m.ascent, m.descent))
            .unwrap_or((px * 0.9, -px * 0.2))
    })
}

/// Corta `text` com reticências para caber em `max_w` px físicos.
pub fn ellipsize(text: &str, px: f32, weight: Weight, max_w: f32) -> String {
    if measure(text, px, weight) <= max_w {
        return text.to_string();
    }
    let ell = "…";
    let mut out = String::new();
    for c in text.chars() {
        let mut probe = out.clone();
        probe.push(c);
        probe.push_str(ell);
        if measure(&probe, px, weight) > max_w {
            break;
        }
        out.push(c);
    }
    out.push_str(ell);
    out
}

/// Desenha `text` num pixmap RGBA pré-multiplicado, com a linha de base em `baseline`.
/// Tudo em px físicos. Devolve a largura desenhada.
pub fn draw(
    pm: &mut resvg::tiny_skia::Pixmap,
    text: &str,
    x: f32,
    baseline: f32,
    px: f32,
    weight: Weight,
    color: super::theme::Rgba,
) -> f32 {
    let (pw, ph) = (pm.width() as i32, pm.height() as i32);
    let stride = pw as usize;
    let data = pm.pixels_mut();
    let mut pen = x;
    with_fonts(|f| {
        for ch in text.chars() {
            let g = f.glyph(ch, px, weight);
            let gx = (pen + g.xmin as f32).round() as i32;
            let gy = (baseline - g.height as f32 - g.ymin as f32).round() as i32;
            for row in 0..g.height {
                let y = gy + row as i32;
                if y < 0 || y >= ph {
                    continue;
                }
                for col in 0..g.width {
                    let xx = gx + col as i32;
                    if xx < 0 || xx >= pw {
                        continue;
                    }
                    let cov = g.coverage[row * g.width + col] as u32;
                    if cov == 0 {
                        continue;
                    }
                    let a = cov * color.3 as u32 / 255; // 0..255
                    let idx = y as usize * stride + xx as usize;
                    let dst = data[idx];
                    let inv = 255 - a;
                    let r = (color.0 as u32 * a + dst.red() as u32 * inv) / 255;
                    let g_ = (color.1 as u32 * a + dst.green() as u32 * inv) / 255;
                    let b = (color.2 as u32 * a + dst.blue() as u32 * inv) / 255;
                    let al = (a + dst.alpha() as u32 * inv / 255).min(255);
                    if let Some(px) = resvg::tiny_skia::PremultipliedColorU8::from_rgba(
                        r.min(255) as u8,
                        g_.min(255) as u8,
                        b.min(255) as u8,
                        al as u8,
                    ) {
                        data[idx] = px;
                    }
                }
            }
            pen += g.advance;
        }
    });
    pen - x
}
