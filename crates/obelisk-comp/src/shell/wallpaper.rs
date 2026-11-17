//! Papel de parede: PNG escalado para cobrir a saída (modo "preencher").

use resvg::tiny_skia::{FilterQuality, Pixmap, PixmapPaint, Transform};

use super::layer::Layer;
use crate::ui::canvas::Canvas;
use crate::ui::theme::Appearance;

pub struct Wallpaper {
    pub layer: Layer,
    key: Option<(i32, i32, Appearance, i32)>,
    /// Fundo desfocado (1/8 da tela lógica) usado pelo material de vidro.
    pub blurred: Option<Pixmap>,
    /// Luminância média (0..1) da faixa da barra de menus: decide texto claro ou escuro.
    pub top_luminance: f32,
    /// imagem escolhida nos Ajustes (senão, a padrão claro/escuro)
    pub custom: Option<String>,
}

impl Wallpaper {
    pub fn new() -> Self {
        Wallpaper { layer: Layer::default(), key: None, blurred: None, top_luminance: 0.0, custom: None }
    }

    pub fn set_custom(&mut self, p: Option<String>) {
        self.custom = p.filter(|s| !s.is_empty());
        self.key = None;
    }

    pub fn update(&mut self, w: i32, h: i32, scale: i32, appearance: Appearance) {
        let key = (w, h, appearance, scale);
        if self.key == Some(key) {
            return;
        }
        self.key = Some(key);
        let mut canvas = Canvas::new(w, h, scale as f32).unwrap();
        let (pw, ph) = (canvas.pm.width() as f32, canvas.pm.height() as f32);
        let src: Option<Pixmap> = self
            .custom
            .as_deref()
            .and_then(|p| crate::settings_ctl::load_image(p, (w * scale) as u32))
            .or_else(|| crate::assets::wallpaper(appearance == Appearance::Dark, w * scale).and_then(|p| crate::ui::icons::load_png(&p)));
        match src {
            Some(src) => {
                let (sw, sh) = (src.width() as f32, src.height() as f32);
                let s = (pw / sw).max(ph / sh);
                let (dx, dy) = ((pw - sw * s) / 2.0, (ph - sh * s) / 2.0);
                let mut paint = PixmapPaint::default();
                paint.quality = FilterQuality::Bicubic;
                canvas.pm.draw_pixmap(0, 0, src.as_ref(), &paint, Transform::from_row(s, 0.0, 0.0, s, dx, dy), None);
            }
            None => {
                // sem ativos: gradiente simples para não ficar preto
                let (a, b) = if appearance == Appearance::Dark {
                    (crate::ui::theme::Rgba(18, 20, 38, 255), crate::ui::theme::Rgba(60, 40, 90, 255))
                } else {
                    (crate::ui::theme::Rgba(170, 200, 245, 255), crate::ui::theme::Rgba(235, 215, 245, 255))
                };
                canvas.fill_rrect_vgradient(0.0, 0.0, w as f32, h as f32, 0.0, a, b);
            }
        }
        self.layer.set(&canvas);

        // luminância da faixa superior (altura da barra)
        let bar_px = ((crate::ui::theme::metrics::MENUBAR_H * scale) as u32).min(canvas.pm.height());
        let (mut sum, mut n) = (0f64, 0u64);
        for y in (0..bar_px).step_by(2) {
            for x in (0..canvas.pm.width()).step_by(8) {
                let p = canvas.pm.pixel(x, y).unwrap();
                sum += (0.2126 * p.red() as f64 + 0.7152 * p.green() as f64 + 0.0722 * p.blue() as f64) / 255.0;
                n += 1;
            }
        }
        self.top_luminance = if n > 0 { (sum / n as f64) as f32 } else { 0.0 };

        // vidro: reduz para 1/8, desfoca e satura (≈ "vibrancy")
        let (bw, bh) = (((w / 8).max(1)) as u32, ((h / 8).max(1)) as u32);
        if let Some(mut small) = Pixmap::new(bw, bh) {
            let mut paint = PixmapPaint::default();
            paint.quality = FilterQuality::Bilinear;
            let s = bw as f32 / canvas.pm.width() as f32;
            small.draw_pixmap(0, 0, canvas.pm.as_ref(), &paint, Transform::from_scale(s, s), None);
            crate::ui::canvas::blur_rgba(&mut small, 4, 1.6);
            self.blurred = Some(small);
        }
    }
}
