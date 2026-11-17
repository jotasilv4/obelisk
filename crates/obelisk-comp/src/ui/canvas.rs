//! Canvas 2D sobre `tiny-skia`. Coordenadas em pixels lógicos; `scale` converte
//! para pixels do buffer (HiDPI).

use resvg::tiny_skia::{
    Color, FillRule, FilterQuality, GradientStop, LinearGradient, Paint, Path, PathBuilder, Pixmap, PixmapPaint,
    Pattern, Point, SpreadMode, Stroke, Transform,
};

use super::text::{self, Weight};
use super::theme::Rgba;

pub struct Canvas {
    pub pm: Pixmap,
    pub scale: f32,
}

/// Retângulo com cantos "contínuos" (superelipse leve), no estilo dos cantos do macOS.
/// `r` = [topo-esq, topo-dir, baixo-dir, baixo-esq].
pub fn rrect_path(x: f32, y: f32, w: f32, h: f32, r: [f32; 4]) -> Option<Path> {
    const N: f32 = 3.2; // expoente: 2 = círculo; maior = mais "quadrado"
    const SEG: usize = 14;
    let max_r = w.min(h) / 2.0;
    let r = r.map(|v| v.clamp(0.0, max_r));
    let mut pb = PathBuilder::new();
    // corners: (center x, center y, start angle)
    let corners = [
        (x + r[0], y + r[0], std::f32::consts::PI, r[0]),                   // TL
        (x + w - r[1], y + r[1], 1.5 * std::f32::consts::PI, r[1]),         // TR
        (x + w - r[2], y + h - r[2], 0.0, r[2]),                            // BR
        (x + r[3], y + h - r[3], 0.5 * std::f32::consts::PI, r[3]),         // BL
    ];
    let mut first = true;
    for (cx, cy, start, rad) in corners {
        if rad <= 0.01 {
            let (px, py) = (cx - 0.0, cy - 0.0);
            // canto reto: o ponto é o próprio canto
            let (qx, qy) = match start {
                s if (s - std::f32::consts::PI).abs() < 1e-3 => (x, y),
                s if (s - 1.5 * std::f32::consts::PI).abs() < 1e-3 => (x + w, y),
                s if s.abs() < 1e-3 => (x + w, y + h),
                _ => (x, y + h),
            };
            let _ = (px, py);
            if first {
                pb.move_to(qx, qy);
                first = false;
            } else {
                pb.line_to(qx, qy);
            }
            continue;
        }
        for i in 0..=SEG {
            let t = i as f32 / SEG as f32 * std::f32::consts::FRAC_PI_2;
            let (c, s) = (t.cos(), t.sin());
            // superelipse: |x|^N + |y|^N = 1
            let ex = c.abs().powf(2.0 / N) * c.signum();
            let ey = s.abs().powf(2.0 / N) * s.signum();
            // rotaciona pelo ângulo inicial do canto
            let (sa, ca) = start.sin_cos();
            let (lx, ly) = (ex * ca - ey * sa, ex * sa + ey * ca);
            let (px, py) = (cx + lx * rad, cy + ly * rad);
            if first {
                pb.move_to(px, py);
                first = false;
            } else {
                pb.line_to(px, py);
            }
        }
    }
    pb.close();
    pb.finish()
}

fn paint(c: Rgba) -> Paint<'static> {
    let mut p = Paint::default();
    p.set_color(c.to_skia());
    p.anti_alias = true;
    p
}

impl Canvas {
    pub fn new(w: i32, h: i32, scale: f32) -> Option<Self> {
        let pm = Pixmap::new(((w as f32) * scale).ceil().max(1.0) as u32, ((h as f32) * scale).ceil().max(1.0) as u32)?;
        Some(Canvas { pm, scale })
    }

    pub fn clear(&mut self) {
        self.pm.fill(Color::TRANSPARENT);
    }

    fn tf(&self) -> Transform {
        Transform::from_scale(self.scale, self.scale)
    }

    pub fn fill_rrect(&mut self, x: f32, y: f32, w: f32, h: f32, r: f32, color: Rgba) {
        self.fill_rrect_corners(x, y, w, h, [r; 4], color);
    }

    pub fn fill_rrect_corners(&mut self, x: f32, y: f32, w: f32, h: f32, r: [f32; 4], color: Rgba) {
        if let Some(path) = rrect_path(x, y, w, h, r) {
            self.pm.fill_path(&path, &paint(color), FillRule::Winding, self.tf(), None);
        }
    }

    /// Preenchimento com gradiente vertical (brilho de "vidro").
    pub fn fill_rrect_vgradient(&mut self, x: f32, y: f32, w: f32, h: f32, r: f32, top: Rgba, bottom: Rgba) {
        if let Some(path) = rrect_path(x, y, w, h, [r; 4]) {
            let shader = LinearGradient::new(
                Point::from_xy(x, y),
                Point::from_xy(x, y + h),
                vec![GradientStop::new(0.0, top.to_skia()), GradientStop::new(1.0, bottom.to_skia())],
                SpreadMode::Pad,
                Transform::identity(),
            );
            if let Some(shader) = shader {
                let mut p = Paint::default();
                p.shader = shader;
                p.anti_alias = true;
                self.pm.fill_path(&path, &p, FillRule::Winding, self.tf(), None);
            }
        }
    }

    pub fn stroke_rrect(&mut self, x: f32, y: f32, w: f32, h: f32, r: f32, width: f32, color: Rgba) {
        // traço centrado na borda: deslocamos meia largura para dentro
        let o = width / 2.0;
        if let Some(path) = rrect_path(x + o, y + o, w - width, h - width, [(r - o).max(0.0); 4]) {
            let stroke = Stroke { width, ..Stroke::default() };
            self.pm.stroke_path(&path, &paint(color), &stroke, self.tf(), None);
        }
    }

    pub fn fill_circle(&mut self, cx: f32, cy: f32, r: f32, color: Rgba) {
        if let Some(path) = PathBuilder::from_circle(cx, cy, r) {
            self.pm.fill_path(&path, &paint(color), FillRule::Winding, self.tf(), None);
        }
    }

    pub fn stroke_circle_outline(&mut self, cx: f32, cy: f32, r: f32) {
        if let Some(path) = PathBuilder::from_circle(cx, cy, r - 0.25) {
            let stroke = Stroke { width: 0.5, ..Stroke::default() };
            self.pm.stroke_path(&path, &paint(Rgba(0, 0, 0, 55)), &stroke, self.tf(), None);
        }
    }

    pub fn fill_path(&mut self, path: &Path, color: Rgba) {
        self.pm.fill_path(path, &paint(color), FillRule::Winding, self.tf(), None);
    }

    pub fn stroke_path_with(&mut self, path: &Path, stroke: &Stroke, color: Rgba) {
        self.pm.stroke_path(path, &paint(color), stroke, self.tf(), None);
    }

    pub fn stroke_path(&mut self, path: &Path, width: f32, color: Rgba) {
        let stroke = Stroke { width, line_cap: resvg::tiny_skia::LineCap::Round, ..Stroke::default() };
        self.pm.stroke_path(path, &paint(color), &stroke, self.tf(), None);
    }

    /// Vidro: preenche o retângulo com o recorte correspondente do fundo desfocado.
    /// `origin` = posição lógica deste canvas na tela; `screen` = tamanho lógico da tela.
    #[allow(clippy::too_many_arguments)]
    pub fn fill_rrect_backdrop(
        &mut self,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        r: f32,
        backdrop: &Pixmap,
        origin: (f32, f32),
        screen: (f32, f32),
    ) {
        let Some(path) = rrect_path(x, y, w, h, [r; 4]) else { return };
        let (bw, bh) = (backdrop.width() as f32, backdrop.height() as f32);
        let tf = Transform::from_row(screen.0 / bw, 0.0, 0.0, screen.1 / bh, -origin.0, -origin.1);
        let mut p = Paint::default();
        p.shader = Pattern::new(backdrop.as_ref(), SpreadMode::Pad, FilterQuality::Bilinear, 1.0, tf);
        p.anti_alias = true;
        self.pm.fill_path(&path, &p, FillRule::Winding, self.tf(), None);
    }

    /// Sombra suave: retângulo preto desfocado (3 passes de box blur ≈ gaussiana).
    /// Deve ser chamada antes de qualquer outro desenho (opera sobre o canvas inteiro).
    pub fn shadow_rrect(&mut self, x: f32, y: f32, w: f32, h: f32, r: f32, spread: f32, alpha: u8) {
        self.fill_rrect(x, y, w, h, r, Rgba(0, 0, 0, alpha));
        let radius = ((spread / 3.0) * self.scale / 1.05).round().max(1.0) as usize;
        blur_alpha(&mut self.pm, radius);
    }

    /// Sombra local: desfoca num pixmap à parte e compõe por cima (não afeta o resto).
    pub fn shadow_rrect_add(&mut self, x: f32, y: f32, w: f32, h: f32, r: f32, spread: f32, alpha: u8) {
        if alpha == 0 || w <= 1.0 || h <= 1.0 {
            return;
        }
        let pad = spread.ceil();
        let Some(mut tmp) = Canvas::new((w + 2.0 * pad) as i32, (h + 2.0 * pad) as i32, self.scale) else { return };
        tmp.shadow_rrect(pad, pad, w, h, r, spread, alpha);
        let pp = PixmapPaint::default();
        let tf = Transform::from_translate((x - pad) * self.scale, (y - pad) * self.scale);
        self.pm.draw_pixmap(0, 0, tmp.pm.as_ref(), &pp, tf, None);
    }

    /// Brilho colorido (sombra na cor `color`) atrás de um retângulo arredondado.
    pub fn glow_rrect_add(&mut self, x: f32, y: f32, w: f32, h: f32, r: f32, spread: f32, color: Rgba) {
        let pad = spread.ceil();
        let Some(mut tmp) = Canvas::new((w + 2.0 * pad) as i32, (h + 2.0 * pad) as i32, self.scale) else { return };
        tmp.fill_rrect(pad, pad, w, h, r, color);
        let radius = ((spread / 3.0) * self.scale).round().max(1.0) as usize;
        blur_rgba(&mut tmp.pm, radius, 1.0);
        let tf = Transform::from_translate((x - pad) * self.scale, (y - pad) * self.scale);
        self.pm.draw_pixmap(0, 0, tmp.pm.as_ref(), &PixmapPaint::default(), tf, None);
    }

    /// Como `blit`, recortado num retângulo arredondado de raio `r` (cobre a área, sem distorcer).
    pub fn blit_rounded(&mut self, src: &Pixmap, x: f32, y: f32, w: f32, h: f32, r: f32, opacity: f32) {
        if src.width() == 0 || src.height() == 0 {
            return;
        }
        let Some(mut mask) = resvg::tiny_skia::Mask::new(self.pm.width(), self.pm.height()) else { return };
        if let Some(p) = rrect_path(x, y, w, h, [r; 4]) {
            mask.fill_path(&p, FillRule::Winding, true, self.tf());
        }
        let s = (w * self.scale / src.width() as f32).max(h * self.scale / src.height() as f32);
        let (dx, dy) = ((w * self.scale - src.width() as f32 * s) / 2.0, (h * self.scale - src.height() as f32 * s) / 2.0);
        let mut pp = PixmapPaint::default();
        pp.opacity = opacity;
        pp.quality = FilterQuality::Bicubic;
        let tf = Transform::from_row(s, 0.0, 0.0, s, x * self.scale + dx, y * self.scale + dy);
        self.pm.draw_pixmap(0, 0, src.as_ref(), &pp, tf, Some(&mask));
    }

    /// Cola `src` (já em px do buffer) em (x,y) lógicos, escalado para w×h lógicos.
    pub fn blit(&mut self, src: &Pixmap, x: f32, y: f32, w: f32, h: f32, opacity: f32) {
        if src.width() == 0 || src.height() == 0 {
            return;
        }
        let sx = w * self.scale / src.width() as f32;
        let sy = h * self.scale / src.height() as f32;
        let mut pp = PixmapPaint::default();
        pp.opacity = opacity;
        pp.quality = FilterQuality::Bicubic;
        let tf = Transform::from_row(sx, 0.0, 0.0, sy, x * self.scale, y * self.scale);
        self.pm.draw_pixmap(0, 0, src.as_ref(), &pp, tf, None);
    }

    /// Texto: `y` = topo da caixa de linha (px lógicos). Devolve largura lógica.
    pub fn text(&mut self, s: &str, x: f32, y: f32, size: f32, weight: Weight, color: Rgba) -> f32 {
        let px = size * self.scale;
        let (asc, _) = text::line_metrics(px, weight);
        let w = text::draw(&mut self.pm, s, x * self.scale, y * self.scale + asc, px, weight, color);
        w / self.scale
    }

    pub fn measure(&self, s: &str, size: f32, weight: Weight) -> f32 {
        text::measure(s, size * self.scale, weight) / self.scale
    }

    /// Texto centralizado verticalmente em [y, y+h].
    pub fn text_vcenter(&mut self, s: &str, x: f32, y: f32, h: f32, size: f32, weight: Weight, color: Rgba) -> f32 {
        let px = size * self.scale;
        let (asc, desc) = text::line_metrics(px, weight);
        let line_h = (asc - desc) / self.scale;
        self.text(s, x, y + (h - line_h) / 2.0, size, weight, color)
    }

    pub fn ellipsized(&self, s: &str, size: f32, weight: Weight, max_w: f32) -> String {
        text::ellipsize(s, size * self.scale, weight, max_w * self.scale)
    }
}

/// Desfoca só o canal alfa (a sombra é preta, então o RGB pré-multiplicado é 0).
fn blur_alpha(pm: &mut Pixmap, radius: usize) {
    let (w, h) = (pm.width() as usize, pm.height() as usize);
    let data = pm.data_mut();
    let mut a: Vec<f32> = data.chunks_exact(4).map(|p| p[3] as f32).collect();
    let mut tmp = vec![0f32; a.len()];
    for _ in 0..3 {
        box_blur_h(&a, &mut tmp, w, h, radius);
        box_blur_v(&tmp, &mut a, w, h, radius);
    }
    for (px, v) in data.chunks_exact_mut(4).zip(a.iter()) {
        px[0] = 0;
        px[1] = 0;
        px[2] = 0;
        px[3] = v.round().clamp(0.0, 255.0) as u8;
    }
}

fn box_blur_h(src: &[f32], dst: &mut [f32], w: usize, h: usize, r: usize) {
    let norm = 1.0 / (2 * r + 1) as f32;
    for y in 0..h {
        let row = &src[y * w..(y + 1) * w];
        let out = &mut dst[y * w..(y + 1) * w];
        let mut sum = 0.0;
        for i in 0..=r.min(w - 1) {
            sum += row[i];
        }
        for x in 0..w {
            out[x] = sum * norm;
            let add = x + r + 1;
            let sub = x as isize - r as isize;
            if add < w {
                sum += row[add];
            }
            if sub >= 0 {
                sum -= row[sub as usize];
            }
        }
    }
}

fn box_blur_v(src: &[f32], dst: &mut [f32], w: usize, h: usize, r: usize) {
    let norm = 1.0 / (2 * r + 1) as f32;
    for x in 0..w {
        let mut sum = 0.0;
        for i in 0..=r.min(h - 1) {
            sum += src[i * w + x];
        }
        for y in 0..h {
            dst[y * w + x] = sum * norm;
            let add = y + r + 1;
            let sub = y as isize - r as isize;
            if add < h {
                sum += src[add * w + x];
            }
            if sub >= 0 {
                sum -= src[sub as usize * w + x];
            }
        }
    }
}

/// Desfoque RGBA (pré-multiplicado) + reforço de saturação, para o material de vidro.
pub fn blur_rgba(pm: &mut Pixmap, radius: usize, saturation: f32) {
    let (w, h) = (pm.width() as usize, pm.height() as usize);
    let data = pm.data_mut();
    for ch in 0..4 {
        let mut a: Vec<f32> = data.chunks_exact(4).map(|p| p[ch] as f32).collect();
        let mut tmp = vec![0f32; a.len()];
        for _ in 0..3 {
            box_blur_h(&a, &mut tmp, w, h, radius);
            box_blur_v(&tmp, &mut a, w, h, radius);
        }
        for (px, v) in data.chunks_exact_mut(4).zip(a.iter()) {
            px[ch] = v.round().clamp(0.0, 255.0) as u8;
        }
    }
    if (saturation - 1.0).abs() > 0.01 {
        for px in data.chunks_exact_mut(4) {
            let (r, g, b) = (px[0] as f32, px[1] as f32, px[2] as f32);
            let l = 0.2126 * r + 0.7152 * g + 0.0722 * b;
            let a = px[3] as f32;
            for (i, c) in [r, g, b].into_iter().enumerate() {
                px[i] = (l + (c - l) * saturation).clamp(0.0, a) as u8;
            }
        }
    }
}

impl Canvas {
    /// Desenha um símbolo centralizado no ponto (cx, cy), com `size` px lógicos.
    pub fn symbol(&mut self, name: &str, cx: f32, cy: f32, size: f32, color: Rgba) {
        let g = super::symbols::sym(name);
        let w = self.measure(&g, size, Weight::Symbol);
        let px = size * self.scale;
        let (asc, desc) = text::line_metrics(px, Weight::Symbol);
        let line_h = (asc - desc) / self.scale;
        self.text(&g, cx - w / 2.0, cy - line_h / 2.0, size, Weight::Symbol, color);
    }
}

pub fn blur_alpha_pub(pm: &mut Pixmap, radius: usize) {
    blur_alpha(pm, radius.max(1));
}
