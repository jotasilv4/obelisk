//! Captura de tela feita pelo próprio compositor: lê o framebuffer e salva PNG.

use smithay::{
    backend::{
        allocator::Fourcc,
        renderer::{gles::GlesRenderer, ExportMem},
    },
    utils::{Physical, Rectangle, Size},
};

/// Renderiza a cena numa textura fora da tela e lê `crop` (px físicos) como RGBA, já na orientação
/// certa. Não toca na superfície da janela/tela (ler dela quebra a EGLSurface do backend aninhado).
pub fn offscreen(
    renderer: &mut GlesRenderer,
    elements: &[crate::render::ObeliskElement],
    size: Size<i32, Physical>,
    scale: f64,
    crop: Rectangle<i32, Physical>,
) -> Option<Vec<u8>> {
    use smithay::backend::renderer::{damage::OutputDamageTracker, gles::GlesTexture, Bind, Offscreen};
    let mut tex: GlesTexture = renderer.create_buffer(Fourcc::Abgr8888, (size.w, size.h).into()).ok()?;
    let mut fb = renderer.bind(&mut tex).ok()?;
    let mut dt = OutputDamageTracker::new(size, scale, smithay::utils::Transform::Normal);
    dt.render_output(renderer, &mut fb, 0, elements, [0.0, 0.0, 0.0, 1.0]).ok()?;
    let full = Rectangle::<i32, Physical>::from_size(size);
    let c = crop.intersection(full)?;
    let region = Rectangle::new((c.loc.x, c.loc.y).into(), (c.size.w, c.size.h).into());
    let mapping = renderer.copy_framebuffer(&fb, region, Fourcc::Abgr8888).ok()?;
    let data = renderer.map_texture(&mapping).ok()?;
    let mut out = data.to_vec();
    for px in out.chunks_exact_mut(4) {
        px[3] = 255;
    }
    Some(out)
}

pub fn save_rgba(data: Vec<u8>, w: i32, h: i32, path: &std::path::Path) -> Result<(), String> {
    let pm = resvg::tiny_skia::Pixmap::from_vec(data, resvg::tiny_skia::IntSize::from_wh(w as u32, h as u32).ok_or("tamanho")?)
        .ok_or("pixmap")?;
    pm.save_png(path).map_err(|e| e.to_string())
}
