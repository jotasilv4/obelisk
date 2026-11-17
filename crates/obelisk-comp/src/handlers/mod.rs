mod compositor;
pub mod protocols;
pub mod gaming;
mod xdg_shell;

use crate::Obelisk;

//
// Wl Seat
//

use smithay::input::{Seat, SeatHandler, SeatState};
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::reexports::wayland_server::Resource;
use smithay::wayland::output::OutputHandler;
use smithay::wayland::selection::data_device::{
    set_data_device_focus, ClientDndGrabHandler, DataDeviceHandler, DataDeviceState, ServerDndGrabHandler,
};
use smithay::wayland::selection::SelectionHandler;
use smithay::{delegate_data_device, delegate_output, delegate_seat};

impl SeatHandler for Obelisk {
    type KeyboardFocus = WlSurface;
    type PointerFocus = WlSurface;
    type TouchFocus = WlSurface;

    fn seat_state(&mut self) -> &mut SeatState<Obelisk> {
        &mut self.seat_state
    }

    fn cursor_image(&mut self, _seat: &Seat<Self>, image: smithay::input::pointer::CursorImageStatus) {
        self.cursor_status = image;
    }

    fn focus_changed(&mut self, seat: &Seat<Self>, focused: Option<&WlSurface>) {
        let dh = &self.display_handle;
        let client = focused.and_then(|s| dh.get_client(s.id()).ok());
        set_data_device_focus(dh, seat, client);
    }
}

delegate_seat!(Obelisk);

//
// Wl Data Device
//

impl SelectionHandler for Obelisk {
    /// Texto oferecido pelo próprio compositor (ex.: emoji escolhido no seletor).
    type SelectionUserData = std::sync::Arc<String>;

    fn send_selection(
        &mut self,
        _ty: smithay::wayland::selection::SelectionTarget,
        _mime_type: String,
        fd: std::os::fd::OwnedFd,
        _seat: Seat<Self>,
        user_data: &Self::SelectionUserData,
    ) {
        let text = user_data.clone();
        std::thread::spawn(move || {
            use std::io::Write;
            let mut f = std::fs::File::from(fd);
            let _ = f.write_all(text.as_bytes());
        });
    }
}

impl DataDeviceHandler for Obelisk {
    fn data_device_state(&self) -> &DataDeviceState {
        &self.data_device_state
    }
}

impl ClientDndGrabHandler for Obelisk {
    /// Um app começou a arrastar: se oferece arquivos, já pede a lista (para o notch receber).
    fn started(&mut self, source: Option<smithay::reexports::wayland_server::protocol::wl_data_source::WlDataSource>, _icon: Option<WlSurface>, _seat: Seat<Self>) {
        self.dnd_uris = None;
        let Some(source) = source else { return };
        let offers_files = smithay::wayland::selection::data_device::with_source_metadata(&source, |m| m.mime_types.iter().any(|t| t == "text/uri-list")).unwrap_or(false);
        if !offers_files {
            return;
        }
        use std::os::fd::{AsFd, FromRawFd, OwnedFd};
        let mut fds = [0i32; 2];
        if unsafe { libc::pipe2(fds.as_mut_ptr(), libc::O_CLOEXEC) } != 0 {
            return;
        }
        let (read, write) = unsafe { (OwnedFd::from_raw_fd(fds[0]), OwnedFd::from_raw_fd(fds[1])) };
        source.send("text/uri-list".into(), write.as_fd());
        drop(write);
        let buf = std::sync::Arc::new(std::sync::Mutex::new(String::new()));
        let out = buf.clone();
        std::thread::spawn(move || {
            use std::io::Read;
            let mut s = String::new();
            let _ = std::fs::File::from(read).read_to_string(&mut s);
            *out.lock().unwrap() = s;
        });
        self.dnd_uris = Some(buf);
        self.dirty = true;
    }

    /// Soltou: em cima do notch, os arquivos vão para a Bandeja ou para a área de transferência.
    fn dropped(&mut self, _target: Option<WlSurface>, _validated: bool, _seat: Seat<Self>) {
        let Some(buf) = self.dnd_uris.take() else { return };
        let pos = self.last_pointer;
        if let Some(frac) = self.notch_drop_frac(pos) {
            // dá um instante para o app terminar de escrever a lista
            std::thread::sleep(std::time::Duration::from_millis(30));
            let text = buf.lock().unwrap().clone();
            let uris: Vec<String> = text.lines().map(|l| l.trim().to_string()).filter(|l| l.starts_with("file://")).collect();
            if frac > 0.55 {
                crate::apps::spawn(&format!("wl-copy -t text/uri-list {}", crate::shell::settings_model::q(&uris.join("\r\n"))));
            } else {
                for u in uris {
                    let p = crate::shell::notch::uri_to_path(&u);
                    if !self.shelf.contains(&p) {
                        self.shelf.push(p);
                    }
                }
                self.shell.notch.tab = crate::shell::notch::Tab::Tray;
                self.shell.notch.open = true;
            }
        }
        self.dirty = true;
    }
}
impl ServerDndGrabHandler for Obelisk {
    /// Um app pediu os arquivos arrastados (Bandeja ou mesa).
    fn send(&mut self, mime_type: String, fd: std::os::fd::OwnedFd, _seat: Seat<Self>) {
        let Some(d) = &self.drag_out else { return };
        let text = if mime_type == "text/uri-list" {
            d.paths.iter().map(|p| format!("file://{}", crate::shell::notch::path_to_uri(p))).collect::<Vec<_>>().join("\r\n") + "\r\n"
        } else {
            d.paths.join("\n")
        };
        std::thread::spawn(move || {
            use std::io::Write;
            let _ = std::fs::File::from(fd).write_all(text.as_bytes());
        });
    }

    fn dropped(&mut self, _seat: Seat<Self>) {
        // um app aceitou se `cancelled` não vier logo em seguida
        if let Some(d) = &mut self.drag_out {
            d.dropped = true;
        }
        let pending = self.drag_out.clone();
        if pending.is_some() {
            // `cancelled` (sem app) chega na mesma chamada; aqui só finaliza quando foi aceito
            self.shell.desktop.dragging = None;
        }
    }

    fn cancelled(&mut self, _seat: Seat<Self>) {
        if let Some(d) = &mut self.drag_out {
            d.dropped = false;
        }
        self.drag_out_landed();
    }

    fn finished(&mut self, _seat: Seat<Self>) {
        self.drag_out = None;
        self.shell.desktop.dragging = None;
        self.dirty = true;
    }
}

delegate_data_device!(Obelisk);

//
// Wl Output & Xdg Output
//

impl OutputHandler for Obelisk {}
delegate_output!(Obelisk);

// wp_cursor_shape: apps pedem o cursor por nome (texto, mão, redimensionar...)
impl smithay::wayland::tablet_manager::TabletSeatHandler for Obelisk {}
smithay::delegate_cursor_shape!(Obelisk);
