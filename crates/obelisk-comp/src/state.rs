#[allow(unused_imports)]
use smithay::wayland::seat::WaylandFocus;
use std::{collections::HashSet, ffi::OsString, sync::Arc, sync::Mutex, time::Instant};

use smithay::utils::IsAlive;
use smithay::{
    desktop::{PopupManager, Space, Window, WindowSurfaceType},
    input::{pointer::CursorImageStatus, Seat, SeatState},
    output::Output,
    reexports::{
        calloop::{generic::Generic, EventLoop, Interest, LoopHandle, LoopSignal, Mode, PostAction},
        wayland_server::{
            backend::{ClientData, ClientId, DisconnectReason},
            protocol::wl_surface::WlSurface,
            Display, DisplayHandle, Resource,
        },
    },
    utils::{Logical, Point, Rectangle, Size},
    wayland::{
        compositor::{with_states, CompositorClientState, CompositorState},
        output::OutputManagerState,
        selection::data_device::DataDeviceState,
        shell::xdg::{decoration::XdgDecorationState, ToplevelSurface, XdgShellState, XdgToplevelSurfaceData},
        shm::ShmState,
        socket::ListeningSocketSource,
    },
};

use crate::shell::Shell;
use crate::ui::theme::{metrics, Theme};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BackendKind {
    Nested,
    Drm,
}

/// Metadados por janela (guardados no `UserDataMap` da `Window`).
#[derive(Default)]
pub struct WinMeta {
    pub saved_geo: Option<Rectangle<i32, Logical>>,
    pub restore_pos: Option<Point<i32, Logical>>,
    pub fullscreen: bool,
    pub maximized: bool,
    /// Geometria antes do encaixe (para ⌘⌥⌫).
    pub tiled_from: Option<Rectangle<i32, Logical>>,
    /// Animações: abertura e minimização (até onde encolher, na tela).
    pub opened_at: Option<Instant>,
    pub minimizing: Option<(Instant, Point<f64, Logical>)>,
    /// fechar: some em 150 ms e só então pede ao app para fechar
    pub closing_at: Option<Instant>,
    pub close_sent: bool,
    /// "Manter na Frente" (como uma janela de vídeo em PiP)
    pub on_top: bool,
}

pub fn meta(window: &Window) -> &Mutex<WinMeta> {
    window.user_data().get_or_insert_threadsafe(|| Mutex::new(WinMeta::default()))
}

pub struct Obelisk {
    pub start_time: Instant,
    pub socket_name: OsString,
    pub display_handle: DisplayHandle,
    pub loop_signal: LoopSignal,
    pub loop_handle: LoopHandle<'static, Obelisk>,

    pub space: Space<Window>,
    pub minimized: Vec<Window>,

    pub compositor_state: CompositorState,
    pub xdg_shell_state: XdgShellState,
    pub xdg_decoration_state: XdgDecorationState,
    pub shm_state: ShmState,
    pub output_manager_state: OutputManagerState,
    pub seat_state: SeatState<Obelisk>,
    pub data_device_state: DataDeviceState,
    pub popups: PopupManager,
    pub seat: Seat<Self>,

    pub output: Option<Output>,
    pub screen: Size<i32, Logical>,
    pub scale: i32,
    pub backend: BackendKind,
    pub mod_alt: bool,
    pub shell: Shell,
    pub cursor_status: CursorImageStatus,
    pub swallow_release: bool,
    pub intercepted: HashSet<u32>,
    pub last_title_click: Option<(Instant, smithay::reexports::wayland_server::backend::ObjectId)>,
    pub extra_exit: bool,
    pub round_program: Option<smithay::backend::renderer::gles::GlesTexProgram>,
    /// Mesa atual (0..9) e janelas guardadas das outras mesas.
    pub workspace: usize,
    pub ws_store: Vec<Vec<(Window, Point<i32, Logical>)>>,
    /// Janelas na ordem de uso (mais recente primeiro), para o ⌘Tab.
    pub mru: Vec<Window>,
    pub services: crate::services::Services,
    pub last_tick: Instant,
    /// Janelas do Mission Control e do ⌘Tab (na mesma ordem dos itens desenhados).
    pub mc_windows: Vec<Window>,
    pub switch_windows: Vec<Window>,
    /// Captura pedida: salva no próximo quadro (caminho de destino).
    pub screenshot: Option<std::path::PathBuf>,
    /// Recorte (px lógicos) da próxima captura; `None` = tela inteira.
    pub screenshot_rect: Option<(f32, f32, f32, f32)>,
    /// Testes: depois de N quadros, salva captura em `.0` e encerra se `.1`.
    pub debug_capture: Option<(u32, std::path::PathBuf, bool)>,
    pub debug_capture_path: Option<std::path::PathBuf>,
    /// testes: força o modo de soltar arquivo no notch
    pub debug_drop: Option<f32>,
    pub debug_media: Option<crate::services::Media>,
    pub game: crate::gamemode::GameMode,
    /// Algo mudou desde o último quadro (commit de cliente, entrada, estado do sistema).
    pub dirty: bool,
    pub snapping: bool,
    pub hot_corners: bool,
    pub in_hot_corner: bool,
    pub dmabuf_state: smithay::wayland::dmabuf::DmabufState,
    pub primary_selection_state: smithay::wayland::selection::primary_selection::PrimarySelectionState,
    pub idle_inhibitors: std::collections::HashSet<WlSurface>,
    /// Instante da última entrada do usuário (bloqueio por inatividade).
    pub last_input: Instant,
    pub xwm: Option<smithay::xwayland::X11Wm>,
    /// Janela X11 que deve receber foco quando a superfície Wayland dela for associada.
    pub pending_focus: Option<Window>,
    pub notif_conn: Option<zbus::blocking::Connection>,
    /// Histórico (mais recente primeiro) e fila de banners do notch.
    pub notifs: Vec<crate::notifications::Notif>,
    pub banners: std::collections::VecDeque<crate::notifications::Notif>,
    pub banner_since: Option<Instant>,
    pub lock: crate::lock::Lock,
    /// Minutos sem uso até bloquear (0 = desligado). `OBELISK_LOCK_MINUTES`.
    pub lock_minutes: u64,
    /// Night Shift (tons quentes) — aplicado pelo backend nativo na gamma da tela.
    pub night_shift: bool,
    pub low_power: bool,
    /// Aparência automática: clara das 7h às 19h.
    pub auto_appearance: bool,
    /// App Exposé: Mission Control só com as janelas deste app.
    pub expose_app: Option<String>,
    pub gesture: (f64, f64, u32),
    /// ⌘ (Super) chega aos apps como Ctrl (Ctrl+Shift em terminais): ⌘C/⌘V como no macOS.
    pub cmd_as_ctrl: bool,
    pub cmd_down: bool,
    /// teclas sintéticas enviadas ao app enquanto ⌘ está pressionado
    pub cmd_synth: Vec<u32>,
    /// estado físico de Ctrl/Shift/Alt (o lógico inclui os sintéticos)
    pub phys_mods: (bool, bool, bool),
    pub recorder: Option<crate::recorder::Recorder>,
    pub settings: crate::settings::Settings,
    pub setctl: crate::settings_ctl::SettingsCtl,
    pub applied_layout: Option<String>,
    pub natural_scroll: bool,
    /// Bandeja do notch: arquivos guardados (caminhos)
    pub shelf: Vec<String>,
    /// cartão da última captura no notch: (arquivo, miniatura, deu certo, desde)
    pub capture_toast: Option<(String, Option<resvg::tiny_skia::Pixmap>, bool, Instant)>,
    /// arrastar e soltar vindo de um app: lista de URIs pedida ao começar
    pub dnd_uris: Option<std::sync::Arc<std::sync::Mutex<String>>>,
    /// aparelhos Bluetooth conectados na última leitura (para avisar no notch)
    pub bt_prev: Option<Vec<String>>,
    /// clique num item arrastável (Bandeja/mesa): vira arrasto depois de mover 5 px
    pub press_drag: Option<(DragFrom, Point<f64, Logical>, smithay::utils::Serial, u32)>,
    /// arrasto iniciado pelo compositor (arquivos para outros apps)
    pub drag_out: Option<DragOut>,
    pub drag_ghost: crate::shell::layer::Layer,
    pub pending_shelf_open: Option<usize>,
    /// última posição do ponteiro (para callbacks de arrasto, onde o ponteiro está travado)
    pub last_pointer: Point<f64, Logical>,
    /// escala real da tela principal (fracionária); `scale` é ela arredondada para cima (buffers da UI)
    pub fscale: f64,
    /// desenho de uma tela extra: origem lógica dela (as janelas são deslocadas)
    pub view_origin: Point<i32, Logical>,
    pub sec_wallpapers: std::collections::HashMap<String, crate::shell::wallpaper::Wallpaper>,
    pub clock: smithay::utils::Clock<smithay::utils::Monotonic>,
    pub protocols: crate::handlers::protocols::ExtraProtocols,
    /// pedidos de captura de tela dos apps (wlr-screencopy), atendidos depois do próximo quadro
    pub screencopy: Vec<crate::screencopy::PendingCopy>,
    /// telas do backend nativo: (nome, modos "LxA@Hz", modo atual, escala)
    pub output_modes: Vec<(String, Vec<String>, String, f64)>,
    /// os Ajustes mudaram resolução/escala: o backend aplica no próximo quadro
    pub outputs_changed: bool,
    /// telas arrumadas (backend nativo; no aninhado, só a janela)
    pub displays: Vec<DisplayInfo>,
    pub mirror: bool,
    /// escala forçada ao desenhar (tela espelhada)
    pub render_scale: Option<f64>,
    pub force_update_check: bool,
    /// rodando como tela de login (greetd)
    pub greeter: bool,
    /// dispositivos do libinput (backend nativo) para aplicar aceleração/rolagem
    pub input_devices: Vec<smithay::reexports::input::Device>,
    pub tray: Option<crate::tray::Tray>,
    pub tray_items: Vec<crate::tray::TrayItem>,
    pub tray_refresh_at: Instant,
    /// Zoom de acessibilidade (1 = desligado), centrado no cursor.
    pub zoom: f64,
    /// Stage Manager: o app em foco no palco, os outros em miniaturas à esquerda.
    pub stage_manager: bool,
    /// "usuário@máquina" mostrado no notch ocioso
    pub user_host: String,
    pub polkit_conn: Option<zbus::blocking::Connection>,
    pub auth: Option<crate::polkit::AuthRequest>,
    pub auth_password: String,
    pub xwayland_shell_state: smithay::wayland::xwayland_shell::XWaylandShellState,
}

impl Obelisk {
    pub fn new(
        event_loop: &mut EventLoop<'static, Obelisk>,
        display: Display<Self>,
        backend: BackendKind,
        theme: Theme,
        scale: i32,
        demo_dock: bool,
    ) -> Self {
        let dh = display.handle();
        let compositor_state = CompositorState::new::<Self>(&dh);
        let xdg_shell_state = XdgShellState::new::<Self>(&dh);
        let xdg_decoration_state = XdgDecorationState::new::<Self>(&dh);
        let shm_state = ShmState::new::<Self>(&dh, vec![]);
        let output_manager_state = OutputManagerState::new_with_xdg_output::<Self>(&dh);
        let mut seat_state = SeatState::new();
        let data_device_state = DataDeviceState::new::<Self>(&dh);
        smithay::wayland::cursor_shape::CursorShapeManagerState::new::<Self>(&dh);
        smithay::wayland::relative_pointer::RelativePointerManagerState::new::<Self>(&dh);
        smithay::wayland::pointer_constraints::PointerConstraintsState::new::<Self>(&dh);
        smithay::wayland::idle_inhibit::IdleInhibitManagerState::new::<Self>(&dh);
        smithay::wayland::content_type::ContentTypeState::new::<Self>(&dh);
        smithay::wayland::viewporter::ViewporterState::new::<Self>(&dh);
        crate::screencopy::init(&dh);
        let protocols = crate::handlers::protocols::ExtraProtocols::new(&dh, libc::CLOCK_MONOTONIC as u32);
        let xwayland_shell_state = smithay::wayland::xwayland_shell::XWaylandShellState::new::<Self>(&dh);
        let primary_selection_state = smithay::wayland::selection::primary_selection::PrimarySelectionState::new::<Self>(&dh);
        let popups = PopupManager::default();

        let mut seat: Seat<Self> = seat_state.new_wl_seat(&dh, "seat0");
        seat.add_keyboard(Default::default(), 300, 30).expect("teclado");
        seat.add_pointer();

        let socket_name = Self::init_wayland_listener(display, event_loop);
        let mod_alt = match std::env::var("OBELISK_MOD").as_deref() {
            Ok("alt") => true,
            Ok("super") => false,
            _ => backend == BackendKind::Nested, // aninhado: o host costuma capturar Super
        };

        Obelisk {
            start_time: Instant::now(),
            display_handle: dh,
            loop_signal: event_loop.get_signal(),
            loop_handle: event_loop.handle(),
            space: Space::default(),
            minimized: Vec::new(),
            compositor_state,
            xdg_shell_state,
            xdg_decoration_state,
            shm_state,
            output_manager_state,
            seat_state,
            data_device_state,
            popups,
            seat,
            output: None,
            screen: (1280, 800).into(),
            scale,
            backend,
            mod_alt,
            shell: Shell::new(theme, demo_dock),
            cursor_status: CursorImageStatus::default_named(),
            swallow_release: false,
            intercepted: HashSet::new(),
            last_title_click: None,
            extra_exit: false,
            round_program: None,
            workspace: 0,
            ws_store: vec![Vec::new(); 9],
            mru: Vec::new(),
            services: crate::services::Services::start(),
            last_tick: Instant::now(),
            mc_windows: Vec::new(),
            switch_windows: Vec::new(),
            screenshot: None,
            screenshot_rect: None,
            debug_capture: None,
            debug_capture_path: None,
            debug_drop: None,
            debug_media: None,
            game: Default::default(),
            dirty: true,
            snapping: std::env::var("OBELISK_SNAP").map(|v| v != "0").unwrap_or(true),
            hot_corners: std::env::var("OBELISK_HOT_CORNERS").map(|v| v != "0").unwrap_or(true),
            in_hot_corner: false,
            dmabuf_state: smithay::wayland::dmabuf::DmabufState::new(),
            primary_selection_state,
            idle_inhibitors: Default::default(),
            last_input: Instant::now(),
            xwm: None,
            pending_focus: None,
            notif_conn: None,
            notifs: Vec::new(),
            banners: Default::default(),
            banner_since: None,
            lock: crate::lock::Lock::new(),
            night_shift: false,
            low_power: false,
            auto_appearance: std::env::var("OBELISK_APPEARANCE").is_ok_and(|v| v == "auto"),
            expose_app: None,
            gesture: (0.0, 0.0, 0),
            cmd_as_ctrl: !mod_alt && std::env::var("OBELISK_CMD_AS_CTRL").map(|v| v != "0").unwrap_or(true),
            cmd_down: false,
            cmd_synth: Vec::new(),
            phys_mods: (false, false, false),
            recorder: None,
            settings: crate::settings::Settings::load(),
            setctl: Default::default(),
            applied_layout: None,
            natural_scroll: false,
            shelf: Vec::new(),
            capture_toast: None,
            dnd_uris: None,
            bt_prev: None,
            press_drag: None,
            drag_out: None,
            drag_ghost: Default::default(),
            pending_shelf_open: None,
            last_pointer: Point::from((0.0, 0.0)),
            fscale: scale as f64,
            view_origin: Point::from((0, 0)),
            sec_wallpapers: Default::default(),
            clock: smithay::utils::Clock::new(),
            protocols,
            screencopy: Vec::new(),
            output_modes: Vec::new(),
            outputs_changed: false,
            greeter: false,
            displays: Vec::new(),
            mirror: false,
            render_scale: None,
            force_update_check: false,
            input_devices: Vec::new(),
            tray: None,
            tray_items: Vec::new(),
            tray_refresh_at: Instant::now(),
            zoom: 1.0,
            stage_manager: false,
            polkit_conn: None,
            auth: None,
            auth_password: String::new(),
            user_host: format!(
                "{}@{}",
                crate::lock::current_user(),
                if crate::lock::demo().is_some() { "obelisk".to_string() } else { std::fs::read_to_string("/etc/hostname").map(|h| h.trim().to_string()).unwrap_or_else(|_| "obelisk".into()) }
            ),
            lock_minutes: std::env::var("OBELISK_LOCK_MINUTES").ok().and_then(|v| v.parse().ok()).unwrap_or(10),
            xwayland_shell_state,
            socket_name,
        }
    }

    fn init_wayland_listener(display: Display<Obelisk>, event_loop: &mut EventLoop<'static, Obelisk>) -> OsString {
        let listening_socket = ListeningSocketSource::new_auto().expect("socket wayland");
        let socket_name = listening_socket.socket_name().to_os_string();
        let handle = event_loop.handle();
        handle
            .insert_source(listening_socket, move |stream, _, state| {
                state
                    .display_handle
                    .insert_client(stream, Arc::new(ClientState::default()))
                    .unwrap();
            })
            .expect("fonte do socket");
        handle
            .insert_source(Generic::new(display, Interest::READ, Mode::Level), |_, display, state| {
                // Safety: o display não é descartado enquanto o loop roda.
                unsafe {
                    display.get_mut().dispatch_clients(state).unwrap();
                }
                Ok(PostAction::Continue)
            })
            .expect("fonte do display");
        socket_name
    }

    // ------------------------------------------------------------------ janelas

    pub fn window_of(&self, surface: &WlSurface) -> Option<Window> {
        self.space
            .elements()
            .chain(self.minimized.iter())
            .find(|w| w.wl_surface().as_deref() == Some(surface))
            .cloned()
    }

    pub fn is_ssd(&self, window: &Window) -> bool {
        use smithay::reexports::wayland_protocols::xdg::decoration::zv1::server::zxdg_toplevel_decoration_v1::Mode;
        let fullscreen = meta(window).lock().unwrap().fullscreen;
        if fullscreen {
            return false;
        }
        if let Some(x) = window.x11_surface() {
            // X11 sem moldura própria e que não é menu/dica ganha a barra do Obelisk
            return !x.is_override_redirect() && !x.is_decorated();
        }
        let Some(t) = window.toplevel() else { return false };
        t.current_state().decoration_mode == Some(Mode::ServerSide)
    }

    pub fn app_id(window: &Window) -> String {
        if let Some(x) = window.x11_surface() {
            return x.class();
        }
        window
            .toplevel()
            .and_then(|t| {
                with_states(t.wl_surface(), |s| {
                    s.data_map.get::<XdgToplevelSurfaceData>().and_then(|d| d.lock().unwrap().app_id.clone())
                })
            })
            .unwrap_or_default()
    }

    pub fn title(window: &Window) -> String {
        if let Some(x) = window.x11_surface() {
            return x.title();
        }
        window
            .toplevel()
            .and_then(|t| {
                with_states(t.wl_surface(), |s| {
                    s.data_map.get::<XdgToplevelSurfaceData>().and_then(|d| d.lock().unwrap().title.clone())
                })
            })
            .unwrap_or_default()
    }

    pub fn win_id(window: &Window) -> Option<smithay::reexports::wayland_server::backend::ObjectId> {
        window.wl_surface().map(|s| s.id())
    }

    /// Janela "de verdade" (não menu/dica X11 sem moldura).
    pub fn is_normal(window: &Window) -> bool {
        window.x11_surface().is_none_or(|x| !x.is_override_redirect())
    }

    pub fn focused_window(&self) -> Option<Window> {
        let kb = self.seat.get_keyboard()?;
        let focus = kb.current_focus()?;
        self.window_of(&focus)
    }

    /// Janela visível mais acima.
    pub fn top_window(&self) -> Option<Window> {
        self.space.elements().last().cloned()
    }

    pub fn focus_window(&mut self, window: &Window) {
        self.mru.retain(|w| w != window);
        self.mru.insert(0, window.clone());
        self.space.raise_element(window, true);
        let serial = smithay::utils::SERIAL_COUNTER.next_serial();
        for w in self.space.elements() {
            let active = w == window;
            w.set_activated(active);
            if let Some(t) = w.toplevel() {
                t.send_pending_configure();
            }
        }
        if let Some(x) = window.x11_surface() {
            if let Some(xwm) = self.xwm.as_mut() {
                let _ = xwm.raise_window(x);
            }
        }
        // janelas "Manter na Frente" ficam acima das outras
        let tops: Vec<Window> = self.space.elements().filter(|w| meta(w).lock().unwrap().on_top).cloned().collect();
        for t in tops {
            self.space.raise_element(&t, false);
        }
        match (self.seat.get_keyboard(), window.wl_surface().map(|s| s.into_owned())) {
            (Some(kb), Some(s)) => kb.set_focus(self, Some(s), serial),
            _ => self.pending_focus = Some(window.clone()),
        }
    }

    pub fn unfocus_all(&mut self) {
        let serial = smithay::utils::SERIAL_COUNTER.next_serial();
        for w in self.space.elements() {
            w.set_activated(false);
            if let Some(t) = w.toplevel() {
                t.send_pending_configure();
            }
        }
        if let Some(kb) = self.seat.get_keyboard() {
            kb.set_focus(self, Option::<WlSurface>::None, serial);
        }
    }

    pub fn close_window(&mut self, window: &Window) {
        // como no macOS: a janela some rápido (encolhe e esmaece) e então fecha
        if self.settings.animations && !self.game.active && self.space.element_location(window).is_some() {
            let mut m = meta(window).lock().unwrap();
            if m.closing_at.is_none() {
                m.closing_at = Some(Instant::now());
                m.close_sent = false;
                self.dirty = true;
                return;
            }
        }
        self.send_close(window);
    }

    fn send_close(&mut self, window: &Window) {
        if let Some(t) = window.toplevel() {
            t.send_close();
        } else if let Some(x) = window.x11_surface() {
            let _ = x.close();
        }
    }

    pub fn minimize(&mut self, window: &Window) {
        if let Some(loc) = self.space.element_location(window) {
            meta(window).lock().unwrap().restore_pos = Some(loc);
        }
        // anima até o Dock (área dos minimizados, à esquerda do Lixo) e só então tira da tela
        if !self.game.active && meta(window).lock().unwrap().minimizing.is_none() {
            let (px, py, pw, _) = self.shell.dock.panel;
            let target = Point::<f64, Logical>::from(((px + pw - 90.0).max(0.0) as f64, py as f64 + 20.0));
            meta(window).lock().unwrap().minimizing = Some((Instant::now(), target));
            if !self.minimized.contains(window) {
                self.minimized.push(window.clone());
            }
            let next = self.space.elements().rev().find(|w| *w != window && meta(w).lock().unwrap().minimizing.is_none()).cloned();
            match next {
                Some(next) => self.focus_window(&next),
                None => self.unfocus_all(),
            }
            self.dirty = true;
            return;
        }
        self.space.unmap_elem(window);
        if !self.minimized.contains(window) {
            self.minimized.push(window.clone());
        }
        match self.top_window() {
            Some(next) => self.focus_window(&next),
            None => self.unfocus_all(),
        }
    }

    pub fn restore(&mut self, window: &Window) {
        self.minimized.retain(|w| w != window);
        {
            let mut m = meta(window).lock().unwrap();
            m.minimizing = None;
            m.opened_at = Some(Instant::now());
        }
        let pos = meta(window).lock().unwrap().restore_pos.unwrap_or((120, 120).into());
        self.space.map_element(window.clone(), pos, true);
        self.focus_window(window);
    }

    /// Área útil para janelas (abaixo da barra de menus, acima do dock).
    pub fn work_area(&self) -> Rectangle<i32, Logical> {
        let top = metrics::MENUBAR_H;
        let bottom_reserved = (crate::shell::dock::PLATE_H + crate::shell::dock::BOTTOM) as i32 + 6;
        Rectangle::new((0, top).into(), (self.screen.w, self.screen.h - top - bottom_reserved).into())
    }

    /// Tela (retângulo lógico) que contém `p`; fora de todas, a principal.
    pub fn output_rect_at(&self, p: Point<f64, Logical>) -> Rectangle<i32, Logical> {
        self.space
            .outputs()
            .filter_map(|o| self.space.output_geometry(o))
            .find(|g| g.to_f64().contains(p))
            .unwrap_or_else(|| Rectangle::from_size(self.screen))
    }

    /// Área útil da tela onde está a janela: a principal tem barra e Dock; as outras, a tela toda.
    pub fn work_area_for(&self, window: &Window) -> Rectangle<i32, Logical> {
        let c = self.space.element_location(window).map(|l| {
            let s = window.geometry().size;
            Point::<f64, Logical>::from((l.x as f64 + s.w as f64 / 2.0, l.y as f64 + s.h as f64 / 2.0))
        });
        let r = c.map(|c| self.output_rect_at(c)).unwrap_or_else(|| Rectangle::from_size(self.screen));
        if r.loc == Point::from((0, 0)) {
            self.work_area()
        } else {
            let gap = 0;
            Rectangle::new((r.loc.x, r.loc.y + gap).into(), r.size)
        }
    }

    /// Limites de todas as telas juntas (para o ponteiro).
    pub fn desktop_bounds(&self) -> Rectangle<i32, Logical> {
        self.space
            .outputs()
            .filter_map(|o| self.space.output_geometry(o))
            .fold(Rectangle::from_size(self.screen), |a, g| a.merge(g))
    }

    pub fn toggle_maximize(&mut self, window: &Window) {
        if let Some(x) = window.x11_surface().cloned() {
            let mut m = meta(window).lock().unwrap();
            if let Some(saved) = m.saved_geo.take() {
                m.maximized = false;
                drop(m);
                let _ = x.set_maximized(false);
                let _ = x.configure(saved);
                self.space.map_element(window.clone(), saved.loc, false);
            } else {
                let Some(loc) = self.space.element_location(window) else { return };
                m.saved_geo = Some(Rectangle::new(loc, window.geometry().size));
                m.maximized = true;
                drop(m);
                let area = self.work_area_for(window);
                let tb = if self.is_ssd(window) { metrics::TITLEBAR_H } else { 0 };
                let r = Rectangle::new((area.loc.x, area.loc.y + tb).into(), (area.size.w, area.size.h - tb).into());
                let _ = x.set_maximized(true);
                let _ = x.configure(r);
                self.space.map_element(window.clone(), r.loc, false);
            }
            return;
        }
        let Some(t) = window.toplevel() else { return };
        let ssd = self.is_ssd(window);
        let mut m = meta(window).lock().unwrap();
        if let Some(saved) = m.saved_geo.take() {
            m.maximized = false;
            drop(m);
            t.with_pending_state(|s| {
                s.states.unset(smithay::reexports::wayland_protocols::xdg::shell::server::xdg_toplevel::State::Maximized);
                s.size = Some(saved.size);
            });
            t.send_pending_configure();
            self.space.map_element(window.clone(), saved.loc, false);
        } else {
            let Some(loc) = self.space.element_location(window) else { return };
            m.saved_geo = Some(Rectangle::new(loc, window.geometry().size));
            m.maximized = true;
            drop(m);
            let area = self.work_area_for(window);
            let tb = if ssd { metrics::TITLEBAR_H } else { 0 };
            t.with_pending_state(|s| {
                s.states.set(smithay::reexports::wayland_protocols::xdg::shell::server::xdg_toplevel::State::Maximized);
                s.size = Some((area.size.w, area.size.h - tb).into());
            });
            t.send_pending_configure();
            self.space.map_element(window.clone(), (area.loc.x, area.loc.y + tb), false);
        }
    }

    pub fn set_fullscreen(&mut self, surface: &ToplevelSurface, on: bool) {
        use smithay::reexports::wayland_protocols::xdg::shell::server::xdg_toplevel::State;
        let Some(window) = self.window_of(surface.wl_surface()) else { return };
        let mut m = meta(&window).lock().unwrap();
        if on {
            if m.fullscreen {
                return;
            }
            let loc = self.space.element_location(&window).unwrap_or_default();
            m.saved_geo = Some(Rectangle::new(loc, window.geometry().size));
            m.fullscreen = true;
            drop(m);
            let out = self.output_rect_at(Point::from((loc.x as f64 + window.geometry().size.w as f64 / 2.0, loc.y as f64 + window.geometry().size.h as f64 / 2.0)));
            let size = out.size;
            surface.with_pending_state(|s| {
                s.states.set(State::Fullscreen);
                s.size = Some(size);
            });
            surface.send_pending_configure();
            self.space.map_element(window.clone(), out.loc, true);
        } else {
            if !m.fullscreen {
                return;
            }
            m.fullscreen = false;
            let saved = m.saved_geo.take();
            drop(m);
            surface.with_pending_state(|s| {
                s.states.unset(State::Fullscreen);
                s.size = saved.map(|r| r.size);
            });
            surface.send_pending_configure();
            if let Some(r) = saved {
                self.space.map_element(window.clone(), r.loc, false);
            }
        }
    }

    pub fn any_fullscreen_on_top(&self) -> bool {
        self.top_window().is_some_and(|w| meta(&w).lock().unwrap().fullscreen)
    }

    /// Alt-Tab: traz a próxima janela (a de baixo) para a frente; com `reverse`, restaura minimizada.
    pub fn cycle_windows(&mut self, reverse: bool) {
        let wins: Vec<Window> = self.space.elements().cloned().collect();
        if reverse {
            if let Some(w) = self.minimized.last().cloned() {
                self.restore(&w);
                return;
            }
        }
        if wins.len() >= 2 {
            let next = wins[wins.len() - 2].clone();
            self.focus_window(&next);
        }
    }

    pub fn surface_under(&self, pos: Point<f64, Logical>) -> Option<(WlSurface, Point<f64, Logical>)> {
        self.space.element_under(pos).and_then(|(window, location)| {
            window
                .surface_under(pos - location.to_f64(), WindowSurfaceType::ALL)
                .map(|(s, p)| (s, (p + location).to_f64()))
        })
    }

    /// Troca de mesa: guarda as janelas visíveis e mostra as da mesa `n`.
    pub fn switch_workspace(&mut self, n: usize) {
        if n >= self.ws_store.len() || n == self.workspace {
            return;
        }
        let visible: Vec<(Window, Point<i32, Logical>)> = self
            .space
            .elements()
            .cloned()
            .filter_map(|w| self.space.element_location(&w).map(|l| (w, l)))
            .collect();
        for (w, _) in &visible {
            self.space.unmap_elem(w);
        }
        self.ws_store[self.workspace] = visible;
        self.workspace = n;
        let incoming = std::mem::take(&mut self.ws_store[n]);
        for (w, l) in &incoming {
            self.space.map_element(w.clone(), *l, false);
        }
        match self.mru.iter().find(|w| incoming.iter().any(|(i, _)| i == *w)).cloned().or_else(|| self.top_window()) {
            Some(w) => self.focus_window(&w),
            None => self.unfocus_all(),
        }
    }

    /// Manda a janela para a mesa `n` (ela some da mesa atual).
    pub fn move_to_workspace(&mut self, window: &Window, n: usize) {
        if n >= self.ws_store.len() || n == self.workspace {
            return;
        }
        let loc = self.space.element_location(window).unwrap_or((120, 120).into());
        self.space.unmap_elem(window);
        self.ws_store[n].push((window.clone(), loc));
        match self.top_window() {
            Some(w) => self.focus_window(&w),
            None => self.unfocus_all(),
        }
    }

    /// Apps abertos na ordem de uso: (app_id, janela mais recente). Inclui minimizados.
    pub fn apps_by_mru(&self) -> Vec<(String, Window)> {
        let mut out: Vec<(String, Window)> = Vec::new();
        let all: Vec<Window> = self
            .mru
            .iter()
            .cloned()
            .chain(self.space.elements().rev().cloned())
            .chain(self.minimized.iter().cloned())
            .collect();
        for w in all {
            if !w.alive() || !Self::is_normal(&w) || (self.space.element_location(&w).is_none() && !self.minimized.contains(&w)) {
                continue;
            }
            let id = Self::app_id(&w).to_lowercase();
            let key = if id.is_empty() { Self::title(&w) } else { id };
            if !out.iter().any(|(k, _)| *k == key) {
                out.push((key, w));
            }
        }
        out
    }

    /// Traz o app para a frente (restaura se estiver minimizado).
    pub fn activate_window(&mut self, w: &Window) {
        if self.minimized.contains(w) {
            self.restore(w);
        } else {
            self.focus_window(w);
        }
    }

    /// PID do processo dono da janela (credenciais do cliente Wayland).
    pub fn window_pid(&self, w: &Window) -> Option<i32> {
        let t = w.toplevel()?;
        let client = self.display_handle.get_client(t.wl_surface().id()).ok()?;
        client.get_credentials(&self.display_handle).ok().map(|c| c.pid)
    }

    pub fn on_notif_event(&mut self, ev: crate::notifications::NotifEvent) {
        use crate::notifications::NotifEvent;
        match ev {
            NotifEvent::New(n) => {
                self.notifs.retain(|x| x.id != n.id);
                self.notifs.insert(0, n.clone());
                self.notifs.truncate(50);
                // Foco (Não Perturbe) e Modo Jogo: só vai para a Central (exceto críticas)
                let quiet = self.shell.cc.focus_on || self.game.active || !self.settings.notif_banners;
                if !quiet || n.critical {
                    if let Some(b) = self.banners.iter_mut().find(|b| b.id == n.id) {
                        *b = n;
                    } else {
                        self.banners.push_back(n);
                    }
                }
            }
            NotifEvent::Close(id) => {
                self.dismiss_notif(id, 3);
            }
        }
        self.dirty = true;
    }

    /// Remove do histórico e da fila. `reason`: 1 expirou, 2 dispensada, 3 fechada pelo app.
    pub fn dismiss_notif(&mut self, id: u32, reason: u32) {
        self.notifs.retain(|x| x.id != id);
        if self.banners.front().is_some_and(|b| b.id == id) {
            self.banner_since = None;
        }
        self.banners.retain(|x| x.id != id);
        if let Some(c) = &self.notif_conn {
            crate::notifications::emit_closed(c, id, reason);
        }
        self.dirty = true;
    }

    pub fn invoke_notif_action(&mut self, id: u32, key: &str) {
        if let Some(c) = &self.notif_conn {
            crate::notifications::emit_action(c, id, key);
        }
        self.dismiss_notif(id, 2);
    }

    /// Banner atual: some depois de 5 s (ou o tempo pedido), parado com o mouse em cima; críticas ficam.
    fn tick_banners(&mut self) {
        let Some(front) = self.banners.front() else {
            self.banner_since = None;
            return;
        };
        let since = *self.banner_since.get_or_insert_with(Instant::now);
        let hovered = self.shell.notch.hovered_since.is_some();
        let limit = if front.timeout_ms > 0 { (front.timeout_ms as u64).min(10_000) } else { 5_000 };
        if hovered {
            self.banner_since = Some(Instant::now());
        } else if !front.critical && since.elapsed().as_millis() as u64 > limit {
            self.banners.pop_front();
            self.banner_since = None;
            self.dirty = true;
        }
    }

    /// Bloqueia a tela: fecha painéis e tira o foco dos apps (nenhuma entrada chega a eles).
    pub fn lock_screen(&mut self) {
        if self.lock.active {
            return;
        }
        self.shell.close_popups();
        self.shell.switcher.open = false;
        self.lock.active = true;
        self.lock.password.clear();
        self.lock.state = crate::lock::LockState::Typing;
        self.unfocus_all();
        self.dirty = true;
    }

    /// Primeiro nome do usuário (para o "Olá").
    pub fn lock_display_name(&self) -> String {
        let n = crate::lock::login_users().into_iter().find(|u| u.0 == crate::lock::current_user()).map(|u| u.1).unwrap_or_else(crate::lock::current_user);
        n.split_whitespace().next().unwrap_or("").to_string()
    }

    pub fn on_auth_result(&mut self, ok: bool) {
        // tela de login: o greetd já iniciou a sessão; o greeter sai
        if ok && self.lock.greeter.is_some() {
            if let Some((users, i)) = &self.lock.greeter {
                if let Some(u) = users.get(*i) {
                    let _ = std::fs::write("/var/cache/obelisk-greeter/last-user", &u.0);
                }
            }
            self.loop_signal.stop();
            return;
        }
        if ok {
            self.lock.active = false;
            self.lock.state = crate::lock::LockState::Typing;
            if let Some(w) = self.mru.first().cloned().or_else(|| self.top_window()) {
                self.focus_window(&w);
            }
        } else {
            self.lock.state = crate::lock::LockState::Wrong;
            self.lock.shake = Some(Instant::now());
        }
        self.dirty = true;
    }

    /// Insere texto no app em foco: vira a seleção (oferecida pelo compositor) e "cola" (Ctrl+V,
    /// Ctrl+Shift+V em terminais) com teclas sintéticas.
    pub fn insert_text(&mut self, text: &str, terminal: bool) {
        use smithay::input::keyboard::{FilterResult, Keycode};
        use smithay::backend::input::KeyState;
        smithay::wayland::selection::data_device::set_data_device_selection(
            &self.display_handle,
            &self.seat,
            vec!["text/plain;charset=utf-8".into(), "text/plain".into(), "UTF8_STRING".into(), "TEXT".into()],
            std::sync::Arc::new(text.to_string()),
        );
        let Some(kb) = self.seat.get_keyboard() else { return };
        let mut keys = vec![37u32]; // Ctrl
        if terminal {
            keys.push(50); // Shift
        }
        let t = self.start_time.elapsed().as_millis() as u32;
        for k in &keys {
            kb.input::<(), _>(self, Keycode::new(*k), KeyState::Pressed, smithay::utils::SERIAL_COUNTER.next_serial(), t, |_, _, _| FilterResult::Forward);
        }
        for st in [KeyState::Pressed, KeyState::Released] {
            kb.input::<(), _>(self, Keycode::new(55), st, smithay::utils::SERIAL_COUNTER.next_serial(), t, |_, _, _| FilterResult::Forward); // V
        }
        for k in keys.iter().rev() {
            kb.input::<(), _>(self, Keycode::new(*k), KeyState::Released, smithay::utils::SERIAL_COUNTER.next_serial(), t, |_, _, _| FilterResult::Forward);
        }
    }

    /// Arrasto com arquivos sobre o notch? Devolve a fração x dentro da área (Bandeja < 0,55 < Copiar).
    pub fn notch_drop_frac(&self, pos: smithay::utils::Point<f64, smithay::utils::Logical>) -> Option<f32> {
        if self.debug_drop.is_some() {
            return self.debug_drop;
        }
        self.dnd_uris.as_ref()?;
        let w = crate::shell::notch::OPEN_W;
        let x0 = self.screen.w as f32 / 2.0 - w / 2.0;
        let (x, y) = (pos.x as f32, pos.y as f32);
        (y < 160.0 && x >= x0 - 20.0 && x < x0 + w + 20.0).then(|| ((x - x0) / w).clamp(0.0, 1.0))
    }

    /// Captura feita pelo --debug-capture (testes): não mostra o cartão no notch.
    pub fn debug_shot(&self, path: &std::path::Path) -> bool {
        self.debug_capture_path.as_deref() == Some(path)
    }

    /// Ação do cartão da captura no notch.
    pub fn capture_toast_action(&mut self, what: &str) {
        let Some((path, ..)) = self.capture_toast.clone() else { return };
        let q = crate::shell::settings_model::q(&path);
        match what {
            "open" => crate::apps::spawn(&format!("xdg-open {q}")),
            "copy" => {
                crate::apps::spawn(&format!("wl-copy --type image/png < {q}"));
                self.notify_local("Captura de Tela", "Copiada para a área de transferência", false);
            }
            "annotate" => {
                let tool = crate::apps::first_in_path(&["satty", "swappy"]);
                match tool.as_deref() {
                    Some(t) if t.ends_with("satty") => crate::apps::spawn(&format!("satty --filename {q} --output-filename {q}")),
                    Some(_) => crate::apps::spawn(&format!("swappy -f {q} -o {q}")),
                    None => self.notify_local("Captura de Tela", "Para anotar, instale o satty ou o swappy", false),
                }
            }
            // "Salvar como…": abre a pasta com o arquivo selecionado no gerenciador de arquivos
            "saveas" => crate::apps::spawn(&format!(
                "dbus-send --session --dest=org.freedesktop.FileManager1 --type=method_call /org/freedesktop/FileManager1 org.freedesktop.FileManager1.ShowItems array:string:{} string:'' || xdg-open {}",
                crate::shell::settings_model::q(&format!("file://{path}")),
                crate::shell::settings_model::q(std::path::Path::new(&path).parent().map(|p| p.to_string_lossy().to_string()).unwrap_or_default().as_str())
            )),
            _ => {}
        }
        self.capture_toast = None;
        self.dirty = true;
    }

    /// Envia um atalho (modificadores + tecla) ao app em foco, como se fosse digitado.
    pub fn send_chord(&mut self, mods: &[u32], key: u32) {
        use smithay::backend::input::KeyState;
        use smithay::input::keyboard::{FilterResult, Keycode};
        let Some(kb) = self.seat.get_keyboard() else { return };
        let t = self.start_time.elapsed().as_millis() as u32;
        let send = |s: &mut Self, k: u32, st: KeyState| {
            kb.input::<(), _>(s, Keycode::new(k), st, smithay::utils::SERIAL_COUNTER.next_serial(), t, |_, _, _| FilterResult::Forward);
        };
        for m in mods {
            send(self, *m, KeyState::Pressed);
        }
        send(self, key, KeyState::Pressed);
        send(self, key, KeyState::Released);
        for m in mods.iter().rev() {
            send(self, *m, KeyState::Released);
        }
    }

    /// Começa a gravar (px lógicos; `None` = tela inteira).
    pub fn start_recording(&mut self, rect: Option<(f32, f32, f32, f32)>) {
        self.shell.capture.close();
        self.cursor_status = smithay::input::pointer::CursorImageStatus::default_named();
        let f = self.fscale as f32;
        let (x, y, w, h) = rect.unwrap_or((0.0, 0.0, self.screen.w as f32, self.screen.h as f32));
        let region = ((x * f) as i32, (y * f) as i32, (w * f) as i32, (h * f) as i32);
        match crate::recorder::Recorder::start(region, false) {
            Ok(r) => {
                tracing::info!("gravando em {}", r.path.display());
                self.recorder = Some(r);
            }
            Err(e) => self.notify_local("Gravação", &format!("Não foi possível gravar: {e}"), true),
        }
        self.dirty = true;
    }

    pub fn stop_recording(&mut self) {
        if let Some(r) = self.recorder.take() {
            let dropped = r.dropped;
            tracing::info!("gravação: {} quadros, {} descartados", r.frames, dropped);
            let path = r.stop();
            let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
            let extra = if dropped > 0 { format!(" ({dropped} quadros descartados)") } else { String::new() };
            self.notify_local("Gravação de Tela", &format!("Salva em Vídeos/Gravações de Tela: {name}{extra}"), false);
        }
        self.dirty = true;
    }

    /// Notificação gerada pelo próprio shell (aparece no notch e na Central).
    pub fn notify_local(&mut self, app: &str, body: &str, critical: bool) {
        use std::sync::atomic::{AtomicU32, Ordering};
        static NEXT: AtomicU32 = AtomicU32::new(1_000_000);
        let n = crate::notifications::Notif {
            id: NEXT.fetch_add(1, Ordering::Relaxed),
            app: "Obelisk".into(),
            summary: app.into(),
            body: body.into(),
            icon: String::new(),
            actions: vec![],
            critical,
            timeout_ms: -1,
            at: Instant::now(),
        };
        self.on_notif_event(crate::notifications::NotifEvent::New(n));
    }

    pub fn on_tray_event(&mut self, ev: crate::tray::TrayEvent) {
        use crate::tray::TrayEvent;
        match ev {
            TrayEvent::Registered(bus, path) => {
                if let Some(t) = &self.tray {
                    t.refresh(bus, path);
                }
            }
            TrayEvent::Updated(mut item) => {
                if item.icon.is_none() && !item.icon_name.is_empty() {
                    item.icon = Some(self.shell.icons.get_theme(&item.icon_name, &item.title).clone());
                }
                let key = format!("{}{}", item.bus, item.path);
                match self.tray_items.iter_mut().find(|i| format!("{}{}", i.bus, i.path) == key) {
                    Some(old) => *old = item,
                    None => self.tray_items.push(item),
                }
            }
            TrayEvent::Removed(key) => self.tray_items.retain(|i| format!("{}{}", i.bus, i.path) != key),
            TrayEvent::Menu(key, entries) => self.open_tray_menu(&key, entries),
        }
        self.dirty = true;
    }

    fn open_tray_menu(&mut self, key: &str, entries: Vec<crate::tray::MenuEntry>) {
        use crate::shell::menu::{Menu, MenuAction, MenuItem};
        if entries.is_empty() {
            return;
        }
        let Some(idx) = self.tray_items.iter().position(|i| format!("{}{}", i.bus, i.path) == key) else { return };
        let items: Vec<MenuItem> = entries
            .iter()
            .map(|e| {
                if e.separator {
                    MenuItem::sep()
                } else {
                    let mut m = MenuItem::new(&e.label, MenuAction::TrayMenu(idx, e.id)).enabled(e.enabled);
                    if let Some(c) = e.checked {
                        m = m.check(c);
                    }
                    m
                }
            })
            .collect();
        let x = self.shell.menubar.item_x(crate::shell::menubar::BarItem::Tray(idx)).map(|(a, _)| a).unwrap_or(0.0);
        let mut menu = Menu::new(items, (0, 35), 200.0);
        let (w, _) = menu.size();
        menu.pos = ((x as i32).min(self.screen.w - w - 6), 35);
        menu.origin = Some(crate::shell::menubar::BarItem::Tray(idx));
        self.shell.menu = Some(menu);
    }

    /// Janela no palco? (Stage Manager desligado = todas)
    pub fn on_stage(&self, w: &Window) -> bool {
        if !self.stage_manager || !Self::is_normal(w) {
            return true;
        }
        let Some(f) = self.focused_window() else { return true };
        Self::app_id(w) == Self::app_id(&f)
    }

    /// Miniaturas do Stage Manager: uma por app fora do palco (janela mais recente), em coluna.
    pub fn stage_strip(&self) -> Vec<(Window, (f32, f32, f32, f32))> {
        if !self.stage_manager {
            return vec![];
        }
        let mut seen: Vec<String> = Vec::new();
        let mut wins: Vec<Window> = Vec::new();
        for w in self.mru.iter().chain(self.space.elements().rev()) {
            if self.space.element_location(w).is_none() || self.on_stage(w) || !Self::is_normal(w) {
                continue;
            }
            let id = Self::app_id(w);
            if !seen.contains(&id) {
                seen.push(id);
                wins.push(w.clone());
            }
        }
        wins.truncate(6);
        let tw = 150.0f32;
        let gap = 18.0;
        let heights: Vec<f32> = wins.iter().map(|w| {
            let g = w.geometry().size;
            tw * g.h.max(1) as f32 / g.w.max(1) as f32
        }).map(|h| h.min(120.0)).collect();
        let total: f32 = heights.iter().sum::<f32>() + gap * (heights.len() as f32 - 1.0).max(0.0);
        let mut y = (self.screen.h as f32 - total) / 2.0;
        wins.into_iter().zip(heights).map(|(w, h)| {
            let g = w.geometry().size;
            let width = (h * g.w as f32 / g.h.max(1) as f32).min(tw);
            let r = (24.0, y, width, h);
            y += h + gap;
            (w, r)
        }).collect()
    }

    pub fn on_polkit_event(&mut self, ev: crate::polkit::PolkitEvent) {
        use crate::polkit::PolkitEvent;
        match ev {
            PolkitEvent::Begin(req) => {
                self.shell.close_popups();
                self.auth = Some(req);
                self.auth_password.clear();
            }
            PolkitEvent::Done => {
                self.auth = None;
                self.auth_password.clear();
            }
        }
        self.dirty = true;
    }

    /// Responde ao pedido de senha (`None` = cancelar).
    pub fn auth_reply(&mut self, ok: bool) {
        if let Some(req) = self.auth.take() {
            let pw = std::mem::take(&mut self.auth_password);
            let _ = req.reply.send(if ok && !pw.is_empty() { Some(pw) } else { None });
        }
        self.dirty = true;
    }

    /// Aplica os Ajustes salvos ao estado.
    pub fn apply_settings(&mut self) {
        let s = self.settings.clone();
        self.auto_appearance = s.appearance == "auto";
        if !self.auto_appearance {
            let want_dark = s.appearance != "light";
            if want_dark != (self.shell.theme.appearance == crate::ui::theme::Appearance::Dark) {
                self.shell.theme = if want_dark { crate::ui::theme::Theme::dark() } else { crate::ui::theme::Theme::light() };
                self.shell.decos.clear();
            }
        }
        self.shell.theme = self.shell.theme.with_accent(&s.accent);
        self.shell.decos.clear();
        self.hot_corners = s.hot_corners;
        self.snapping = s.snapping;
        self.lock_minutes = s.lock_minutes;
        if !self.mod_alt {
            self.cmd_as_ctrl = s.cmd_as_ctrl;
        }
        self.apply_internal_settings();
        self.dirty = true;
    }

    /// Clique nos Ajustes.
    pub fn settings_action(&mut self, a: crate::shell::settings_app::SetAction) {
        use crate::shell::settings_app::SetAction;
        match a {
            SetAction::Page(p) => {
                self.shell.settings.set_page(p);
                self.shell.settings.query.clear();
                self.setctl.focus = None;
                self.setctl.expanded = None;
                self.refresh_settings_page();
            }
            SetAction::Section(t) => {
                if !self.shell.settings.collapsed.remove(t) {
                    self.shell.settings.collapsed.insert(t);
                }
            }
            SetAction::Search => self.setctl.focus = None,
            SetAction::Close => self.shell.settings.open = false,
            SetAction::Act(act) => {
                if !matches!(act, crate::shell::settings_model::Act::Focus(_)) {
                    self.setctl.focus = None;
                }
                self.settings_act(act);
            }
            SetAction::Slider(t, root, x0, w) => {
                let x = self.seat.get_pointer().unwrap().current_location().x as f32;
                self.setctl.drag = Some((t.clone(), root, x0, w));
                self.settings_slider(&t, root, (x - x0) / w);
            }
        }
        self.dirty = true;
    }

    /// Modo de Pouca Energia (power-saver) via power-profiles-daemon.
    pub fn set_low_power(&mut self, on: bool) {
        self.low_power = on;
        crate::apps::spawn(&format!("powerprofilesctl set {}", if on { "power-saver" } else { "balanced" }));
        self.dirty = true;
    }

    /// VRR desejado: só com jogo em tela cheia (onde a taxa variável faz diferença).
    pub fn want_vrr(&self) -> bool {
        self.game.active && self.any_fullscreen_on_top()
    }

    /// Há animação em curso (precisa de quadros mesmo sem mudança de estado)?
    pub fn animating(&self) -> bool {
        let sh = &self.shell;
        sh.dock.needs_redraw()
            || sh.notch.animating()
            || (!self.game.active && sh.status.media.as_ref().is_some_and(|m| m.playing))
            || sh.menu.as_ref().is_some_and(|m| m.alpha() < 1.0)
            || (sh.cc.open && sh.cc.alpha() < 1.0)
            || (sh.launchpad.open && sh.launchpad.intro().0 < 1.0)
            || sh.mission.animating()
            || !self.banners.is_empty()
            || self.lock.animating()
            || self.space.elements().any(|w| {
                let m = meta(w).lock().unwrap();
                m.minimizing.is_some() || m.opened_at.is_some_and(|t| t.elapsed().as_millis() < 240) || m.closing_at.is_some()
            })
            || self.shell.capture.countdown.is_some()
            || self.recorder.is_some()
            || self.screenshot.is_some()
            || self.debug_capture.is_some()
    }

    /// ⇧⌘3: captura a tela inteira em ~/Imagens/Capturas de Tela (nome no estilo do macOS).
    pub fn request_screenshot(&mut self) {
        let home = std::path::PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| "/tmp".into()));
        let pics = ["Imagens", "Pictures"].iter().map(|d| home.join(d)).find(|p| p.is_dir()).unwrap_or(home.join("Imagens"));
        let dir = pics.join("Capturas de Tela");
        let _ = std::fs::create_dir_all(&dir);
        let name = chrono::Local::now().format("Captura de Tela %Y-%m-%d às %H.%M.%S.png").to_string();
        self.screenshot = Some(dir.join(name));
        self.screenshot_rect = None;
        self.dirty = true;
    }

    /// Captura de uma região (px lógicos); com timer, conta antes.
    pub fn capture_region(&mut self, rect: Option<(f32, f32, f32, f32)>) {
        let timer = self.shell.capture.timer;
        self.shell.capture.close();
        self.cursor_status = smithay::input::pointer::CursorImageStatus::default_named();
        if timer > 0 {
            self.shell.capture.countdown = Some((Instant::now(), timer, rect.unwrap_or((0.0, 0.0, self.screen.w as f32, self.screen.h as f32))));
            self.dirty = true;
            return;
        }
        self.request_screenshot();
        self.screenshot_rect = rect;
    }

    /// Nome do app em foco para a barra de menus ("Finder" do Obelisk = "Obelisk").
    pub fn focused_app_name(&self) -> String {
        let Some(w) = self.focused_window() else { return "Obelisk".into() };
        let id = Self::app_id(&w);
        if let Some(app) = crate::apps::for_app_id(&self.shell.apps, &id) {
            return app.name.clone();
        }
        let tail = id.rsplit('.').next().unwrap_or(&id).to_string();
        if tail.is_empty() {
            let t = Self::title(&w);
            return if t.is_empty() { "Obelisk".into() } else { t };
        }
        let mut cs = tail.chars();
        cs.next().map(|f| f.to_uppercase().collect::<String>() + cs.as_str()).unwrap_or(tail)
    }

    /// Ícone (nome no tema) da janela: o do .desktop, senão o próprio app_id.
    pub fn window_icon(&self, w: &Window) -> String {
        let id = Self::app_id(w);
        crate::apps::for_app_id(&self.shell.apps, &id).map(|a| a.icon.clone()).unwrap_or(id)
    }

    /// Todas as janelas vivas (mesa atual, outras mesas e minimizadas).
    pub fn all_windows(&self) -> Vec<Window> {
        self.space
            .elements()
            .cloned()
            .chain(self.ws_store.iter().flatten().map(|(w, _)| w.clone()))
            .chain(self.minimized.iter().cloned())
            .collect()
    }

    /// Itens do Dock: Launchpad · fixados · [abertos não fixados] · minimizados · Lixo.
    pub fn rebuild_dock(&mut self) {
        use crate::shell::dock::Entry;
        let wins: Vec<Window> = self.all_windows().into_iter().filter(Self::is_normal).collect();
        let ids: Vec<String> = wins.iter().map(Self::app_id).collect();
        let running = |app: &crate::apps::App| ids.iter().any(|i| crate::apps::matches(app, i));
        let mut entries = vec![Entry::Launchpad];
        for id in &self.shell.pinned {
            if let Some(app) = self.shell.app_by_id(id) {
                entries.push(Entry::App { app: app.clone(), pinned: true, running: running(app) });
            }
        }
        let mut extra: Vec<Entry> = Vec::new();
        for id in &ids {
            if let Some(app) = crate::apps::for_app_id(&self.shell.apps, id) {
                let listed = self.shell.pinned.contains(&app.id)
                    || extra.iter().any(|e| matches!(e, Entry::App { app: a, .. } if a.id == app.id));
                if !listed {
                    extra.push(Entry::App { app: app.clone(), pinned: false, running: true });
                }
            }
        }
        if !extra.is_empty() {
            entries.push(Entry::Separator);
            entries.extend(extra);
        }
        entries.push(Entry::Separator);
        for (i, w) in self.minimized.iter().enumerate() {
            entries.push(Entry::Minimized { index: i, label: Self::title(w), icon: self.window_icon(w) });
        }
        entries.push(Entry::Trash { full: self.shell.trash_full });
        // o app abriu: para de saltar
        for e in &entries {
            if let Entry::App { app, running: true, .. } = e {
                self.shell.dock.stop_bounce_for(&app.id);
            }
        }
        self.shell.dock.set_entries(entries);
    }

    /// Tarefas periódicas por quadro: relógio, estado do dock, limpeza.
    pub fn tick(&mut self) {
        self.check_updates();
        // apps internos abertos pelo Dock/Launchpad/Busca ("obelisk-open X" no .desktop)
        for what in crate::apps::take_internal() {
            self.open_internal(&what);
        }
        // ícones da mesa: relê a pasta (no máximo a cada 2 s)
        {
            let sh = &mut self.shell;
            if sh.desktop.scan(&mut sh.icons) {
                self.dirty = true;
            }
        }
        // fechar animado: depois de 150 ms pede ao app; se ele não fechar (ex.: "salvar?"), volta a aparecer
        let closing: Vec<Window> = self.space.elements().filter(|w| meta(w).lock().unwrap().closing_at.is_some()).cloned().collect();
        for w in closing {
            let (t, sent) = {
                let m = meta(&w).lock().unwrap();
                (m.closing_at.unwrap(), m.close_sent)
            };
            if !sent && t.elapsed().as_millis() >= 150 {
                meta(&w).lock().unwrap().close_sent = true;
                self.send_close(&w);
            } else if sent && t.elapsed().as_millis() >= 700 {
                meta(&w).lock().unwrap().closing_at = None;
                self.dirty = true;
            }
        }
        // Ajustes: páginas dinâmicas (Wi‑Fi, Bluetooth, som…) relidas a cada 4 s
        if self.shell.settings.open
            && crate::shell::settings_pages::live(self.shell.settings.page)
            && self.setctl.focus.is_none()
            && self.setctl.refreshed.map_or(true, |t| t.elapsed().as_secs() >= 4)
        {
            self.refresh_settings_page();
        }
        self.tick_banners();
        // Stage Manager: o palco começa depois da faixa de miniaturas
        if self.stage_manager {
            let stage: Vec<(Window, Point<i32, Logical>)> = self
                .space
                .elements()
                .filter(|w| self.on_stage(w) && Self::is_normal(w) && !meta(w).lock().unwrap().fullscreen)
                .filter_map(|w| self.space.element_location(w).filter(|l| l.x < 200).map(|l| (w.clone(), l)))
                .collect();
            for (w, l) in stage {
                self.space.map_element(w.clone(), (200, l.y), false);
                if let Some(x) = w.x11_surface() {
                    let _ = x.configure(Rectangle::new((200, l.y).into(), x.geometry().size));
                }
                self.dirty = true;
            }
        }
        // bandeja: relê ícones/estado a cada 5 s (não durante jogo); item sem dono some
        if !self.game.active && self.tray_refresh_at.elapsed().as_secs() >= 5 {
            self.tray_refresh_at = Instant::now();
            if let Some(t) = &self.tray {
                for i in &self.tray_items {
                    t.refresh(i.bus.clone(), i.path.clone());
                }
            }
        }
        // Monitor de Atividade: redesenha só quando a amostra de 1 s chega
        if self.shell.activity.open && self.shell.activity.sample() {
            self.dirty = true;
        }
        if self.auto_appearance {
            use chrono::Timelike;
            let h = chrono::Local::now().hour();
            let want_dark = !(7..19).contains(&h);
            let is_dark = self.shell.theme.appearance == crate::ui::theme::Appearance::Dark;
            if want_dark != is_dark {
                self.shell.theme = if want_dark { crate::ui::theme::Theme::dark() } else { crate::ui::theme::Theme::light() }.with_accent(&self.settings.accent);
                self.shell.decos.clear();
                self.dirty = true;
            }
        }
        // fim das minimizações animadas
        let done: Vec<Window> = self
            .space
            .elements()
            .filter(|w| meta(w).lock().unwrap().minimizing.is_some_and(|(t, _)| t.elapsed().as_millis() >= 300))
            .cloned()
            .collect();
        for w in done {
            meta(&w).lock().unwrap().minimizing = None;
            self.space.unmap_elem(&w);
            self.dirty = true;
        }
        if let Some((t0, secs, rect)) = self.shell.capture.countdown {
            if t0.elapsed().as_secs_f32() >= secs as f32 {
                self.shell.capture.countdown = None;
                self.request_screenshot();
                self.screenshot_rect = Some(rect);
            }
        }
        // bloqueio por inatividade (não durante jogo nem com app inibindo, ex.: vídeo)
        if self.lock_minutes > 0
            && !self.lock.active
            && !self.game.active
            && self.idle_inhibitors.is_empty()
            && self.last_input.elapsed().as_secs() >= self.lock_minutes * 60
        {
            self.lock_screen();
        }
        if let Some(w) = self.pending_focus.take() {
            if w.alive() {
                if w.wl_surface().is_some() {
                    self.focus_window(&w);
                    self.dirty = true;
                } else {
                    self.pending_focus = Some(w);
                }
            }
        }
        self.mru.retain(|w| w.alive());
        for ws in &mut self.ws_store {
            ws.retain(|(w, _)| w.alive());
        }
        self.space.refresh();
        self.popups.cleanup();
        self.minimized.retain(|w| w.alive());

        if self.shell.clock_at.elapsed().as_secs() >= 5 {
            let c = crate::shell::menubar::clock_string();
            if c != self.shell.clock {
                self.shell.clock = c;
                self.dirty = true;
            }
            self.shell.clock_at = Instant::now();
        }
        if self.shell.apps_loaded.elapsed().as_secs() > 120 {
            self.shell.apps = crate::apps::load();
            self.shell.apps_loaded = Instant::now();
        }

        let mut st = self.services.get();
        if let Some(m) = &self.debug_media {
            st.media = Some(m.clone());
        }
        // avisos do sistema no notch: fone/controle Bluetooth conectou ou desconectou
        if st.bt_available {
            if let Some(prev) = &self.bt_prev {
                let joined: Vec<String> = st.bt_connected.iter().filter(|d| !prev.contains(d)).cloned().collect();
                let left: Vec<String> = prev.iter().filter(|d| !st.bt_connected.contains(d)).cloned().collect();
                for d in joined {
                    self.notify_local("Bluetooth", &format!("{d} conectado"), false);
                }
                for d in left {
                    self.notify_local("Bluetooth", &format!("{d} desconectado"), false);
                }
            }
            self.bt_prev = Some(st.bt_connected.clone());
        }
        // cartão da captura: 8 s, pausado com o mouse em cima
        if let Some(t) = &mut self.capture_toast {
            if self.shell.notch.hovered_since.is_some() {
                t.3 = Instant::now();
            } else if t.3.elapsed().as_secs() >= 8 {
                self.capture_toast = None;
                self.dirty = true;
            }
        }
        if st != self.shell.status {
            self.shell.status = st;
            self.dirty = true;
        }
        self.shell.notch.observe(&self.shell.status);

        // Modo Jogo: liga/desliga conforme há janela de jogo
        let ids: Vec<String> = self.all_windows().iter().map(Self::app_id).collect();
        let by_content = self
            .all_windows()
            .iter()
            .any(|w| w.toplevel().is_some_and(|t| crate::handlers::gaming::is_game_content(t.wl_surface())));
        let running = self.settings.game_auto && (by_content || ids.iter().any(|i| self.game.is_game(i)));
        if let Some(on) = self.game.evaluate(running) {
            self.services.set_paused(on);
            self.dirty = true;
        }
        // PIDs dos jogos abertos → Feral GameMode
        let wins = self.all_windows();
        let mut games = Vec::new();
        for w in wins {
            let by_content = w.toplevel().is_some_and(|t| crate::handlers::gaming::is_game_content(t.wl_surface()));
            if by_content || self.game.is_game(&Self::app_id(&w)) {
                games.push(w);
            }
        }
        let pids: Vec<i32> = games.iter().filter_map(|w| self.window_pid(w)).collect();
        self.game.sync_pids(&pids);
        if self.shell.clock_at.elapsed().as_millis() < 20 {
            self.shell.trash_full = crate::shell::trash_full();
        }
        self.rebuild_dock();

        // limpa decorações de janelas que já não existem
        let alive: HashSet<_> = self.space.elements().chain(self.minimized.iter()).filter_map(Self::win_id).collect();
        self.shell.decos.retain(|id, _| alive.contains(id));
    }
}

#[derive(Default)]
pub struct ClientState {
    pub compositor_state: CompositorClientState,
    /// app em sandbox (Flatpak): sem captura de tela nem novos contextos
    pub security_context: Option<Arc<smithay::wayland::security_context::SecurityContext>>,
}

impl ClientData for ClientState {
    fn initialized(&self, _client_id: ClientId) {}
    fn disconnected(&self, _client_id: ClientId, _reason: DisconnectReason) {}
}

/// De onde vem o arquivo arrastado pelo compositor.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DragFrom {
    Shelf(usize),
    Desk(usize),
}

#[derive(Clone, Debug)]
pub struct DragOut {
    pub from: DragFrom,
    pub paths: Vec<String>,
    /// algum app aceitou (então não é soltura "no vazio")
    pub dropped: bool,
}

impl Obelisk {
    /// Começa a arrastar arquivos para fora (Bandeja ou mesa): qualquer app que aceite
    /// `text/uri-list` recebe; soltar sobre a mesa, o Lixo ou o notch é tratado aqui.
    pub fn start_drag_out(&mut self, from: DragFrom, serial: smithay::utils::Serial, button: u32, start: Point<f64, Logical>) {
        let (paths, icon): (Vec<String>, Option<resvg::tiny_skia::Pixmap>) = match from {
            DragFrom::Shelf(i) => (self.shelf.get(i).cloned().into_iter().collect(), None),
            DragFrom::Desk(i) => (
                self.shell.desktop.items.get(i).map(|it| it.path.clone()).into_iter().collect(),
                self.shell.desktop.icon_of(i).cloned(),
            ),
        };
        if paths.is_empty() {
            return;
        }
        // fantasma que segue o cursor
        if let Some(mut c) = crate::ui::canvas::Canvas::new(64, 64, self.scale as f32) {
            match &icon {
                Some(pm) => c.blit(pm, 4.0, 4.0, 56.0, 56.0, 0.75),
                None => {
                    c.fill_rrect(4.0, 4.0, 56.0, 56.0, 14.0, crate::ui::theme::Rgba(30, 30, 34, 200));
                    c.symbol(crate::shell::notch::file_symbol(&paths[0]), 32.0, 32.0, 32.0, crate::ui::theme::Rgba(0x0a, 0x84, 0xff, 255));
                }
            }
            self.drag_ghost.set(&c);
        }
        if let DragFrom::Desk(i) = from {
            self.shell.desktop.dragging = Some(i);
        }
        self.drag_out = Some(DragOut { from, paths, dropped: false });
        self.swallow_release = false; // a soltura precisa chegar ao arrasto
        let meta = smithay::wayland::selection::data_device::SourceMetadata {
            mime_types: vec!["text/uri-list".into(), "text/plain;charset=utf-8".into(), "text/plain".into()],
            dnd_action: smithay::reexports::wayland_server::protocol::wl_data_device_manager::DndAction::Copy
                | smithay::reexports::wayland_server::protocol::wl_data_device_manager::DndAction::Move,
        };
        let start_data = smithay::input::pointer::GrabStartData { focus: None, button, location: start };
        let dh = self.display_handle.clone();
        let seat = self.seat.clone();
        smithay::wayland::selection::data_device::start_dnd(&dh, &seat, self, serial, Some(start_data), None, meta);
        self.dirty = true;
    }

    /// Soltou sem nenhum app aceitar: mesa (move o ícone), Lixo do Dock ou notch (Bandeja).
    pub fn drag_out_landed(&mut self) {
        let Some(d) = self.drag_out.take() else { return };
        self.shell.desktop.dragging = None;
        self.dirty = true;
        if d.dropped {
            return;
        }
        let pos = self.last_pointer;
        // Lixo do Dock
        if let Some(i) = self.shell.dock.item_at(pos.x, pos.y) {
            if matches!(self.shell.dock.entries.get(i), Some(crate::shell::dock::Entry::Trash { .. })) {
                for p in &d.paths {
                    crate::apps::spawn(&format!("gio trash {}", crate::shell::settings_model::q(p)));
                }
                if let DragFrom::Shelf(_) = d.from {
                    self.shelf.retain(|f| !d.paths.contains(f));
                }
                return;
            }
        }
        // notch: guarda na Bandeja
        let x0 = self.screen.w as f32 / 2.0 - crate::shell::notch::OPEN_W / 2.0;
        if (pos.y as f32) < 60.0 && (pos.x as f32) >= x0 && (pos.x as f32) < x0 + crate::shell::notch::OPEN_W {
            for p in d.paths {
                if !self.shelf.contains(&p) {
                    self.shelf.push(p);
                }
            }
            self.shell.notch.tab = crate::shell::notch::Tab::Tray;
            self.shell.notch.open = true;
            return;
        }
        // mesa: o ícone vai para onde foi solto
        if let DragFrom::Desk(i) = d.from {
            self.shell.desktop.move_icon(i, pos.x as f32, pos.y as f32, 32.0 + 14.0);
        }
    }
}

impl Obelisk {
    /// Muda a escala da tela principal (1, 1,25, 1,5, 1,75, 2…). A UI é desenhada na escala inteira
    /// acima e a GPU reduz; os apps recebem a escala fracionária pelo wp-fractional-scale.
    pub fn set_output_scale(&mut self, f: f64) {
        let f = (f * 4.0).round() / 4.0;
        if !(1.0..=3.0).contains(&f) || (f - self.fscale).abs() < 0.01 {
            return;
        }
        self.fscale = f;
        self.scale = f.ceil() as i32;
        if let Some(o) = self.output.clone() {
            let sc = if f.fract() == 0.0 { smithay::output::Scale::Integer(f as i32) } else { smithay::output::Scale::Fractional(f) };
            o.change_current_state(None, None, Some(sc), None);
            if let Some(m) = o.current_mode() {
                self.screen = m.size.to_f64().to_logical(f).to_i32_round();
            }
            self.send_fractional_scale();
        }
        self.shell.cursor = Default::default();
        self.shell.cursor_layers.clear();
        self.shell.decos.clear();
        self.dirty = true;
    }
}

impl Obelisk {
    /// Depois de apresentar um quadro: gravação, captura (⇧⌘3/4/5) e pedidos de captura dos apps.
    /// Renderiza de novo numa textura fora da tela e lê dela (ler a superfície da janela quebra o EGL).
    pub fn after_render(
        &mut self,
        renderer: &mut smithay::backend::renderer::gles::GlesRenderer,
        elements: &[crate::render::ObeliskElement],
        size: smithay::utils::Size<i32, smithay::utils::Physical>,
        fsc: f64,
        output: &smithay::output::Output,
    ) {
        use smithay::utils::{Physical, Rectangle};
        let primary = self.output.as_ref() == Some(output);
        if primary {
            if let Some(rec) = self.recorder.as_mut().filter(|r| r.due()) {
                let (x, y, w, h) = rec.region;
                let crop = Rectangle::<i32, Physical>::new((x, y).into(), (w, h).into());
                match crate::capture::offscreen(renderer, elements, size, fsc, crop) {
                    Some(frame) => rec.push(frame),
                    None => tracing::warn!("gravação: leitura do quadro falhou"),
                }
            }
            if let Some(path) = self.screenshot.take() {
                let crop = match self.screenshot_rect.take() {
                    Some((x, y, w, h)) => {
                        let f = fsc as f32;
                        Rectangle::<i32, Physical>::new(((x * f) as i32, (y * f) as i32).into(), ((w * f).round() as i32, (h * f).round() as i32).into())
                    }
                    None => Rectangle::from_size(size),
                };
                let crop = crop.intersection(Rectangle::from_size(size)).unwrap_or(crop);
                let res = crate::capture::offscreen(renderer, elements, size, fsc, crop)
                    .ok_or_else(|| "leitura falhou".to_string())
                    .and_then(|d| crate::capture::save_rgba(d, crop.size.w, crop.size.h, &path));
                match res {
                    Ok(()) => {
                        tracing::info!("captura salva em {}", path.display());
                        crate::apps::spawn(&format!("wl-copy --type image/png < {}", crate::shell::settings_model::q(&path.display().to_string())));
                        if !self.debug_shot(&path) {
                            let p = path.display().to_string();
                            let thumb = crate::settings_ctl::load_image(&p, 224);
                            self.capture_toast = Some((p, thumb, true, Instant::now()));
                            self.dirty = true;
                        }
                    }
                    Err(e) => {
                        tracing::warn!("captura falhou: {e}");
                        if !self.debug_shot(&path) {
                            self.capture_toast = Some((path.display().to_string(), None, false, Instant::now()));
                            self.dirty = true;
                        }
                    }
                }
            }
        }
        if !self.screencopy.is_empty() {
            let mut read = |r: Rectangle<i32, Physical>| crate::capture::offscreen(renderer, elements, size, fsc, r);
            crate::screencopy::serve(self, output, &mut read);
        }
    }
}

/// Uma tela como aparece em Ajustes › Telas.
#[derive(Clone, Debug, PartialEq)]
pub struct DisplayInfo {
    pub name: String,
    /// posição e tamanho lógicos (a principal em 0,0)
    pub rect: Rectangle<i32, Logical>,
    pub primary: bool,
}

impl Obelisk {
    /// Telas para os Ajustes: as do backend nativo, ou a janela do modo aninhado.
    pub fn display_list(&self) -> Vec<DisplayInfo> {
        if !self.displays.is_empty() {
            return self.displays.clone();
        }
        let name = self.output.as_ref().map(|o| o.name()).unwrap_or_else(|| "Tela".into());
        vec![DisplayInfo { name, rect: Rectangle::from_size(self.screen), primary: true }]
    }

    /// Solta a tela `name` arrastada para (x, y) lógicos: encosta na borda mais próxima de outra
    /// tela (sem sobrepor), salva todas as posições relativas à principal e pede o novo arranjo.
    pub fn place_display(&mut self, name: &str, x: i32, y: i32) {
        let list = self.display_list();
        let Some(me) = list.iter().find(|d| d.name == name).cloned() else { return };
        if me.primary && list.len() == 1 {
            return;
        }
        let others: Vec<Rectangle<i32, Logical>> = list.iter().filter(|d| d.name != name).map(|d| d.rect).collect();
        let (w, h) = (me.rect.size.w, me.rect.size.h);
        let mut best: Option<((i32, i32), i64)> = None;
        for o in &others {
            let clamp_y = |y: i32| y.clamp(o.loc.y - h + 40, o.loc.y + o.size.h - 40);
            let clamp_x = |x: i32| x.clamp(o.loc.x - w + 40, o.loc.x + o.size.w - 40);
            let cands = [
                (o.loc.x - w, clamp_y(y)),        // à esquerda
                (o.loc.x + o.size.w, clamp_y(y)), // à direita
                (clamp_x(x), o.loc.y - h),        // em cima
                (clamp_x(x), o.loc.y + o.size.h), // embaixo
            ];
            for c in cands {
                let r = Rectangle::new(c.into(), me.rect.size);
                if others.iter().any(|p| p.overlaps(r)) {
                    continue;
                }
                let dist = ((c.0 - x) as i64).pow(2) + ((c.1 - y) as i64).pow(2);
                if best.is_none_or(|b| dist < b.1) {
                    best = Some((c, dist));
                }
            }
        }
        let Some(((nx, ny), _)) = best else { return };
        // mover a principal = mover todas as outras ao contrário (ela fica em 0,0)
        if me.primary {
            for d in list.iter().filter(|d| !d.primary) {
                self.settings.set(&format!("pos:{}", d.name), &format!("{},{}", d.rect.loc.x - nx, d.rect.loc.y - ny));
            }
        } else {
            self.settings.set(&format!("pos:{name}"), &format!("{nx},{ny}"));
        }
        self.settings.save();
        self.outputs_changed = true;
        self.dirty = true;
    }
}

impl Obelisk {
    /// Começa a instalação: grava a configuração e roda o `obelisk-install-auto` como root,
    /// lendo as linhas PROGRESS/ERROR/DONE numa thread.
    pub fn start_install(&mut self) {
        use smithay::reexports::calloop::channel;
        let cfg = match self.shell.installer.write_config() {
            Ok(p) => p,
            Err(e) => {
                self.shell.installer.error = Some(format!("Não deu para preparar a instalação: {e}"));
                self.shell.installer.step = 6;
                return;
            }
        };
        let inst = &mut self.shell.installer;
        inst.step = 5;
        inst.error = None;
        inst.done = false;
        inst.progress = (0.0, "Começando…".into());
        inst.started = Some(Instant::now());
        let (tx, rx) = channel::channel::<String>();
        let _ = self.loop_handle.insert_source(rx, |ev, _, state| {
            if let channel::Event::Msg(line) = ev {
                state.on_install_line(&line);
            }
        });
        std::thread::spawn(move || {
            use std::io::BufRead;
            let child = std::process::Command::new("sudo")
                .args(["-n", "obelisk-install-auto", &cfg])
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::null())
                .spawn();
            let mut child = match child {
                Ok(c) => c,
                Err(e) => {
                    let _ = std::fs::remove_file(&cfg);
                    let _ = tx.send(format!("ERROR Não deu para iniciar o instalador: {e}"));
                    return;
                }
            };
            let mut finished = false;
            if let Some(out) = child.stdout.take() {
                for line in std::io::BufReader::new(out).lines().map_while(Result::ok) {
                    finished |= line == "DONE" || line.starts_with("ERROR");
                    let _ = tx.send(line);
                }
            }
            let status = child.wait();
            let _ = std::fs::remove_file(&cfg);
            if !finished {
                let _ = tx.send(format!("ERROR O instalador parou sem terminar ({status:?})"));
            }
        });
        self.dirty = true;
    }

    fn on_install_line(&mut self, line: &str) {
        let inst = &mut self.shell.installer;
        if let Some(rest) = line.strip_prefix("PROGRESS ") {
            let (pct, msg) = rest.split_once(' ').unwrap_or((rest, ""));
            inst.progress = (pct.parse().unwrap_or(inst.progress.0), msg.to_string());
        } else if let Some(e) = line.strip_prefix("ERROR ") {
            inst.error = Some(e.to_string());
            inst.step = 6;
        } else if line == "DONE" {
            inst.done = true;
            inst.step = 6;
        }
        self.dirty = true;
    }
}
