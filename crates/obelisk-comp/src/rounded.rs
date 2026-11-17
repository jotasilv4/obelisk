//! Recorte de cantos arredondados nas janelas: um elemento que envolve a superfície do
//! cliente e a desenha com um shader que descarta os pixels fora do retângulo arredondado.

use smithay::{
    backend::renderer::{
        element::{
            surface::{WaylandSurfaceRenderElement, WaylandSurfaceTexture},
            Element, Id, Kind, RenderElement, UnderlyingStorage,
        },
        gles::{GlesError, GlesFrame, GlesRenderer, GlesTexProgram, Uniform, UniformName, UniformType},
        utils::{CommitCounter, DamageSet, OpaqueRegions},
        Texture,
    },
    utils::{Buffer, Physical, Point, Rectangle, Scale, Transform},
};

const FRAG: &str = r#"#version 100

//_DEFINES_

#if defined(EXTERNAL)
#extension GL_OES_EGL_image_external : require
#endif

precision highp float;
#if defined(EXTERNAL)
uniform samplerExternalOES tex;
#else
uniform sampler2D tex;
#endif

uniform float alpha;
varying vec2 v_coords;

uniform vec4 src_n;
uniform vec2 elem;
uniform vec4 rect;
uniform float radius;

#if defined(DEBUG_FLAGS)
uniform float tint;
#endif

float rbox(vec2 p, vec2 half_size, float r) {
    vec2 d = abs(p) - half_size + vec2(r);
    return length(max(d, vec2(0.0))) + min(max(d.x, d.y), 0.0) - r;
}

void main() {
    vec2 q = (v_coords - src_n.xy) / src_n.zw;
    vec2 p = q * elem;
    vec2 half_size = rect.zw * 0.5;
    float d = rbox(p - (rect.xy + half_size), half_size, radius);
    float m = clamp(0.5 - d, 0.0, 1.0);

    vec4 color = texture2D(tex, v_coords);
#if defined(NO_ALPHA)
    color = vec4(color.rgb, 1.0) * alpha;
#else
    color = color * alpha;
#endif

#if defined(DEBUG_FLAGS)
    if (tint == 1.0)
        color = vec4(0.0, 0.2, 0.0, 0.2) + color * 0.8;
#endif

    gl_FragColor = color * m;
}
"#;

pub fn compile(renderer: &mut GlesRenderer) -> Result<GlesTexProgram, GlesError> {
    renderer.compile_custom_texture_shader(
        FRAG,
        &[
            UniformName::new("src_n", UniformType::_4f),
            UniformName::new("elem", UniformType::_2f),
            UniformName::new("rect", UniformType::_4f),
            UniformName::new("radius", UniformType::_1f),
        ],
    )
}

pub struct RoundedSurface {
    inner: WaylandSurfaceRenderElement<GlesRenderer>,
    program: GlesTexProgram,
    /// Retângulo do quadro da janela em px físicos, relativo ao canto do elemento.
    rect: [f32; 4],
    radius: f32,
}

impl RoundedSurface {
    /// `frame_rect`: quadro da janela em px físicos absolutos (x, y, w, h).
    pub fn new(
        inner: WaylandSurfaceRenderElement<GlesRenderer>,
        program: GlesTexProgram,
        frame_rect: [f32; 4],
        radius: f32,
        scale: Scale<f64>,
    ) -> Self {
        let loc = inner.location(scale);
        let rect = [frame_rect[0] - loc.x as f32, frame_rect[1] - loc.y as f32, frame_rect[2], frame_rect[3]];
        RoundedSurface { inner, program, rect, radius }
    }
}

impl Element for RoundedSurface {
    fn id(&self) -> &Id {
        self.inner.id()
    }
    fn current_commit(&self) -> CommitCounter {
        self.inner.current_commit()
    }
    fn geometry(&self, scale: Scale<f64>) -> Rectangle<i32, Physical> {
        self.inner.geometry(scale)
    }
    fn transform(&self) -> Transform {
        self.inner.transform()
    }
    fn src(&self) -> Rectangle<f64, Buffer> {
        self.inner.src()
    }
    fn damage_since(&self, scale: Scale<f64>, commit: Option<CommitCounter>) -> DamageSet<i32, Physical> {
        self.inner.damage_since(scale, commit)
    }
    /// Os cantos ficam transparentes: não declaramos regiões opacas.
    fn opaque_regions(&self, _scale: Scale<f64>) -> OpaqueRegions<i32, Physical> {
        OpaqueRegions::default()
    }
    fn alpha(&self) -> f32 {
        self.inner.alpha()
    }
    fn kind(&self) -> Kind {
        self.inner.kind()
    }
    fn location(&self, scale: Scale<f64>) -> Point<i32, Physical> {
        self.inner.location(scale)
    }
}

impl RenderElement<GlesRenderer> for RoundedSurface {
    fn draw(
        &self,
        frame: &mut GlesFrame<'_, '_>,
        src: Rectangle<f64, Buffer>,
        dst: Rectangle<i32, Physical>,
        damage: &[Rectangle<i32, Physical>],
        opaque_regions: &[Rectangle<i32, Physical>],
    ) -> Result<(), GlesError> {
        match self.inner.texture() {
            WaylandSurfaceTexture::Texture(tex) => {
                let ts = tex.size();
                let (tw, th) = (ts.w as f64, ts.h as f64);
                let src_n = [(src.loc.x / tw) as f32, (src.loc.y / th) as f32, (src.size.w / tw) as f32, (src.size.h / th) as f32];
                let uniforms = [
                    Uniform::new("src_n", src_n),
                    Uniform::new("elem", [dst.size.w as f32, dst.size.h as f32]),
                    Uniform::new("rect", self.rect),
                    Uniform::new("radius", self.radius),
                ];
                frame.render_texture_from_to(
                    tex,
                    src,
                    dst,
                    damage,
                    opaque_regions,
                    self.inner.transform(),
                    self.inner.alpha(),
                    Some(&self.program),
                    &uniforms,
                )
            }
            WaylandSurfaceTexture::SolidColor(color) => {
                frame.draw_solid(dst, damage, *color * self.inner.alpha())
            }
        }
    }

    fn underlying_storage(&self, _renderer: &mut GlesRenderer) -> Option<UnderlyingStorage<'_>> {
        None
    }
}
