//! Protocolos que apps comuns esperam de um desktop completo:
//! - xdg-activation: link/notificação traz a janela do app para a frente;
//! - fractional-scale: escala 1,25/1,5 nítida nos apps que suportam;
//! - presentation-time: apps (vídeo, jogos) sabem quando o quadro apareceu;
//! - keyboard-shortcuts-inhibit: VMs e acesso remoto recebem os atalhos;
//! - text-input + input-method: teclados de japonês/chinês/coreano (fcitx5, ibus);
//! - security-context: apps em sandbox (Flatpak) ficam sem acesso a globais sensíveis;
//! - single-pixel-buffer e xdg-foreign: usados por toolkits e pelos diálogos dos portais.

use std::sync::Arc;

use smithay::{
    delegate_fractional_scale, delegate_input_method_manager, delegate_keyboard_shortcuts_inhibit, delegate_presentation,
    delegate_security_context, delegate_single_pixel_buffer, delegate_text_input_manager, delegate_xdg_activation,
    delegate_xdg_foreign,
    desktop::PopupKind,
    input::Seat,
    reexports::wayland_server::protocol::wl_surface::WlSurface,
    utils::{Logical, Rectangle},
    wayland::{
        compositor::{send_surface_state, with_states},
        fractional_scale::{with_fractional_scale, FractionalScaleHandler},
        input_method::{InputMethodHandler, PopupSurface},
        keyboard_shortcuts_inhibit::{KeyboardShortcutsInhibitHandler, KeyboardShortcutsInhibitState, KeyboardShortcutsInhibitor},
        security_context::{SecurityContext, SecurityContextHandler, SecurityContextListenerSource},
        xdg_activation::{XdgActivationHandler, XdgActivationState, XdgActivationToken, XdgActivationTokenData},
        xdg_foreign::{XdgForeignHandler, XdgForeignState},
    },
};

use crate::state::{ClientState, Obelisk};

/// Estado dos protocolos que guardam dados.
pub struct ExtraProtocols {
    pub activation: XdgActivationState,
    pub shortcuts_inhibit: KeyboardShortcutsInhibitState,
    pub foreign: XdgForeignState,
}

impl ExtraProtocols {
    pub fn new(dh: &smithay::reexports::wayland_server::DisplayHandle, clock_id: u32) -> Self {
        smithay::wayland::fractional_scale::FractionalScaleManagerState::new::<Obelisk>(dh);
        smithay::wayland::presentation::PresentationState::new::<Obelisk>(dh, clock_id);
        smithay::wayland::text_input::TextInputManagerState::new::<Obelisk>(dh);
        smithay::wayland::input_method::InputMethodManagerState::new::<Obelisk, _>(dh, |_client| true);
        // sandbox não cria outro contexto de sandbox
        smithay::wayland::security_context::SecurityContextState::new::<Obelisk, _>(dh, |client| {
            client.get_data::<ClientState>().is_none_or(|c| c.security_context.is_none())
        });
        smithay::wayland::single_pixel_buffer::SinglePixelBufferState::new::<Obelisk>(dh);
        ExtraProtocols {
            activation: XdgActivationState::new::<Obelisk>(dh),
            shortcuts_inhibit: KeyboardShortcutsInhibitState::new::<Obelisk>(dh),
            foreign: XdgForeignState::new::<Obelisk>(dh),
        }
    }
}

// ---------------------------------------------------------------- fractional-scale
impl FractionalScaleHandler for Obelisk {
    fn new_fractional_scale(&mut self, surface: WlSurface) {
        let scale = self.fscale;
        with_states(&surface, |states| {
            with_fractional_scale(states, |fs| fs.set_preferred_scale(scale));
        });
    }
}
delegate_fractional_scale!(Obelisk);

impl Obelisk {
    /// Avisa todas as janelas da escala atual (depois de mudar a escala da tela).
    pub fn send_fractional_scale(&mut self) {
        let scale = self.fscale;
        let int = self.scale;
        for w in self.space.elements() {
            if let Some(t) = w.toplevel() {
                with_states(t.wl_surface(), |states| {
                    with_fractional_scale(states, |fs| fs.set_preferred_scale(scale));
                    send_surface_state(t.wl_surface(), states, int, smithay::utils::Transform::Normal);
                });
            }
        }
    }
}

// ---------------------------------------------------------------- presentation-time
delegate_presentation!(Obelisk);

// ---------------------------------------------------------------- xdg-activation
impl XdgActivationHandler for Obelisk {
    fn activation_state(&mut self) -> &mut XdgActivationState {
        &mut self.protocols.activation
    }

    /// Token novo: só vale se veio de um app com o teclado (o foco atual), como no GNOME/KDE.
    fn token_created(&mut self, _token: XdgActivationToken, data: XdgActivationTokenData) -> bool {
        let Some((serial, seat)) = data.serial else { return true };
        let Some(seat) = Seat::<Obelisk>::from_resource(&seat) else { return true };
        seat.get_keyboard().is_some_and(|k| k.last_enter().is_none_or(|e| serial.is_no_older_than(&e)))
    }

    fn request_activation(&mut self, token: XdgActivationToken, data: XdgActivationTokenData, surface: WlSurface) {
        // tokens velhos (mais de 10 s) não roubam o foco: a janela só pula no Dock
        let fresh = data.timestamp.elapsed().as_secs() < 10;
        let window = self.window_of(&surface);
        if let Some(w) = window {
            if fresh {
                self.activate_window(&w);
            } else {
                let id = Self::app_id(&w);
                if let Some(i) = self.shell.dock.entries.iter().position(|e| matches!(e, crate::shell::dock::Entry::App { app, .. } if app.id == id || app.wm_class == id)) {
                    self.shell.dock.start_bounce(i);
                }
            }
        }
        self.protocols.activation.remove_token(&token);
        self.dirty = true;
    }
}
delegate_xdg_activation!(Obelisk);

// ---------------------------------------------------------------- keyboard-shortcuts-inhibit
impl KeyboardShortcutsInhibitHandler for Obelisk {
    fn keyboard_shortcuts_inhibit_state(&mut self) -> &mut KeyboardShortcutsInhibitState {
        &mut self.protocols.shortcuts_inhibit
    }

    /// VM / acesso remoto pediu os atalhos: concede (⌥⌘⎋ e o bloqueio continuam do sistema).
    fn new_inhibitor(&mut self, inhibitor: KeyboardShortcutsInhibitor) {
        inhibitor.activate();
    }
}
delegate_keyboard_shortcuts_inhibit!(Obelisk);

impl Obelisk {
    /// O app em foco pediu para receber todos os atalhos?
    pub fn shortcuts_inhibited(&self) -> bool {
        use smithay::wayland::keyboard_shortcuts_inhibit::KeyboardShortcutsInhibitorSeat;
        self.seat.keyboard_shortcuts_inhibited()
    }
}

// ---------------------------------------------------------------- text-input / input-method
delegate_text_input_manager!(Obelisk);

impl InputMethodHandler for Obelisk {
    fn new_popup(&mut self, surface: PopupSurface) {
        if let Err(e) = self.popups.track_popup(PopupKind::from(surface)) {
            tracing::warn!("popup do método de entrada: {e}");
        }
    }

    fn dismiss_popup(&mut self, surface: PopupSurface) {
        if let Some(parent) = surface.get_parent().map(|p| p.surface.clone()) {
            let _ = smithay::desktop::PopupManager::dismiss_popup(&parent, &PopupKind::from(surface));
        }
    }

    fn popup_repositioned(&mut self, _: PopupSurface) {}

    fn parent_geometry(&self, parent: &WlSurface) -> Rectangle<i32, Logical> {
        self.window_of(parent).map(|w| w.geometry()).unwrap_or_default()
    }
}
delegate_input_method_manager!(Obelisk);

// ---------------------------------------------------------------- security-context (Flatpak)
impl SecurityContextHandler for Obelisk {
    fn context_created(&mut self, source: SecurityContextListenerSource, context: SecurityContext) {
        let context = Arc::new(context);
        let res = self.loop_handle.insert_source(source, move |stream, _, state| {
            let client_state = ClientState { security_context: Some(context.clone()), ..Default::default() };
            if let Err(e) = state.display_handle.insert_client(stream, Arc::new(client_state)) {
                tracing::warn!("cliente em sandbox: {e}");
            }
        });
        if let Err(e) = res {
            tracing::warn!("security-context: {e}");
        }
    }
}
delegate_security_context!(Obelisk);

// ---------------------------------------------------------------- single-pixel-buffer / xdg-foreign
delegate_single_pixel_buffer!(Obelisk);

impl XdgForeignHandler for Obelisk {
    fn xdg_foreign_state(&mut self) -> &mut XdgForeignState {
        &mut self.protocols.foreign
    }
}
delegate_xdg_foreign!(Obelisk);
