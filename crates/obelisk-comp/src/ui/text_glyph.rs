//! Pequenos glifos vetoriais (×, −, +, lupa, logo) desenhados em código.

use resvg::tiny_skia::PathBuilder;

use super::canvas::Canvas;
use super::theme::Rgba;

pub fn glyph_color() -> Rgba {
    Rgba(0, 0, 0, 140)
}

pub fn cross(c: &mut Canvas, cx: f32, cy: f32, r: f32, color: Rgba) {
    let mut pb = PathBuilder::new();
    pb.move_to(cx - r, cy - r);
    pb.line_to(cx + r, cy + r);
    pb.move_to(cx + r, cy - r);
    pb.line_to(cx - r, cy + r);
    if let Some(p) = pb.finish() {
        c.stroke_path(&p, 1.3, color);
    }
}

pub fn minus(c: &mut Canvas, cx: f32, cy: f32, r: f32, color: Rgba) {
    let mut pb = PathBuilder::new();
    pb.move_to(cx - r, cy);
    pb.line_to(cx + r, cy);
    if let Some(p) = pb.finish() {
        c.stroke_path(&p, 1.3, color);
    }
}

pub fn plus(c: &mut Canvas, cx: f32, cy: f32, r: f32, color: Rgba) {
    let mut pb = PathBuilder::new();
    pb.move_to(cx - r, cy);
    pb.line_to(cx + r, cy);
    pb.move_to(cx, cy - r);
    pb.line_to(cx, cy + r);
    if let Some(p) = pb.finish() {
        c.stroke_path(&p, 1.3, color);
    }
}

/// Marca do Obelisk (pilar afunilado + pirâmide) para a barra de menus.
/// Símbolo do Obelisk (branding/obelisk-symbol.svg: hexágono de quatro peças), centrado em
/// (cx, cy) num quadrado de lado `size`.
pub fn obelisk_logo(c: &mut Canvas, cx: f32, cy: f32, size: f32, color: Rgba) {
    const SVG: &str = include_str!("../../../../branding/obelisk-symbol.svg");
    thread_local! {
        static PATH: Option<resvg::tiny_skia::Path> = parse_ml(SVG);
    }
    PATH.with(|p| {
        let Some(p) = p else { return };
        // o desenho ocupa ~x 11.9..88.1, y 6.4..93.6 do viewBox 100
        let s = size / 87.2;
        let t = resvg::tiny_skia::Transform::from_row(s, 0.0, 0.0, s, cx - 50.0 * s, cy - 50.0 * s);
        if let Some(p) = p.clone().transform(t) {
            c.fill_path(&p, color);
        }
    });
}

/// Lê o atributo `d` (só M/L/Z) do SVG.
fn parse_ml(svg: &str) -> Option<resvg::tiny_skia::Path> {
    let d = svg.split(" d=\"").nth(1)?.split('"').next()?;
    let mut pb = PathBuilder::new();
    let mut toks = d.split_whitespace().peekable();
    while let Some(t) = toks.next() {
        let (cmd, rest) = t.split_at(1);
        match cmd {
            "M" | "L" => {
                let x: f32 = rest.parse().ok()?;
                let y: f32 = toks.next()?.parse().ok()?;
                if cmd == "M" { pb.move_to(x, y) } else { pb.line_to(x, y) }
            }
            "Z" => pb.close(),
            _ => {}
        }
    }
    pb.finish()
}

/// Cursor em seta (branco com contorno escuro).
pub fn arrow_cursor(c: &mut Canvas, x: f32, y: f32) {
    let mut pb = PathBuilder::new();
    pb.move_to(x + 1.0, y + 1.0);
    pb.line_to(x + 1.0, y + 17.0);
    pb.line_to(x + 5.2, y + 13.2);
    pb.line_to(x + 8.2, y + 19.6);
    pb.line_to(x + 10.6, y + 18.5);
    pb.line_to(x + 7.7, y + 12.3);
    pb.line_to(x + 13.2, y + 12.3);
    pb.close();
    if let Some(p) = pb.finish() {
        c.stroke_path(&p, 2.2, Rgba(0, 0, 0, 230));
        c.fill_path(&p, Rgba(255, 255, 255, 255));
    }
}
