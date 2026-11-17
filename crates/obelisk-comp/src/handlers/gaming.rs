//! Protocolos que jogos e apps de GPU precisam:
//! - linux-dmabuf: buffers de GPU (OpenGL/Vulkan) sem cópia pela CPU;
//! - relative-pointer + pointer-constraints: mira com mouse em jogos (trava/confina o ponteiro);
//! - content-type: o app diz "sou um jogo" → liga o Modo Jogo;
//! - idle-inhibit: jogo/vídeo impede o bloqueio por inatividade;
//! - viewporter: escala de vídeo/jogo feita pelo compositor;
//! - primary selection: colar com o botão do meio.

use smithay::{
    backend::allocator::dmabuf::Dmabuf,
    delegate_dmabuf, delegate_idle_inhibit, delegate_pointer_constraints, delegate_primary_selection,
    delegate_relative_pointer, delegate_viewporter, delegate_content_type,
    input::pointer::PointerHandle,
    reexports::wayland_server::protocol::wl_surface::WlSurface,
    utils::{Logical, Point},
    wayland::{
        dmabuf::{DmabufGlobal, DmabufHandler, DmabufState, ImportNotifier},
        idle_inhibit::IdleInhibitHandler,
        pointer_constraints::{with_pointer_constraint, PointerConstraintsHandler},
        selection::primary_selection::{PrimarySelectionHandler, PrimarySelectionState},
    },
};

use crate::state::Obelisk;

impl DmabufHandler for Obelisk {
    fn dmabuf_state(&mut self) -> &mut DmabufState {
        &mut self.dmabuf_state
    }

    fn dmabuf_imported(&mut self, _global: &DmabufGlobal, _dmabuf: Dmabuf, notifier: ImportNotifier) {
        // a importação real acontece no render (GlesRenderer); buffers inválidos são ignorados lá
        let _ = notifier.successful::<Obelisk>();
    }
}
delegate_dmabuf!(Obelisk);

delegate_relative_pointer!(Obelisk);

impl PointerConstraintsHandler for Obelisk {
    fn new_constraint(&mut self, surface: &WlSurface, pointer: &PointerHandle<Self>) {
        if pointer.current_focus().as_ref() == Some(surface) {
            with_pointer_constraint(surface, pointer, |c| {
                if let Some(c) = c {
                    c.activate();
                }
            });
        }
    }

    fn cursor_position_hint(&mut self, surface: &WlSurface, pointer: &PointerHandle<Self>, location: Point<f64, Logical>) {
        if with_pointer_constraint(surface, pointer, |c| c.is_some_and(|c| c.is_active())) {
            if let Some(w) = self.window_of(surface) {
                if let Some(loc) = self.space.element_location(&w) {
                    pointer.set_location(loc.to_f64() + location);
                }
            }
        }
    }
}
delegate_pointer_constraints!(Obelisk);

impl IdleInhibitHandler for Obelisk {
    fn inhibit(&mut self, surface: WlSurface) {
        self.idle_inhibitors.insert(surface);
    }
    fn uninhibit(&mut self, surface: WlSurface) {
        self.idle_inhibitors.remove(&surface);
    }
}
delegate_idle_inhibit!(Obelisk);

delegate_content_type!(Obelisk);
delegate_viewporter!(Obelisk);

impl PrimarySelectionHandler for Obelisk {
    fn primary_selection_state(&self) -> &PrimarySelectionState {
        &self.primary_selection_state
    }
}
delegate_primary_selection!(Obelisk);

/// A janela declara que é um jogo (wp_content_type_v1 = game)?
pub fn is_game_content(surface: &WlSurface) -> bool {
    use smithay::reexports::wayland_protocols::wp::content_type::v1::server::wp_content_type_v1::Type;
    use smithay::wayland::{compositor::with_states, content_type::ContentTypeSurfaceCachedState};
    with_states(surface, |s| {
        *s.cached_state.get::<ContentTypeSurfaceCachedState>().current().content_type() == Type::Game
    })
}
