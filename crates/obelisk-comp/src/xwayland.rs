//! XWayland: apps X11 (Steam, jogos via Proton/Wine, launchers) dentro do Obelisk.

use smithay::{
    desktop::Window,
    utils::{Logical, Point, Rectangle},
    wayland::xwayland_shell::{XWaylandShellHandler, XWaylandShellState},
    xwayland::{
        xwm::{Reorder, ResizeEdge as X11ResizeEdge, XwmId},
        X11Surface, X11Wm, XWayland, XWaylandEvent, XwmHandler,
    },
};

use crate::state::{meta, Obelisk};
use crate::ui::theme::metrics;

impl XWaylandShellHandler for Obelisk {
    fn xwayland_shell_state(&mut self) -> &mut XWaylandShellState {
        &mut self.xwayland_shell_state
    }
}
smithay::delegate_xwayland_shell!(Obelisk);

impl Obelisk {
    pub fn start_xwayland(&mut self) {
        let spawned = XWayland::spawn(
            &self.display_handle,
            None,
            std::iter::empty::<(String, String)>(),
            true,
            std::process::Stdio::null(),
            std::process::Stdio::null(),
            |_| (),
        );
        let (xwayland, client) = match spawned {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!("XWayland indisponível (apps X11 não vão abrir): {e:?}");
                return;
            }
        };
        let res = self.loop_handle.insert_source(xwayland, move |event, _, state| match event {
            XWaylandEvent::Ready { x11_socket, display_number } => {
                match X11Wm::start_wm(state.loop_handle.clone(), x11_socket, client.clone()) {
                    Ok(wm) => {
                        state.xwm = Some(wm);
                        std::env::set_var("DISPLAY", format!(":{display_number}"));
                        tracing::info!("XWayland pronto em DISPLAY=:{display_number}");
                        if std::env::var_os("OBELISK_SESSION").is_some() {
                            crate::apps::spawn("dbus-update-activation-environment --systemd DISPLAY");
                        }
                    }
                    Err(e) => tracing::warn!("gerenciador X11 falhou: {e:?}"),
                }
            }
            XWaylandEvent::Error => tracing::warn!("XWayland caiu ao iniciar"),
        });
        if let Err(e) = res {
            tracing::warn!("XWayland: {e}");
        }
    }

    fn x11_window(&self, s: &X11Surface) -> Option<Window> {
        self.space
            .elements()
            .chain(self.minimized.iter())
            .find(|w| w.x11_surface() == Some(s))
            .cloned()
    }

    pub fn x11_fullscreen(&mut self, s: &X11Surface, on: bool) {
        let Some(w) = self.x11_window(s) else { return };
        let mut m = meta(&w).lock().unwrap();
        if on {
            if !m.fullscreen {
                m.saved_geo = Some(Rectangle::new(self.space.element_location(&w).unwrap_or_default(), w.geometry().size));
            }
            m.fullscreen = true;
            drop(m);
            let c = self.space.element_location(&w).unwrap_or_default();
            let r = self.output_rect_at(Point::from((c.x as f64 + w.geometry().size.w as f64 / 2.0, c.y as f64 + w.geometry().size.h as f64 / 2.0)));
            let _ = s.set_fullscreen(true);
            let _ = s.configure(r);
            self.space.map_element(w.clone(), r.loc, true);
            self.focus_window(&w);
        } else {
            m.fullscreen = false;
            let saved = m.saved_geo.take();
            drop(m);
            let _ = s.set_fullscreen(false);
            if let Some(r) = saved {
                let _ = s.configure(r);
                self.space.map_element(w.clone(), r.loc, true);
            }
        }
    }
}

impl XwmHandler for Obelisk {
    fn xwm_state(&mut self, _xwm: XwmId) -> &mut X11Wm {
        self.xwm.as_mut().expect("xwm")
    }

    fn new_window(&mut self, _xwm: XwmId, _window: X11Surface) {}
    fn new_override_redirect_window(&mut self, _xwm: XwmId, _window: X11Surface) {}

    fn map_window_request(&mut self, _xwm: XwmId, window: X11Surface) {
        let _ = window.set_mapped(true);
        let w = Window::new_x11_window(window.clone());
        // jogo que já pede a tela inteira (ou tem o tamanho dela) entra em tela cheia
        let geo = window.geometry();
        let n = self.space.elements().count() as i32;
        let loc: Point<i32, Logical> = (90 + (n % 8) * 30, metrics::MENUBAR_H + metrics::TITLEBAR_H + 40 + (n % 8) * 30).into();
        self.space.map_element(w.clone(), loc, true);
        meta(&w).lock().unwrap().opened_at = Some(std::time::Instant::now());
        let _ = window.configure(Rectangle::new(loc, geo.size));
        if window.is_fullscreen() || (geo.size.w >= self.screen.w && geo.size.h >= self.screen.h) {
            self.x11_fullscreen(&window, true);
        }
        self.focus_window(&w);
    }

    fn mapped_override_redirect_window(&mut self, _xwm: XwmId, window: X11Surface) {
        // menus, dicas e janelas sem moldura do X11: ficam onde o app mandou
        let loc = window.geometry().loc;
        let w = Window::new_x11_window(window);
        self.space.map_element(w, loc, true);
    }

    fn unmapped_window(&mut self, _xwm: XwmId, window: X11Surface) {
        if let Some(w) = self.x11_window(&window) {
            self.space.unmap_elem(&w);
            self.minimized.retain(|m| m != &w);
        }
        if !window.is_override_redirect() {
            let _ = window.set_mapped(false);
        }
        if self.focused_window().is_none() {
            if let Some(next) = self.top_window() {
                self.focus_window(&next);
            }
        }
    }

    fn destroyed_window(&mut self, _xwm: XwmId, _window: X11Surface) {}

    fn configure_request(
        &mut self,
        _xwm: XwmId,
        window: X11Surface,
        _x: Option<i32>,
        _y: Option<i32>,
        w: Option<u32>,
        h: Option<u32>,
        _reorder: Option<Reorder>,
    ) {
        let mut geo = window.geometry();
        if let Some(w) = w {
            geo.size.w = w as i32;
        }
        if let Some(h) = h {
            geo.size.h = h as i32;
        }
        let _ = window.configure(geo);
    }

    fn configure_notify(&mut self, _xwm: XwmId, window: X11Surface, geometry: Rectangle<i32, Logical>, _above: Option<u32>) {
        if let Some(w) = self.x11_window(&window) {
            if window.is_override_redirect() {
                self.space.map_element(w, geometry.loc, false);
            }
        }
    }

    fn fullscreen_request(&mut self, _xwm: XwmId, window: X11Surface) {
        self.x11_fullscreen(&window, true);
    }

    fn unfullscreen_request(&mut self, _xwm: XwmId, window: X11Surface) {
        self.x11_fullscreen(&window, false);
    }

    fn maximize_request(&mut self, _xwm: XwmId, window: X11Surface) {
        if let Some(w) = self.x11_window(&window) {
            if !meta(&w).lock().unwrap().maximized {
                self.toggle_maximize(&w);
            }
        }
    }

    fn unmaximize_request(&mut self, _xwm: XwmId, window: X11Surface) {
        if let Some(w) = self.x11_window(&window) {
            if meta(&w).lock().unwrap().maximized {
                self.toggle_maximize(&w);
            }
        }
    }

    fn minimize_request(&mut self, _xwm: XwmId, window: X11Surface) {
        if let Some(w) = self.x11_window(&window) {
            self.minimize(&w);
        }
    }

    fn resize_request(&mut self, _xwm: XwmId, window: X11Surface, button: u32, edges: X11ResizeEdge) {
        let Some(w) = self.x11_window(&window) else { return };
        use crate::grabs::resize_grab::ResizeEdge;
        let e = match edges {
            X11ResizeEdge::Top => ResizeEdge::TOP,
            X11ResizeEdge::Bottom => ResizeEdge::BOTTOM,
            X11ResizeEdge::Left => ResizeEdge::LEFT,
            X11ResizeEdge::Right => ResizeEdge::RIGHT,
            X11ResizeEdge::TopLeft => ResizeEdge::TOP_LEFT,
            X11ResizeEdge::TopRight => ResizeEdge::TOP_RIGHT,
            X11ResizeEdge::BottomLeft => ResizeEdge::BOTTOM_LEFT,
            X11ResizeEdge::BottomRight => ResizeEdge::BOTTOM_RIGHT,
        };
        let pos = self.seat.get_pointer().unwrap().current_location();
        self.start_resize_pub(&w, e, button, pos);
    }

    fn move_request(&mut self, _xwm: XwmId, window: X11Surface, button: u32) {
        let Some(w) = self.x11_window(&window) else { return };
        let pos = self.seat.get_pointer().unwrap().current_location();
        self.start_move_pub(&w, button, pos);
    }
}
