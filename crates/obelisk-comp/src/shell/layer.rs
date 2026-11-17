//! Uma "camada" de UI: um pixmap desenhado na CPU e enviado como textura.

use smithay::{
    backend::allocator::Fourcc,
    backend::renderer::{
        element::{memory::MemoryRenderBuffer, memory::MemoryRenderBufferRenderElement, Kind},
        gles::GlesRenderer,
    },
    utils::{Buffer, Point, Rectangle, Transform},
};

use crate::ui::canvas::Canvas;

#[derive(Default)]
pub struct Layer {
    buf: Option<MemoryRenderBuffer>,
    size: (i32, i32),
    scale: i32,
}

impl Layer {
    /// Envia o conteúdo do canvas para a textura (recria o buffer se o tamanho mudou).
    pub fn set(&mut self, canvas: &Canvas) {
        let (w, h) = (canvas.pm.width() as i32, canvas.pm.height() as i32);
        let scale = canvas.scale as i32;
        if self.buf.is_some() && self.size == (w, h) && self.scale == scale {
            let buf = self.buf.as_mut().unwrap();
            let mut ctx = buf.render();
            let _ = ctx.draw(|mem| {
                mem.copy_from_slice(canvas.pm.data());
                Ok::<_, ()>(vec![Rectangle::<i32, Buffer>::from_size((w, h).into())])
            });
        } else {
            self.buf = Some(MemoryRenderBuffer::from_slice(
                canvas.pm.data(),
                Fourcc::Abgr8888,
                (w, h),
                scale.max(1),
                Transform::Normal,
                None,
            ));
            self.size = (w, h);
            self.scale = scale;
        }
    }

    /// Envia um pixmap já em pixels do buffer.
    pub fn set_pixmap(&mut self, pm: &resvg::tiny_skia::Pixmap, scale: i32) {
        let c = Canvas { pm: pm.clone(), scale: scale as f32 };
        self.set(&c);
    }

    pub fn is_set(&self) -> bool {
        self.buf.is_some()
    }

    /// Elemento de render na posição física `pos`.
    pub fn element(
        &self,
        renderer: &mut GlesRenderer,
        pos: Point<f64, smithay::utils::Physical>,
        alpha: f32,
    ) -> Option<MemoryRenderBufferRenderElement<GlesRenderer>> {
        let buf = self.buf.as_ref()?;
        MemoryRenderBufferRenderElement::from_buffer(renderer, pos, buf, Some(alpha), None, None, Kind::Unspecified).ok()
    }
}
