//! Cursor do tema Xcursor (padrão: MacTahoe, GPL-3.0, o mesmo do kyro).

use std::collections::HashMap;

use resvg::tiny_skia::{Pixmap, PremultipliedColorU8};
use xcursor::{parser::parse_xcursor, CursorTheme};

pub struct CursorStore {
    theme: CursorTheme,
    size: u32,
    cache: HashMap<(String, u32), Option<(Pixmap, (i32, i32))>>,
}

impl CursorStore {
    pub fn new() -> Self {
        let name = std::env::var("XCURSOR_THEME").unwrap_or_else(|_| "MacTahoe-cursors".into());
        let size = std::env::var("XCURSOR_SIZE").ok().and_then(|s| s.parse().ok()).unwrap_or(24);
        CursorStore { theme: CursorTheme::load(&name), size, cache: HashMap::new() }
    }

    /// Imagem (pixels do buffer) e hotspot para o cursor `name` na escala `scale`.
    pub fn get(&mut self, name: &str, scale: u32) -> Option<&(Pixmap, (i32, i32))> {
        let key = (name.to_string(), scale);
        if !self.cache.contains_key(&key) {
            let v = self.load(name, scale).or_else(|| self.load("default", scale)).or_else(|| self.load("left_ptr", scale));
            self.cache.insert(key.clone(), v);
        }
        self.cache.get(&key).and_then(|v| v.as_ref())
    }

    fn load(&self, name: &str, scale: u32) -> Option<(Pixmap, (i32, i32))> {
        let path = self.theme.load_icon(name)?;
        let data = std::fs::read(path).ok()?;
        let images = parse_xcursor(&data)?;
        let want = self.size * scale;
        let img = images.iter().min_by_key(|i| (i.size as i32 - want as i32).abs())?;
        let mut pm = Pixmap::new(img.width, img.height)?;
        for (dst, src) in pm.pixels_mut().iter_mut().zip(img.pixels_rgba.chunks_exact(4)) {
            let a = src[3] as u16;
            let pre = |c: u8| ((c as u16 * a) / 255) as u8;
            if let Some(p) = PremultipliedColorU8::from_rgba(pre(src[0]), pre(src[1]), pre(src[2]), src[3]) {
                *dst = p;
            }
        }
        Some((pm, (img.xhot as i32, img.yhot as i32)))
    }
}
