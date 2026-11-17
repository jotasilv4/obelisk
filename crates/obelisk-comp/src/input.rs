//! Entrada: atalhos (os do kyro), ponteiro, hit-testing e ações de toda a UI do shell.
//! `Mod` = Super na sessão real (Alt quando aninhado, porque o host captura o Super).

use smithay::{
    backend::input::{
        AbsolutePositionEvent, Axis, AxisSource, ButtonState, Event, InputBackend, InputEvent, KeyState,
        KeyboardKeyEvent, PointerAxisEvent, PointerButtonEvent, PointerMotionEvent,
    },
    desktop::{Window, WindowSurfaceType},
    input::{
        keyboard::{FilterResult, Keysym, ModifiersState},
        pointer::{AxisFrame, ButtonEvent, Focus, GrabStartData as PointerGrabStartData, MotionEvent},
    },
    reexports::wayland_protocols::xdg::shell::server::xdg_toplevel,
    utils::{Logical, Point, Rectangle, SERIAL_COUNTER},
};

use crate::grabs::{resize_grab::ResizeEdge, MoveSurfaceGrab, ResizeSurfaceGrab};
use crate::shell::{
    control_center::{CcHit, Slider},
    deco,
    dock::Entry,
    launcher::{Action as LauncherAction, ShellAction},
    menu::{Menu, MenuAction, MenuItem},
    menubar::BarItem,
    mission::{self, McWindow},
    switcher::SwitchItem,
};
use crate::state::{meta, BackendKind, Obelisk};
use crate::ui::theme::{metrics, Appearance, Theme};

const BTN_LEFT: u32 = 0x110;
const BTN_RIGHT: u32 = 0x111;
const EDGE: i32 = 5;

#[derive(Debug)]
pub enum Hit {
    Lock,
    Installer,
    Welcome,
    AboutApp,
    DeskIcon(usize),
    ForceQuit,
    Capture,
    Switcher,
    Mission,
    Launchpad,
    Launcher,
    Menu,
    Cc,
    Nc,
    Activity,
    Emoji,
    Settings,
    Notch,
    Bar(Option<BarItem>),
    Dock(Option<usize>),
    Titlebar { window: Window, button: Option<usize> },
    Edge { window: Window, edges: ResizeEdge },
    StageThumb(Window),
    Surface(Window),
    Desktop,
}

#[derive(Debug)]
enum KeyAction {
    AuthSubmit,
    AuthCancel,
    AuthBackspace,
    AuthText(String),
    Zoom8,
    ZoomStep(f64),
    AppExpose,
    WorkspaceStep(i32),
    QuitApp,
    Emoji,
    CaptureArea,
    CaptureBar,
    CaptureClose,
    CaptureGo,
    CaptureMode(i32),
    LockScreen,
    LockText(String),
    LockBackspace,
    LockSubmit,
    LockUser(i32),
    WelcomeNext,
    InstallerEnter,
    InstallerTab,
    InstallerBackspace,
    InstallerText(String),
    LockClear,
    Volume(f32),
    Mute,
    Brightness(f32),
    Media(&'static str),
    Tile(crate::tiling::Tile),
    ForceQuit,
    Untile,
    Spotlight,
    Launchpad,
    MissionControl,
    SwitcherNext(bool),
    SwitcherCommit,
    SwitcherCancel,
    Settings,
    Terminal,
    Files,
    CloseWindow,
    Minimize,
    HideApp,
    Fullscreen,
    Zoom,
    CycleAppWindows,
    Workspace(usize),
    MoveToWorkspace(usize),
    Screenshot,
    Logout,
    Escape,
    Text(String),
    Backspace,
    Enter,
    Move(i32),
    Tab(bool),
    Swallow,
}

fn digit(k: Keysym) -> Option<usize> {
    let raw = k.raw();
    (Keysym::_1.raw()..=Keysym::_9.raw()).contains(&raw).then(|| (raw - Keysym::_1.raw()) as usize)
}

impl Obelisk {
    /// Abre partes da UI na inicialização (testes visuais).
    /// `launcher:texto`, `menu:logo|app|window|go`, `launchpad`, `mission`, `cc`, `switcher`.
    pub fn debug_ui(&mut self, spec: &str) {
        if spec.contains(';') {
            for part in spec.split(';') {
                self.debug_ui(part);
            }
            return;
        }
        let apps = std::mem::take(&mut self.shell.apps);
        if let Some(q) = spec.strip_prefix("launcher:") {
            self.shell.launcher.toggle(&apps);
            self.shell.launcher.push_str(q, &apps);
        }
        self.shell.apps = apps;
        match spec {
            "launchpad" => self.shell.launchpad.toggle(),
            "mission" => self.open_mission(),
            "cc" => self.shell.cc.toggle(),
            "switcher" => self.open_switcher(),
            "menu:logo" => self.open_bar_menu(BarItem::Logo),
            "menu:app" => self.open_bar_menu(BarItem::App),
            "menu:window" => self.open_bar_menu(BarItem::Window),
            "menu:go" => self.open_bar_menu(BarItem::Go),
            "menu:file" => self.open_bar_menu(BarItem::File),
            "menu:edit" => self.open_bar_menu(BarItem::Edit),
            "menu:view" => self.open_bar_menu(BarItem::View),
            "menu:help" => self.open_bar_menu(BarItem::Help),
            "dockmenu" => self.open_dock_menu(1),
            "notch" => self.shell.notch.open = true,
            "aboutapp" => self.on_menu_action(MenuAction::AboutApp),
            sp if sp.starts_with("open:") => self.open_internal(&sp[5..]),
            "jobtest" => self.start_job("Instalando Heroic".into(), "for p in 12 35 61; do printf 'Baixando com.heroicgameslauncher.hgl %s%%\\r' $p; sleep 1; done; sleep 30".into()),
            "checkupdates" => {
                self.force_update_check = true;
                self.setctl.updates_checked = None;
            }
            sp if sp.starts_with("installer") => {
                self.shell.installer.show();
                let inst = &mut self.shell.installer;
                inst.step = sp.strip_prefix("installer:").and_then(|n| n.parse().ok()).unwrap_or(0);
                // dados de exemplo para as capturas (no modo demonstração, nunca os discos reais)
                if inst.disks.is_empty() || crate::lock::demo().is_some() {
                    inst.disks = vec![
                        crate::shell::installer::Disk { path: "/dev/nvme0n1".into(), model: "Samsung SSD 980 PRO 1TB".into(), size_gb: 1000.2, kind: "SSD NVMe".into() },
                        crate::shell::installer::Disk { path: "/dev/sda".into(), model: "Kingston A400".into(), size_gb: 240.1, kind: "Disco".into() },
                    ];
                }
                inst.disk = Some(0);
                inst.fields = ["Maria Souza".into(), "maria".into(), "1234".into(), "1234".into(), "obelisk".into()];
                if inst.step == 5 {
                    inst.progress = (46.0, "Instalando mesa (212 de 461)".into());
                    inst.started = Some(std::time::Instant::now());
                }
                if inst.step == 1 {
                    inst.zone_query = "sao".into();
                    inst.focus = Some(5);
                }
            }
            sp if sp.starts_with("dragdisplay:") => {
                let mut it = sp[12..].split(':');
                let name = it.next().unwrap_or("").to_string();
                let (x, y) = it.next().and_then(|v| v.split_once(',')).map(|(x, y)| (x.parse().unwrap_or(0), y.parse().unwrap_or(0))).unwrap_or((0, 0));
                self.setctl.display_preview = Some((name.clone(), (x, y)));
                self.setctl.display_drag = Some((name, (0.0, 0.0), (x, y)));
            }
            sp if sp.starts_with("placedisplay:") => {
                let mut it = sp[13..].split(':');
                let name = it.next().unwrap_or("").to_string();
                let (x, y) = it.next().and_then(|v| v.split_once(',')).map(|(x, y)| (x.parse().unwrap_or(0), y.parse().unwrap_or(0))).unwrap_or((0, 0));
                self.place_display(&name, x, y);
                tracing::warn!("teste: pos:{name} = {:?}", self.settings.get(&format!("pos:{name}")));
            }
            "fakedisplays" => {
                // testes da tela de arranjo no modo aninhado: uma tela 1920×1080 à direita
                let main = smithay::utils::Rectangle::from_size(self.screen);
                self.displays = vec![
                    crate::state::DisplayInfo { name: "eDP-1".into(), rect: main, primary: true },
                    crate::state::DisplayInfo { name: "HDMI-A-1".into(), rect: smithay::utils::Rectangle::new((self.screen.w, -200).into(), (1920, 1080).into()), primary: false },
                ];
                self.output_modes = vec![
                    ("eDP-1".into(), vec!["2560x1600@120".into(), "1920x1200@60".into()], "2560x1600@120".into(), 1.5),
                    ("HDMI-A-1".into(), vec!["1920x1080@144".into(), "1920x1080@60".into()], "1920x1080@144".into(), 1.0),
                ];
            }
            sp if sp.starts_with("welcome") => {
                self.shell.welcome.open = true;
                self.shell.welcome.step = sp.strip_prefix("welcome:").and_then(|n| n.parse().ok()).unwrap_or(0);
            }
            sp if sp.starts_with("lockpw:") => {
                self.lock.password = sp[7..].to_string();
                self.lock.submit();
            }
            // testes do arrasto: Bandeja → janela em foco; ícone da mesa → (400, 500)
            sp if sp.starts_with("dragshelf:") => {
                self.shelf = vec![sp[10..].to_string()];
                let to = self.focused_window().and_then(|w| self.space.element_geometry(&w)).map(|g| (g.loc.x as f64 + g.size.w as f64 / 2.0, g.loc.y as f64 + g.size.h as f64 / 2.0).into());
                self.debug_drag(crate::state::DragFrom::Shelf(0), to.unwrap_or((500.0, 500.0).into()));
            }
            "dragdesk" => self.debug_drag(crate::state::DragFrom::Desk(0), (400.0, 500.0).into()),
            "deskrename" => {
                self.shell.desktop.selected = Some(0);
                self.start_desk_rename(0);
                if let Some((_, t)) = &mut self.shell.desktop.renaming {
                    t.push_str(" novo");
                }
            }
            "deskmenu" => self.open_desktop_menu((300.0, 300.0).into(), None),
            "notch:tray" => {
                self.shell.notch.open = true;
                self.shell.notch.tab = crate::shell::notch::Tab::Tray;
            }
            sp if sp.starts_with("notch:shelf:") => {
                self.shelf = sp[12..].split(',').map(str::to_string).collect();
                self.shell.notch.open = true;
                self.shell.notch.tab = crate::shell::notch::Tab::Tray;
            }
            sp if sp.starts_with("notch:drop:") => self.debug_drop = sp[11..].parse().ok(),
            sp if sp.starts_with("notch:capture:") => {
                let p = sp[14..].to_string();
                let thumb = crate::settings_ctl::load_image(&p, 224);
                self.capture_toast = Some((p, thumb, true, std::time::Instant::now() + std::time::Duration::from_secs(60)));
            }
            sp if sp.starts_with("notch:music:") => {
                // notch:music:CAPA|Título|Artista|Álbum|aberto
                let f: Vec<&str> = sp[12..].split('|').collect();
                let m = crate::services::Media {
                    playing: true,
                    art: f.first().copied().unwrap_or("").into(),
                    title: f.get(1).copied().unwrap_or("Música").into(),
                    artist: f.get(2).copied().unwrap_or("").into(),
                    album: f.get(3).copied().unwrap_or("").into(),
                    player: String::new(),
                };
                self.debug_media = Some(m);
                if f.get(4) == Some(&"aberto") {
                    self.shell.notch.open = true;
                }
            }
            "hud" => self.shell.notch.show_hud(crate::shell::notch::HudKind::Volume, 0.6),
            "game" => self.toggle_game_mode(),
            "forcequit" => self.open_force_quit(),
            "nc" => self.shell.nc.toggle(),
            "lock" => self.lock_screen(),
            "activity" => self.shell.activity.toggle(),
            "emoji" => self.shell.emoji.toggle(),
            "settings" => self.open_settings(),
            sp if sp.starts_with("settings:") => {
                if !self.shell.settings.open {
                    self.shell.settings.toggle();
                }
                let (page, rest) = sp[9..].split_once('#').unwrap_or((&sp[9..], ""));
                self.settings_act(crate::shell::settings_model::Act::Page(page.to_string()));
                if let Some(id) = rest.strip_prefix("focus=") {
                    self.setctl.focus = Some(id.to_string());
                } else if let Some(k) = rest.strip_prefix("expand=") {
                    self.setctl.expanded = Some(k.to_string());
                    self.refresh_settings_page();
                } else if let Some(t) = rest.strip_prefix("collapse=") {
                    self.shell.settings.collapsed.insert(crate::shell::settings_pages::SECTIONS.iter().find(|x| x.0 == t).map(|x| x.0).unwrap_or(""));
                } else if let Some(n) = rest.strip_prefix("side=") {
                    self.shell.settings.side_scroll = n.parse().unwrap_or(0.0);
                } else if let Some(qy) = rest.strip_prefix("search=") {
                    self.shell.settings.query = qy.to_string();
                }
            }
            "widgets" => self.settings.widgets = true,
            "zoom" => self.zoom = 2.0,
            "stage" => self.stage_manager = true,
            "ontop" => {
                if let Some(w) = self.focused_window() {
                    meta(&w).lock().unwrap().on_top = true;
                }
            }
            "traymenu" => {
                if let (Some(item), Some(t)) = (self.tray_items.first().cloned(), &self.tray) {
                    t.open_menu(&item);
                }
            }
            "emojitest" => {
                let term = self.focused_is_terminal();
                self.insert_text("echo 🦀 colado pelo Obelisk", term);
            }
            "capbar" => self.shell.capture.start_bar(),
            "rec" => {
                self.start_recording(Some((100.0, 80.0, 600.0, 400.0)));
                let _ = self.loop_handle.insert_source(
                    smithay::reexports::calloop::timer::Timer::from_duration(std::time::Duration::from_secs(2)),
                    |_, _, state| {
                        state.stop_recording();
                        smithay::reexports::calloop::timer::TimeoutAction::Drop
                    },
                );
            }
            "recstop" => self.stop_recording(),
            "minimize" => {
                if let Some(w) = self.focused_window() {
                    self.minimize(&w);
                }
            }
            "capcrop" => self.capture_region(Some((100.0, 80.0, 400.0, 250.0))),
            "caparea" => {
                self.shell.capture.start_area();
                self.shell.capture.drag_start = Some((300.0, 200.0));
                self.shell.capture.cursor = (700.0, 520.0);
            }
            "tile" => {
                let ws: Vec<Window> = self.space.elements().cloned().collect();
                let tiles = [crate::tiling::Tile::Left, crate::tiling::Tile::TopRight, crate::tiling::Tile::BottomRight];
                for (w, t) in ws.iter().zip(tiles) {
                    self.tile(w, t);
                }
            }
            _ => {}
        }
    }

    // ---------------------------------------------------------------- hit-test

    pub fn hit_test(&self, pos: Point<f64, Logical>) -> Hit {
        if self.shell.installer.open && !self.lock.active && !self.shell.settings.contains(pos.x, pos.y) {
            return Hit::Installer;
        }
        if self.shell.welcome.open && !self.lock.active {
            return Hit::Welcome;
        }
        if self.lock.active {
            return Hit::Lock;
        }
        if self.shell.capture.active {
            return Hit::Capture;
        }
        let sh = &self.shell;
        if sh.switcher.open {
            return Hit::Switcher;
        }
        if sh.mission.open {
            return Hit::Mission;
        }
        if sh.launchpad.open {
            return Hit::Launchpad;
        }
        if sh.launcher.open && sh.launcher.contains(pos.x, pos.y) {
            return Hit::Launcher;
        }
        if sh.menu.as_ref().is_some_and(|m| m.contains(pos.x, pos.y)) {
            return Hit::Menu;
        }
        if sh.cc.contains(pos.x, pos.y) {
            return Hit::Cc;
        }
        if sh.nc.contains(pos.x, pos.y) {
            return Hit::Nc;
        }
        if sh.about_app.contains(pos.x, pos.y) {
            return Hit::AboutApp;
        }
        if sh.force_quit.contains(pos.x, pos.y) {
            return Hit::ForceQuit;
        }
        if sh.activity.contains(pos.x, pos.y) {
            return Hit::Activity;
        }
        if sh.emoji.contains(pos.x, pos.y) {
            return Hit::Emoji;
        }
        if sh.settings.contains(pos.x, pos.y) {
            return Hit::Settings;
        }
        let fullscreen = self.any_fullscreen_on_top();
        if !fullscreen {
            if sh.notch.contains(pos.x, pos.y) {
                return Hit::Notch;
            }
            if pos.y < metrics::MENUBAR_H as f64 {
                return Hit::Bar(sh.menubar.item_at(pos.x));
            }
            if sh.dock.contains(pos.x, pos.y) {
                return Hit::Dock(sh.dock.item_at(pos.x, pos.y));
            }
        }
        for (w, r) in self.stage_strip() {
            if (pos.x as f32) >= r.0 && (pos.x as f32) < r.0 + r.2 && (pos.y as f32) >= r.1 && (pos.y as f32) < r.1 + r.3 {
                return Hit::StageThumb(w);
            }
        }
        for w in self.space.elements().rev() {
            if meta(w).lock().unwrap().minimizing.is_some() || !self.on_stage(w) {
                continue;
            }
            let Some(loc) = self.space.element_location(w) else { continue };
            if w.surface_under(pos - loc.to_f64(), WindowSurfaceType::ALL).is_some() {
                return Hit::Surface(w.clone());
            }
            if !self.is_ssd(w) {
                continue;
            }
            let size = w.geometry().size;
            let frame = Rectangle::<i32, Logical>::new((loc.x, loc.y - metrics::TITLEBAR_H).into(), (size.w, size.h + metrics::TITLEBAR_H).into());
            let maximized = meta(w).lock().unwrap().maximized;
            let b = if maximized { 0 } else { EDGE };
            let outer = Rectangle::<i32, Logical>::new((frame.loc.x - b, frame.loc.y - b).into(), (frame.size.w + 2 * b, frame.size.h + 2 * b).into());
            if !outer.to_f64().contains(pos) {
                continue;
            }
            if frame.to_f64().contains(pos) {
                if pos.y < loc.y as f64 {
                    let rel = pos - frame.loc.to_f64();
                    return Hit::Titlebar { window: w.clone(), button: deco::button_at(rel.x, rel.y) };
                }
                continue;
            }
            let mut edges = ResizeEdge::empty();
            if pos.x < frame.loc.x as f64 {
                edges |= ResizeEdge::LEFT;
            }
            if pos.x >= (frame.loc.x + frame.size.w) as f64 {
                edges |= ResizeEdge::RIGHT;
            }
            if pos.y < frame.loc.y as f64 {
                edges |= ResizeEdge::TOP;
            }
            if pos.y >= (frame.loc.y + frame.size.h) as f64 {
                edges |= ResizeEdge::BOTTOM;
            }
            return Hit::Edge { window: w.clone(), edges };
        }
        if !self.game.active {
            if let Some(i) = self.shell.desktop.hit(pos.x, pos.y) {
                return Hit::DeskIcon(i);
            }
        }
        Hit::Desktop
    }

    // ---------------------------------------------------------------- eventos

    pub fn process_input_event<I: InputBackend>(&mut self, event: InputEvent<I>) {
        self.last_input = std::time::Instant::now();
        match event {
            InputEvent::Keyboard { event, .. } => self.on_key(event.key_code(), event.state(), Event::time_msec(&event)),
            InputEvent::PointerMotion { event, .. } => {
                use smithay::input::pointer::RelativeMotionEvent;
                use smithay::wayland::pointer_constraints::{with_pointer_constraint, PointerConstraint};
                let pointer = self.seat.get_pointer().unwrap();
                let cur = pointer.current_location();
                // trava/confinamento pedidos pelo jogo (mira em FPS)
                let mut locked = false;
                let mut confine: Option<Rectangle<f64, Logical>> = None;
                if let Some(surface) = pointer.current_focus() {
                    let kind = with_pointer_constraint(&surface, &pointer, |c| match c {
                        Some(c) if c.is_active() => match &*c {
                            PointerConstraint::Locked(_) => 1,
                            PointerConstraint::Confined(_) => 2,
                        },
                        _ => 0,
                    });
                    locked = kind == 1;
                    if kind == 2 {
                        if let Some(w) = self.window_of(&surface) {
                            if let Some(loc) = self.space.element_location(&w) {
                                confine = Some(Rectangle::new(loc, w.geometry().size).to_f64());
                            }
                        }
                    }
                }
                let under = pointer.current_focus().map(|s| (s, cur));
                pointer.relative_motion(
                    self,
                    under,
                    &RelativeMotionEvent { delta: event.delta(), delta_unaccel: event.delta_unaccel(), utime: event.time() },
                );
                if locked {
                    pointer.frame(self);
                    return;
                }
                let mut next = cur + event.delta();
                if let Some(r) = confine {
                    next.x = next.x.clamp(r.loc.x, r.loc.x + r.size.w - 1.0);
                    next.y = next.y.clamp(r.loc.y, r.loc.y + r.size.h - 1.0);
                }
                self.on_motion(next, event.time_msec());
            }
            InputEvent::PointerMotionAbsolute { event, .. } => {
                let pos = event.position_transformed(self.desktop_bounds().size);
                self.on_motion(pos, event.time_msec());
            }
            InputEvent::PointerButton { event, .. } => self.on_button(event.button_code(), event.state(), event.time_msec()),
            InputEvent::GestureSwipeBegin { event, .. } => {
                use smithay::backend::input::GestureBeginEvent;
                self.gesture = (0.0, 0.0, event.fingers());
            }
            InputEvent::GestureSwipeUpdate { event, .. } => {
                use smithay::backend::input::GestureSwipeUpdateEvent;
                let d = event.delta();
                self.gesture.0 += d.x;
                self.gesture.1 += d.y;
            }
            InputEvent::GestureSwipeEnd { event, .. } => {
                use smithay::backend::input::GestureEndEvent;
                let (dx, dy, fingers) = self.gesture;
                if !event.cancelled() && fingers >= 3 && !self.game.active {
                    // gestos do trackpad do macOS: 3 dedos ↑ Mission Control, ↓ App Exposé/fecha, ←/→ mesa
                    if dy < -80.0 && dy.abs() > dx.abs() {
                        if self.shell.mission.open { self.shell.mission.toggle() } else { self.open_mission() }
                    } else if dy > 80.0 && dy.abs() > dx.abs() {
                        if self.shell.mission.open {
                            self.shell.mission.toggle();
                        } else if let Some(w) = self.focused_window() {
                            self.expose_app = crate::apps::for_app_id(&self.shell.apps, &Self::app_id(&w)).map(|a| a.id.clone());
                            self.open_mission();
                        }
                    } else if dx.abs() > 120.0 {
                        let n = self.workspace as i64 + if dx < 0.0 { 1 } else { -1 };
                        if (0..9).contains(&n) {
                            self.switch_workspace(n as usize);
                        }
                    }
                }
                self.gesture = (0.0, 0.0, 0);
            }
            InputEvent::GesturePinchUpdate { event, .. } => {
                use smithay::backend::input::GesturePinchUpdateEvent;
                self.gesture.0 = event.scale();
            }
            InputEvent::GesturePinchEnd { event, .. } => {
                use smithay::backend::input::GestureEndEvent;
                // pinça com 4+ dedos (polegar e três dedos) abre o Launchpad
                if !event.cancelled() && self.gesture.0 > 0.0 && self.gesture.0 < 0.7 && !self.game.active {
                    self.toggle_launchpad();
                }
                self.gesture = (0.0, 0.0, 0);
            }
            InputEvent::PointerAxis { event, .. } => {
                let source = event.source();
                let h = event.amount(Axis::Horizontal).unwrap_or_else(|| event.amount_v120(Axis::Horizontal).unwrap_or(0.0) * 15.0 / 120.);
                let v = event.amount(Axis::Vertical).unwrap_or_else(|| event.amount_v120(Axis::Vertical).unwrap_or(0.0) * 15.0 / 120.);
                if self.on_ui_scroll(v) {
                    return;
                }
                let (h, v) = if self.natural_scroll && self.input_devices.is_empty() { (-h, -v) } else { (h, v) };
                let mut frame = AxisFrame::new(event.time_msec()).source(source);
                if h != 0.0 {
                    frame = frame.value(Axis::Horizontal, h);
                    if let Some(d) = event.amount_v120(Axis::Horizontal) {
                        frame = frame.v120(Axis::Horizontal, if self.natural_scroll && self.input_devices.is_empty() { -d } else { d } as i32);
                    }
                }
                if v != 0.0 {
                    frame = frame.value(Axis::Vertical, v);
                    if let Some(d) = event.amount_v120(Axis::Vertical) {
                        frame = frame.v120(Axis::Vertical, if self.natural_scroll && self.input_devices.is_empty() { -d } else { d } as i32);
                    }
                }
                if source == AxisSource::Finger {
                    if event.amount(Axis::Horizontal) == Some(0.0) {
                        frame = frame.stop(Axis::Horizontal);
                    }
                    if event.amount(Axis::Vertical) == Some(0.0) {
                        frame = frame.stop(Axis::Vertical);
                    }
                }
                let pointer = self.seat.get_pointer().unwrap();
                pointer.axis(self, frame);
                pointer.frame(self);
            }
            _ => {}
        }
    }

    /// Rolagem consumida pela UI: Launchpad rola a grade; ícone do som muda o volume.
    fn on_ui_scroll(&mut self, v: f64) -> bool {
        if v == 0.0 {
            return false;
        }
        let pos = self.seat.get_pointer().unwrap().current_location();
        match self.hit_test(pos) {
            Hit::Launchpad => {
                self.shell.launchpad.scroll = (self.shell.launchpad.scroll + v as f32 * 2.0).max(0.0);
                true
            }
            Hit::Settings => {
                if self.shell.settings.in_content(pos.x) {
                    self.shell.settings.scroll_by(v as f32 * 2.0);
                } else {
                    self.shell.settings.scroll_side(v as f32 * 2.0);
                }
                self.dirty = true;
                true
            }
            Hit::Notch => {
                let v = self.services.volume_step(-(v.signum() as f32) * 0.05);
                self.shell.notch.show_hud(crate::shell::notch::HudKind::Volume, v);
                true
            }
            Hit::Bar(Some(BarItem::Sound)) => {
                let cur = self.shell.status.volume.unwrap_or(0.5);
                self.services.set_volume(cur - (v.signum() as f32) * 0.05);
                true
            }
            Hit::Mission | Hit::Switcher | Hit::Menu | Hit::Cc | Hit::Nc | Hit::Activity | Hit::Emoji | Hit::Launcher | Hit::Bar(_) | Hit::Dock(_) => true,
            _ => false,
        }
    }

    // ---------------------------------------------------------------- teclado

    /// O app em foco é um terminal (⌘C/⌘V viram Ctrl+Shift+C/V)?
    pub fn focused_is_terminal(&self) -> bool {
        const TERMS: [&str; 9] = ["foot", "footclient", "kitty", "alacritty", "org.wezfurlong.wezterm", "org.gnome.ptyxis", "org.gnome.console", "xterm", "com.mitchellh.ghostty"];
        self.focused_window().is_some_and(|w| TERMS.contains(&Self::app_id(&w).to_lowercase().as_str()))
    }

    /// Modificadores como o shell deve ver: ⌘ real e Ctrl/Shift físicos (sem os sintéticos).
    fn effective_mods(&self, mods: &ModifiersState) -> ModifiersState {
        let mut m = *mods;
        if self.cmd_as_ctrl {
            m.logo = self.cmd_down;
            m.ctrl = self.phys_mods.0;
            m.shift = self.phys_mods.1;
        }
        m
    }

    fn on_key(&mut self, code: smithay::input::keyboard::Keycode, state: KeyState, time: u32) {
        let serial = SERIAL_COUNTER.next_serial();
        let keyboard = self.seat.get_keyboard().unwrap();
        let raw: u32 = code.raw();
        let pressed = state == KeyState::Pressed;
        // keycodes xkb (= evdev + 8): Ctrl 37/105, Shift 50/62, Alt 64/108, Super 133/134
        match raw {
            37 | 105 => self.phys_mods.0 = pressed,
            50 | 62 => self.phys_mods.1 = pressed,
            64 | 108 => self.phys_mods.2 = pressed,
            _ => {}
        }
        if self.cmd_as_ctrl && (raw == 133 || raw == 134) {
            self.cmd_down = pressed;
            if pressed {
                self.cmd_synth = if self.focused_is_terminal() { vec![37, 50] } else { vec![37] };
            }
            if !pressed && self.shell.switcher.open {
                self.run_key_action(KeyAction::SwitcherCommit);
            }
            let codes = if pressed { self.cmd_synth.clone() } else { std::mem::take(&mut self.cmd_synth) };
            if !self.lock.active {
                for c in codes {
                    let kc = smithay::input::keyboard::Keycode::new(c);
                    keyboard.input::<(), _>(self, kc, state, serial, time, |_, _, _| FilterResult::Forward);
                }
            }
            return;
        }
        let released_intercept = state == KeyState::Released && self.intercepted.remove(&raw);
        let action = keyboard.input::<KeyAction, _>(self, code, state, serial, time, |data, mods, handle| {
            let mods = &data.effective_mods(mods);
            let mod_down = if data.mod_alt { mods.alt } else { mods.logo };
            if state == KeyState::Released {
                // soltar ⌘ confirma o ⌘Tab
                if data.shell.switcher.open && !mod_down {
                    return FilterResult::Intercept(KeyAction::SwitcherCommit);
                }
                return if released_intercept { FilterResult::Intercept(KeyAction::Swallow) } else { FilterResult::Forward };
            }
            match data.key_action(mods, handle.modified_sym(), handle.raw_latin_sym_or_raw_current_sym()) {
                Some(a) => FilterResult::Intercept(a),
                None => FilterResult::Forward,
            }
        });
        if let Some(a) = action {
            if state == KeyState::Pressed {
                self.intercepted.insert(raw);
            }
            self.run_key_action(a);
        }
    }

    fn key_action(&self, mods: &ModifiersState, sym: Keysym, latin: Option<Keysym>) -> Option<KeyAction> {
        let m = if self.mod_alt { mods.alt } else { mods.logo };
        let key = latin.unwrap_or(sym);

        // VM / acesso remoto em foco pediu todos os atalhos: só ⌥⌘⎋ continua do sistema
        if self.shortcuts_inhibited() && self.auth.is_none() && !self.lock.active {
            let force_quit = m && mods.alt && sym == Keysym::Escape;
            if !force_quit {
                return None;
            }
        }

        // pedido de senha do polkit (no notch)
        if self.auth.is_some() {
            return Some(match sym {
                Keysym::Return | Keysym::KP_Enter => KeyAction::AuthSubmit,
                Keysym::Escape => KeyAction::AuthCancel,
                Keysym::BackSpace => KeyAction::AuthBackspace,
                s => match s.key_char() {
                    Some(c) if !c.is_control() && !mods.ctrl && !mods.logo => KeyAction::AuthText(c.to_string()),
                    _ => KeyAction::Swallow,
                },
            });
        }
        // tela bloqueada: o teclado vai só para a senha
        if self.lock.active {
            return Some(match sym {
                Keysym::Return | Keysym::KP_Enter => KeyAction::LockSubmit,
                Keysym::BackSpace => KeyAction::LockBackspace,
                Keysym::Left if self.lock.greeter.is_some() => KeyAction::LockUser(-1),
                Keysym::Right if self.lock.greeter.is_some() => KeyAction::LockUser(1),
                Keysym::Escape => KeyAction::LockClear,
                s if mods.ctrl || mods.logo || mods.alt => {
                    let _ = s;
                    KeyAction::Swallow
                }
                s => match s.key_char() {
                    Some(c) if !c.is_control() => KeyAction::LockText(c.to_string()),
                    _ => KeyAction::Swallow,
                },
            });
        }
        if self.shell.capture.active {
            return Some(match sym {
                Keysym::Escape => KeyAction::CaptureClose,
                Keysym::Return | Keysym::KP_Enter => KeyAction::CaptureGo,
                Keysym::Left => KeyAction::CaptureMode(-1),
                Keysym::Right => KeyAction::CaptureMode(1),
                _ => KeyAction::Swallow,
            });
        }
        if sym == Keysym::Print {
            return Some(KeyAction::CaptureBar);
        }
        // ⌥⌘Esc: Forçar Encerrar (o jogo travou)
        if sym == Keysym::Escape && mods.logo && mods.alt || (self.mod_alt && sym == Keysym::Escape && mods.alt && mods.ctrl) {
            return Some(KeyAction::ForceQuit);
        }
        // teclas de mídia, volume e brilho (com HUD no notch), em qualquer contexto
        match sym {
            Keysym::XF86_AudioRaiseVolume => return Some(KeyAction::Volume(0.05)),
            Keysym::XF86_AudioLowerVolume => return Some(KeyAction::Volume(-0.05)),
            Keysym::XF86_AudioMute => return Some(KeyAction::Mute),
            Keysym::XF86_MonBrightnessUp => return Some(KeyAction::Brightness(0.05)),
            Keysym::XF86_MonBrightnessDown => return Some(KeyAction::Brightness(-0.05)),
            Keysym::XF86_AudioPlay | Keysym::XF86_AudioPause => return Some(KeyAction::Media("play-pause")),
            Keysym::XF86_AudioNext => return Some(KeyAction::Media("next")),
            Keysym::XF86_AudioPrev => return Some(KeyAction::Media("previous")),
            _ => {}
        }

        if self.shell.switcher.open {
            return Some(match key {
                Keysym::Tab | Keysym::ISO_Left_Tab => KeyAction::SwitcherNext(mods.shift),
                Keysym::Escape => KeyAction::SwitcherCancel,
                _ => KeyAction::Swallow,
            });
        }
        if self.shell.desktop.renaming.is_some() {
            if mods.ctrl || mods.logo || mods.alt {
                return Some(KeyAction::Swallow);
            }
            return Some(match sym {
                Keysym::Escape => KeyAction::Escape,
                Keysym::Return | Keysym::KP_Enter => KeyAction::Enter,
                Keysym::BackSpace => KeyAction::Backspace,
                s => match s.key_char() {
                    Some(c) if !c.is_control() => KeyAction::Text(c.to_string()),
                    _ => KeyAction::Swallow,
                },
            });
        }
        // ícone da mesa selecionado e nenhum app em foco: Enter renomeia (como no Finder)
        if self.shell.desktop.selected.is_some() && self.focused_window().is_none() && self.shell.menu.is_none() && matches!(sym, Keysym::Return | Keysym::KP_Enter) && !(mods.ctrl || mods.logo || mods.alt) {
            return Some(KeyAction::Enter);
        }
        if self.shell.installer.open && !self.lock.active && !self.shell.settings.open {
            if mods.ctrl || mods.logo || mods.alt {
                return Some(KeyAction::Swallow);
            }
            return Some(match sym {
                Keysym::Return | Keysym::KP_Enter => KeyAction::InstallerEnter,
                Keysym::Tab | Keysym::ISO_Left_Tab => KeyAction::InstallerTab,
                Keysym::BackSpace => KeyAction::InstallerBackspace,
                s => match s.key_char() {
                    Some(c) if !c.is_control() => KeyAction::InstallerText(c.to_string()),
                    _ => KeyAction::Swallow,
                },
            });
        }
        if self.shell.welcome.open && !self.lock.active {
            return Some(match sym {
                Keysym::Return | Keysym::KP_Enter => KeyAction::WelcomeNext,
                _ => KeyAction::Swallow,
            });
        }
        if self.shell.about_app.open {
            return Some(match sym {
                Keysym::Escape | Keysym::Return | Keysym::KP_Enter => KeyAction::Escape,
                _ => KeyAction::Swallow,
            });
        }
        if self.shell.force_quit.open {
            return Some(match sym {
                Keysym::Escape => KeyAction::Escape,
                Keysym::Return | Keysym::KP_Enter => KeyAction::Enter,
                Keysym::Up => KeyAction::Move(-1),
                Keysym::Down => KeyAction::Move(1),
                _ => KeyAction::Swallow,
            });
        }
        // painéis com campo de texto capturam o teclado
        let text_ui = self.shell.launcher.open || self.shell.launchpad.open || self.shell.emoji.open || self.shell.settings.open;
        if text_ui || self.shell.mission.open || self.shell.cc.open || self.shell.nc.open || self.shell.activity.open || self.shell.settings.open || self.shell.menu.is_some() {
            if m && key == Keysym::space {
                return Some(KeyAction::Spotlight);
            }
            if m && matches!(key, Keysym::a | Keysym::A) {
                return Some(KeyAction::Launchpad);
            }
            if m && key == Keysym::Up {
                return Some(KeyAction::MissionControl);
            }
            if !text_ui {
                return (sym == Keysym::Escape).then_some(KeyAction::Escape);
            }
            if mods.ctrl || mods.logo || mods.alt {
                return Some(KeyAction::Swallow);
            }
            return Some(match sym {
                Keysym::Escape => KeyAction::Escape,
                Keysym::Return | Keysym::KP_Enter => KeyAction::Enter,
                Keysym::BackSpace => KeyAction::Backspace,
                Keysym::Up => KeyAction::Move(-1),
                Keysym::Down => KeyAction::Move(1),
                Keysym::Tab | Keysym::ISO_Left_Tab => KeyAction::Tab(mods.shift),
                s => match s.key_char() {
                    Some(c) if !c.is_control() => KeyAction::Text(c.to_string()),
                    _ => KeyAction::Swallow,
                },
            });
        }

        // Sessão real com ⌘ como Ctrl: tabela do macOS (o resto do ⌘ vai para o app)
        if self.cmd_as_ctrl {
            return self.mac_shortcuts(mods, key);
        }
        if m && mods.ctrl && key == Keysym::space {
            return Some(KeyAction::Emoji);
        }
        if m && mods.ctrl {
            match key {
                Keysym::_8 => return Some(KeyAction::Zoom8),
                Keysym::equal | Keysym::plus => return Some(KeyAction::ZoomStep(0.5)),
                Keysym::minus => return Some(KeyAction::ZoomStep(-0.5)),
                _ => {}
            }
        }
        if !m {
            return None;
        }
        // ⌘⌥ + setas / 1–4 / ⌫: encaixe (o Alt já é o ⌘ quando aninhado: aí use ⌘⌃)
        let tile_mod = if self.mod_alt { mods.ctrl } else { mods.alt };
        if tile_mod && !mods.shift {
            use crate::tiling::Tile;
            let t = match key {
                Keysym::Left => Some(Tile::Left),
                Keysym::Right => Some(Tile::Right),
                Keysym::Up => Some(Tile::Top),
                Keysym::Down => Some(Tile::Bottom),
                Keysym::_1 => Some(Tile::TopLeft),
                Keysym::_2 => Some(Tile::TopRight),
                Keysym::_3 => Some(Tile::BottomLeft),
                Keysym::_4 => Some(Tile::BottomRight),
                Keysym::Return => Some(Tile::Fill),
                _ => None,
            };
            if let Some(t) = t {
                return Some(KeyAction::Tile(t));
            }
            if key == Keysym::BackSpace {
                return Some(KeyAction::Untile);
            }
        }
        if mods.shift && matches!(key, Keysym::_3 | Keysym::numbersign) {
            return Some(KeyAction::Screenshot);
        }
        if mods.shift && matches!(key, Keysym::_4 | Keysym::dollar) {
            return Some(KeyAction::CaptureArea);
        }
        if mods.shift && matches!(key, Keysym::_5 | Keysym::percent) {
            return Some(KeyAction::CaptureBar);
        }
        if let Some(n) = digit(key) {
            return Some(if mods.ctrl { KeyAction::MoveToWorkspace(n) } else { KeyAction::Workspace(n) });
        }
        if matches!(key, Keysym::l | Keysym::L) || (mods.ctrl && matches!(key, Keysym::q | Keysym::Q)) {
            return Some(KeyAction::LockScreen);
        }
        Some(match key {
            Keysym::space => KeyAction::Spotlight,
            Keysym::a | Keysym::A => KeyAction::Launchpad,
            Keysym::Up => KeyAction::MissionControl,
            Keysym::Tab | Keysym::ISO_Left_Tab => KeyAction::SwitcherNext(mods.shift),
            Keysym::comma => KeyAction::Settings,
            Keysym::Return => KeyAction::Terminal,
            Keysym::e | Keysym::E if mods.shift => KeyAction::Logout,
            Keysym::e | Keysym::E => KeyAction::Files,
            Keysym::q | Keysym::Q | Keysym::w | Keysym::W => KeyAction::CloseWindow,
            Keysym::m | Keysym::M => KeyAction::Minimize,
            Keysym::h | Keysym::H => KeyAction::HideApp,
            Keysym::f | Keysym::F => KeyAction::Fullscreen,
            Keysym::z | Keysym::Z if mods.ctrl => KeyAction::Zoom,
            Keysym::grave => KeyAction::CycleAppWindows,
            _ => return None,
        })
    }

    /// Atalhos do macOS (com ⌘ sendo Ctrl nos apps).
    fn mac_shortcuts(&self, mods: &ModifiersState, key: Keysym) -> Option<KeyAction> {
        let cmd = mods.logo;
        if cmd && mods.alt {
            match key {
                Keysym::_8 => return Some(KeyAction::Zoom8),
                Keysym::equal | Keysym::plus => return Some(KeyAction::ZoomStep(0.5)),
                Keysym::minus => return Some(KeyAction::ZoomStep(-0.5)),
                _ => {}
            }
        }
        if matches!(key, Keysym::F4 | Keysym::XF86_LaunchA) && !cmd {
            return Some(KeyAction::Launchpad);
        }
        // Control sozinho: Mission Control e mesas
        if mods.ctrl && !cmd && !mods.alt {
            if let Some(n) = digit(key) {
                return Some(if mods.shift { KeyAction::MoveToWorkspace(n) } else { KeyAction::Workspace(n) });
            }
            return match key {
                Keysym::Up => Some(KeyAction::MissionControl),
                Keysym::Down => Some(KeyAction::AppExpose),
                Keysym::Left => Some(KeyAction::WorkspaceStep(-1)),
                Keysym::Right => Some(KeyAction::WorkspaceStep(1)),
                _ => None,
            };
        }
        if !cmd {
            return None;
        }
        // ⌘⌥ + setas: encaixe
        if mods.alt && !mods.shift {
            use crate::tiling::Tile;
            let t = match key {
                Keysym::Left => Some(Tile::Left),
                Keysym::Right => Some(Tile::Right),
                Keysym::Up => Some(Tile::Top),
                Keysym::Down => Some(Tile::Bottom),
                Keysym::Return => Some(Tile::Fill),
                _ => None,
            };
            if let Some(t) = t {
                return Some(KeyAction::Tile(t));
            }
            if key == Keysym::BackSpace {
                return Some(KeyAction::Untile);
            }
        }
        if mods.shift {
            return match key {
                Keysym::_3 | Keysym::numbersign => Some(KeyAction::Screenshot),
                Keysym::_4 | Keysym::dollar => Some(KeyAction::CaptureArea),
                Keysym::_5 | Keysym::percent => Some(KeyAction::CaptureBar),
                Keysym::q | Keysym::Q => Some(KeyAction::Logout),
                Keysym::Tab | Keysym::ISO_Left_Tab => Some(KeyAction::SwitcherNext(true)),
                _ => None,
            };
        }
        if mods.ctrl {
            return match key {
                Keysym::f | Keysym::F => Some(KeyAction::Fullscreen),
                Keysym::q | Keysym::Q => Some(KeyAction::LockScreen),
                Keysym::space => Some(KeyAction::Emoji),
                _ => None,
            };
        }
        match key {
            Keysym::space => Some(KeyAction::Spotlight),
            Keysym::Tab => Some(KeyAction::SwitcherNext(false)),
            Keysym::grave => Some(KeyAction::CycleAppWindows),
            Keysym::q | Keysym::Q => Some(KeyAction::QuitApp),
            Keysym::h | Keysym::H => Some(KeyAction::HideApp),
            Keysym::m | Keysym::M => Some(KeyAction::Minimize),
            _ => None,
        }
    }

    fn run_key_action(&mut self, a: KeyAction) {
        let apps = std::mem::take(&mut self.shell.apps);
        match a {
            KeyAction::AuthSubmit => self.auth_reply(true),
            KeyAction::AuthCancel => self.auth_reply(false),
            KeyAction::AuthBackspace => {
                self.auth_password.pop();
            }
            KeyAction::AuthText(t) => self.auth_password.push_str(&t),
            KeyAction::Zoom8 => self.zoom = if self.zoom > 1.0 { 1.0 } else { 2.0 },
            KeyAction::ZoomStep(d) => self.zoom = (self.zoom + d).clamp(1.0, 8.0),
            KeyAction::AppExpose => {
                if let Some(w) = self.focused_window() {
                    self.expose_app = crate::apps::for_app_id(&self.shell.apps, &Self::app_id(&w)).map(|a| a.id.clone());
                    self.open_mission();
                }
            }
            KeyAction::WorkspaceStep(d) => {
                let n = self.workspace as i32 + d;
                if (0..9).contains(&n) {
                    self.switch_workspace(n as usize);
                }
            }
            KeyAction::QuitApp => {
                if let Some(w) = self.focused_window() {
                    let id = Self::app_id(&w);
                    for x in self.all_windows() {
                        if Self::app_id(&x) == id {
                            self.close_window(&x);
                        }
                    }
                }
            }
            KeyAction::Emoji => {
                self.shell.close_popups();
                self.shell.emoji.toggle();
            }
            KeyAction::CaptureArea => {
                self.shell.close_popups();
                self.shell.capture.start_area();
                self.cursor_status = smithay::input::pointer::CursorImageStatus::Named(smithay::input::pointer::CursorIcon::Crosshair);
            }
            KeyAction::CaptureBar => {
                if self.recorder.is_some() {
                    self.stop_recording();
                } else {
                    self.shell.close_popups();
                    self.shell.capture.start_bar();
                }
            }
            KeyAction::CaptureClose => {
                self.shell.capture.close();
                self.cursor_status = smithay::input::pointer::CursorImageStatus::default_named();
            }
            KeyAction::CaptureGo => self.capture_go(None),
            KeyAction::CaptureMode(d) => {
                use crate::shell::capture_ui::CaptureMode::*;
                let all = [Screen, Window, Area, RecordScreen, RecordArea];
                let i = all.iter().position(|m| *m == self.shell.capture.mode).unwrap_or(2) as i32;
                self.shell.capture.mode = all[((i + d).rem_euclid(5)) as usize];
            }
            KeyAction::LockScreen => self.lock_screen(),
            KeyAction::LockText(t) => {
                if self.lock.state != crate::lock::LockState::Checking {
                    self.lock.state = crate::lock::LockState::Typing;
                    self.lock.password.push_str(&t);
                }
            }
            KeyAction::LockBackspace => {
                self.lock.password.pop();
            }
            KeyAction::LockClear => self.lock.password.clear(),
            KeyAction::LockSubmit => self.lock.submit(),
            KeyAction::LockUser(d) => self.lock.step_user(d),
            KeyAction::InstallerText(t) => {
                self.shell.installer.type_text(&t);
                self.dirty = true;
            }
            KeyAction::InstallerBackspace => {
                self.shell.installer.backspace();
                self.dirty = true;
            }
            KeyAction::InstallerTab => {
                self.shell.installer.next_field();
                self.dirty = true;
            }
            KeyAction::InstallerEnter => {
                // na conta, Enter passa para o próximo campo; no último, continua
                if self.shell.installer.step == 3 && self.shell.installer.focus.is_some_and(|f| f < 4) {
                    self.shell.installer.next_field();
                } else if self.shell.installer.blocker().is_none() && self.shell.installer.step <= 4 {
                    self.installer_action(crate::shell::installer::InstHit::Next);
                }
                self.dirty = true;
            }
            KeyAction::WelcomeNext => self.welcome_action(crate::shell::welcome::WelcomeHit::Next),
            KeyAction::Volume(d) => {
                let v = self.services.volume_step(d);
                self.shell.notch.show_hud(crate::shell::notch::HudKind::Volume, v);
            }
            KeyAction::Mute => {
                self.services.toggle_mute();
                let muted = self.services.get().muted;
                let v = if muted { 0.0 } else { self.services.get().volume.unwrap_or(0.0) };
                self.shell.notch.show_hud(crate::shell::notch::HudKind::Volume, v);
            }
            KeyAction::Brightness(d) => {
                let v = self.services.brightness_step(d);
                self.shell.notch.show_hud(crate::shell::notch::HudKind::Brightness, v);
            }
            KeyAction::Media(a) => self.services.media(a),
            KeyAction::ForceQuit => self.open_force_quit(),
            KeyAction::Tile(t) => {
                if let Some(w) = self.focused_window() {
                    self.tile(&w, t);
                }
            }
            KeyAction::Untile => {
                if let Some(w) = self.focused_window() {
                    self.untile(&w);
                }
            }
            KeyAction::Swallow => {}
            KeyAction::Spotlight => {
                let was = self.shell.launcher.open;
                self.shell.close_popups();
                if !was {
                    self.shell.launcher.toggle(&apps);
                }
            }
            KeyAction::Launchpad => self.toggle_launchpad(),
            KeyAction::MissionControl => {
                if self.shell.mission.open {
                    self.shell.mission.toggle();
                } else {
                    self.open_mission();
                }
            }
            KeyAction::SwitcherNext(back) => {
                if !self.shell.switcher.open {
                    self.open_switcher();
                } else {
                    let n = self.shell.switcher.items.len().max(1);
                    let s = &mut self.shell.switcher.selected;
                    *s = if back { (*s + n - 1) % n } else { (*s + 1) % n };
                }
            }
            KeyAction::SwitcherCommit => {
                let sel = self.shell.switcher.selected;
                self.shell.switcher.open = false;
                if let Some(w) = self.switch_windows.get(sel).cloned() {
                    self.activate_window(&w);
                }
            }
            KeyAction::SwitcherCancel => self.shell.switcher.open = false,
            KeyAction::Settings => self.open_settings(),
            KeyAction::Terminal => {
                if let Some(cmd) = crate::apps::first_in_path(&["foot", "kitty", "alacritty", "wezterm", "ptyxis", "xterm"]) {
                    crate::apps::spawn(&cmd);
                }
            }
            KeyAction::Files => self.open_path(&std::env::var("HOME").unwrap_or_else(|_| "/".into())),
            KeyAction::CloseWindow => {
                if let Some(w) = self.focused_window() {
                    self.close_window(&w);
                }
            }
            KeyAction::Minimize => {
                if let Some(w) = self.focused_window() {
                    self.minimize(&w);
                }
            }
            KeyAction::HideApp => self.hide_focused_app(),
            KeyAction::Fullscreen => {
                if let Some(w) = self.focused_window() {
                    if let Some(x) = w.x11_surface().cloned() {
                        let on = !meta(&w).lock().unwrap().fullscreen;
                        self.x11_fullscreen(&x, on);
                    } else if let Some(t) = w.toplevel().cloned() {
                        let on = !meta(&w).lock().unwrap().fullscreen;
                        self.set_fullscreen(&t, on);
                    }
                }
            }
            KeyAction::Zoom => {
                if let Some(w) = self.focused_window() {
                    self.toggle_maximize(&w);
                }
            }
            KeyAction::CycleAppWindows => self.cycle_windows(false),
            KeyAction::Workspace(n) => self.switch_workspace(n),
            KeyAction::MoveToWorkspace(n) => {
                if let Some(w) = self.focused_window() {
                    self.move_to_workspace(&w, n);
                }
            }
            KeyAction::Screenshot => self.request_screenshot(),
            KeyAction::Logout => self.loop_signal.stop(),
            KeyAction::Escape if self.shell.desktop.renaming.is_some() => self.shell.desktop.renaming = None,
            KeyAction::Escape => self.shell.close_popups(),
            KeyAction::Text(s) if self.shell.desktop.renaming.is_some() => {
                if let Some((_, t)) = &mut self.shell.desktop.renaming {
                    t.push_str(&s);
                }
            }
            KeyAction::Text(s) => {
                if self.shell.settings.open {
                    if let Some(id) = self.setctl.focus.clone() {
                        self.setctl.fields.entry(id.clone()).or_default().push_str(&s);
                        if id == "tz_search" {
                            self.refresh_settings_page();
                        }
                    } else {
                        self.shell.settings.query.push_str(&s);
                        if self.shell.settings.store {
                            if self.shell.settings.query.chars().count() >= 2 {
                                self.shell.settings.set_page("s:busca");
                                self.refresh_settings_page();
                            }
                            return;
                        }
                        // primeira página que bate com a busca
                        if let Some(p) = self.shell.settings.visible_pages().first().copied() {
                            if p != self.shell.settings.page {
                                self.settings_act(crate::shell::settings_model::Act::Page(p.to_string()));
                            }
                        }
                    }
                } else if self.shell.emoji.open {
                    self.shell.emoji.query.push_str(&s);
                } else if self.shell.launcher.open {
                    self.shell.launcher.push_str(&s, &apps);
                } else if self.shell.launchpad.open {
                    self.shell.launchpad.query.push_str(&s);
                    self.shell.launchpad.scroll = 0.0;
                }
            }
            KeyAction::Backspace if self.shell.desktop.renaming.is_some() => {
                if let Some((_, t)) = &mut self.shell.desktop.renaming {
                    t.pop();
                }
            }
            KeyAction::Backspace => {
                if self.shell.settings.open {
                    if let Some(id) = self.setctl.focus.clone() {
                        self.setctl.fields.entry(id).or_default().pop();
                    } else {
                        self.shell.settings.query.pop();
                    }
                } else if self.shell.emoji.open {
                    self.shell.emoji.query.pop();
                } else if self.shell.launcher.open {
                    self.shell.launcher.backspace(&apps);
                } else if self.shell.launchpad.open {
                    self.shell.launchpad.query.pop();
                }
            }
            KeyAction::Move(d) if self.shell.force_quit.open => self.shell.force_quit.step(d),
            KeyAction::Move(d) => self.shell.launcher.move_selection(d),
            KeyAction::Tab(back) => {
                if self.shell.launchpad.open {
                    let n = crate::shell::launchpad::CATEGORIES.len();
                    let c = &mut self.shell.launchpad.category;
                    *c = if back { (*c + n - 1) % n } else { (*c + 1) % n };
                } else {
                    self.shell.launcher.move_selection(if back { -1 } else { 1 });
                }
            }
            KeyAction::Enter => {
                if self.shell.desktop.renaming.is_some() {
                    self.shell.desktop.commit_rename();
                    self.dirty = true;
                    return;
                }
                if let (Some(i), None) = (self.shell.desktop.selected, self.focused_window()) {
                    if !self.shell.launcher.open && !self.shell.settings.open && !self.shell.emoji.open {
                        self.start_desk_rename(i);
                        return;
                    }
                }
                if self.shell.force_quit.open {
                    self.force_quit_selected();
                    return;
                }
                if self.shell.settings.open {
                    self.settings_submit_focused();
                    return;
                }
                if self.shell.emoji.open {
                    let e = self.shell.emoji.emoji(0);
                    self.shell.emoji.open = false;
                    self.shell.apps = apps;
                    if let Some(e) = e {
                        let term = self.focused_is_terminal();
                        self.insert_text(e, term);
                    }
                    return;
                }
                if self.shell.launcher.open {
                    self.shell.apps = apps;
                    self.launcher_activate(None);
                    return;
                } else if self.shell.launchpad.open {
                    if let Some(i) = self.shell.launchpad.first() {
                        crate::apps::spawn(&apps[i].exec);
                    }
                    self.shell.launchpad.close();
                }
            }
        }
        self.shell.apps = apps;
    }

    fn launcher_activate(&mut self, idx: Option<usize>) {
        let action = self.shell.launcher.activate(idx);
        self.shell.launcher.close();
        match action {
            LauncherAction::Run(cmd) => crate::apps::spawn(&cmd),
            LauncherAction::Copy(v) => crate::apps::spawn(&format!("wl-copy {}", shell_quote(&v))),
            LauncherAction::Open(p) => self.open_path(&p.to_string_lossy()),
            LauncherAction::Shell(a) => match a {
                ShellAction::Launchpad => self.toggle_launchpad(),
                ShellAction::MissionControl => self.open_mission(),
                ShellAction::ControlCenter => self.shell.cc.toggle(),
                ShellAction::ToggleAppearance => self.toggle_appearance(),
                ShellAction::Screenshot => self.request_screenshot(),
                ShellAction::Settings => self.open_settings(),
                ShellAction::Logout => self.loop_signal.stop(),
                ShellAction::Lock => self.lock_screen(),
                ShellAction::Activity => self.shell.activity.toggle(),
                ShellAction::Console => self.console_mode(),
                ShellAction::Stage => self.stage_manager = !self.stage_manager,
            },
            LauncherAction::Nothing => {}
        }
    }

    // ---------------------------------------------------------------- ponteiro

    fn on_motion(&mut self, pos: Point<f64, Logical>, time: u32) {
        let b = self.desktop_bounds();
        let pos = Point::<f64, Logical>::from((
            pos.x.clamp(b.loc.x as f64, (b.loc.x + b.size.w - 1) as f64),
            pos.y.clamp(b.loc.y as f64, (b.loc.y + b.size.h - 1) as f64),
        ));

        self.last_pointer = pos;
        // item da Bandeja ou da mesa: arrastar 5 px vira arrasto de arquivo
        if let Some((from, start, serial, button)) = self.press_drag {
            if (pos.x - start.x).hypot(pos.y - start.y) > 5.0 {
                self.press_drag = None;
                self.start_drag_out(from, serial, button, start);
            }
        }

        // arrasto de slider da Central de Controle
        if let Some(s) = self.shell.cc.dragging {
            let v = self.shell.cc.slider_value(s, pos.x as f32);
            match s {
                Slider::Brightness => self.services.set_brightness(v),
                Slider::Volume => self.services.set_volume(v),
            }
        }

        self.hot_corner(pos);
        if self.shell.capture.active {
            self.shell.capture.cursor = (pos.x, pos.y);
            if self.shell.capture.mode == crate::shell::capture_ui::CaptureMode::Window {
                self.shell.capture.hover_rect = self.space.elements().rev().filter(|w| Self::is_normal(w)).find_map(|w| {
                    let l = self.space.element_location(w)?;
                    let g = w.geometry().size;
                    let tb = if self.is_ssd(w) { metrics::TITLEBAR_H } else { 0 };
                    let r = (l.x as f32, (l.y - tb) as f32, g.w as f32, (g.h + tb) as f32);
                    ((pos.x as f32) >= r.0 && (pos.x as f32) < r.0 + r.2 && (pos.y as f32) >= r.1 && (pos.y as f32) < r.1 + r.3).then_some(r)
                });
            }
        }
        let hit = self.hit_test(pos);
        self.shell.dock.hover = if let Hit::Dock(i) = hit { i } else { None };
        self.shell.notch.set_hover(matches!(hit, Hit::Notch));
        if let Some(m) = &mut self.shell.menu {
            m.hover = if let Hit::Menu = hit { m.item_at(pos.x, pos.y) } else { None };
        }
        // com um menu da barra aberto, passar sobre outro título troca de menu
        if let Hit::Bar(Some(item)) = hit {
            let origin = self.shell.menu.as_ref().and_then(|m| m.origin);
            if origin.is_some() && item.is_menu() && origin != Some(item) {
                self.open_bar_menu(item);
            }
        }
        if let Hit::Launcher = hit {
            if let Some(r) = self.shell.launcher.row_at(pos.x, pos.y) {
                self.shell.launcher.select_row(r);
            }
        }
        if self.shell.settings.open {
            self.shell.settings.hover = matches!(hit, Hit::Settings).then_some((pos.x as f32, pos.y as f32));
            if let Some((t, root, x0, w)) = self.setctl.drag.clone() {
                self.settings_slider(&t, root, (pos.x as f32 - x0) / w);
            }
            if self.setctl.display_drag.is_some() {
                self.display_drag_motion(pos.x, pos.y);
            }
        }
        if let Hit::Emoji = hit {
            self.shell.emoji.hover = self.shell.emoji.at(pos.x, pos.y);
        }
        if let Hit::Launchpad = hit {
            self.shell.launchpad.hover = self.shell.launchpad.app_at(pos.x, pos.y);
        }
        if let Hit::Mission = hit {
            self.shell.mission.hover = self.shell.mission.window_at(pos.x, pos.y);
        }
        self.shell.hover_title = match &hit {
            Hit::Titlebar { window, button } => Self::win_id(window).map(|id| (id, *button)),
            _ => None,
        };

        let under = if let Hit::Surface(_) = hit { self.surface_under(pos) } else { None };
        let pointer = self.seat.get_pointer().unwrap();
        pointer.motion(self, under.clone(), &MotionEvent { location: pos, serial: SERIAL_COUNTER.next_serial(), time });
        pointer.frame(self);
        // restrição pendente da superfície sob o mouse: ativa ao entrar
        if let Some((surface, _)) = under {
            smithay::wayland::pointer_constraints::with_pointer_constraint(&surface, &pointer, |c| {
                if let Some(c) = c {
                    if !c.is_active() {
                        c.activate();
                    }
                }
            });
        }
    }

    fn on_button(&mut self, button: u32, state: ButtonState, time: u32) {
        let pointer = self.seat.get_pointer().unwrap();
        let pos = pointer.current_location();
        let serial = SERIAL_COUNTER.next_serial();

        if state == ButtonState::Pressed {
            if !pointer.is_grabbed() && self.handle_ui_press(pos, button) {
                self.swallow_release = true;
                return;
            }
        } else {
            self.shell.cc.dragging = None;
            self.setctl.drag = None;
            self.press_drag = None;
            if self.setctl.display_drag.is_some() {
                self.display_drag_end();
            }
            // fim do arrasto da área (⇧⌘4)
            if self.shell.capture.active && self.shell.capture.drag_start.is_some() {
                let sel = self.shell.capture.selection();
                self.shell.capture.drag_start = None;
                let rec = self.shell.capture.mode == crate::shell::capture_ui::CaptureMode::RecordArea;
                match sel {
                    Some(r) if r.2 >= 16.0 && r.3 >= 16.0 && rec => self.start_recording(Some(r)),
                    Some(r) if r.2 >= 4.0 && r.3 >= 4.0 && !rec => self.capture_region(Some(r)),
                    _ => {}
                }
            }
            if let Some(i) = self.pending_shelf_open.take() {
                if self.drag_out.is_none() {
                    if let Some(f) = self.shelf.get(i) {
                        crate::apps::spawn(&format!("xdg-open {}", crate::shell::settings_model::q(f)));
                    }
                }
            }
            if self.swallow_release {
                self.swallow_release = false;
                return;
            }
        }
        pointer.button(self, &ButtonEvent { button, state, serial, time });
        pointer.frame(self);
    }

    /// Clique na UI do shell. `true` = consumido (não vai para o app).
    fn handle_ui_press(&mut self, pos: Point<f64, Logical>, button: u32) -> bool {
        let hit = self.hit_test(pos);
        let left = button == BTN_LEFT;

        match hit {
            Hit::Capture => {
                use crate::shell::capture_ui::{BarHit, CaptureMode};
                if !left {
                    return true;
                }
                match self.shell.capture.bar_hit(pos.x, pos.y) {
                    Some(BarHit::Close) => {
                        self.shell.capture.close();
                        self.cursor_status = smithay::input::pointer::CursorImageStatus::default_named();
                    }
                    Some(BarHit::Mode(m)) => self.shell.capture.mode = m,
                    Some(BarHit::Timer) => {
                        let t = &mut self.shell.capture.timer;
                        *t = match *t { 0 => 5, 5 => 10, _ => 0 };
                    }
                    Some(BarHit::Capture) => self.capture_go(None),
                    None => match self.shell.capture.mode {
                        CaptureMode::Area | CaptureMode::RecordArea => self.shell.capture.drag_start = Some((pos.x, pos.y)),
                        CaptureMode::RecordScreen => self.start_recording(None),
                        CaptureMode::Window => {
                            if let Some(r) = self.shell.capture.hover_rect {
                                self.capture_region(Some(r));
                            }
                        }
                        CaptureMode::Screen => self.capture_region(None),
                    },
                }
                return true;
            }
            Hit::Lock => {
                use crate::lock::LockHit;
                if left {
                    match self.lock.hit(pos.x, pos.y) {
                        Some(LockHit::Submit) => self.lock.submit(),
                        Some(LockHit::PrevUser) => self.lock.step_user(-1),
                        Some(LockHit::NextUser) => self.lock.step_user(1),
                        Some(LockHit::Sleep) => crate::apps::spawn("systemctl suspend"),
                        Some(LockHit::Restart) => crate::apps::spawn("systemctl reboot"),
                        Some(LockHit::Shutdown) => crate::apps::spawn("systemctl poweroff"),
                        None => {}
                    }
                }
                return true;
            }
            Hit::Switcher => return true,
            Hit::Mission => {
                if left {
                    if let Some(i) = self.shell.mission.window_at(pos.x, pos.y) {
                        let w = self.mc_windows.get(i).cloned();
                        self.shell.mission.toggle();
                        if let Some(w) = w {
                            self.activate_window(&w);
                        }
                    } else if let Some(space) = self.shell.mission.space_at(pos.x, pos.y) {
                        let target = space.unwrap_or_else(|| self.ws_store.iter().position(|w| w.is_empty()).unwrap_or(0));
                        self.switch_workspace(target);
                        self.open_mission(); // recalcula com as janelas da mesa nova
                    } else {
                        self.shell.mission.toggle();
                    }
                }
                return true;
            }
            Hit::Launchpad => {
                if left {
                    if let Some(c) = self.shell.launchpad.category_at(pos.x, pos.y, self.screen.w as f32) {
                        self.shell.launchpad.category = c;
                        self.shell.launchpad.scroll = 0.0;
                    } else if let Some(i) = self.shell.launchpad.app_at(pos.x, pos.y) {
                        let exec = self.shell.apps[i].exec.clone();
                        crate::apps::spawn(&exec);
                        self.shell.launchpad.close();
                    } else if pos.y > 140.0 {
                        self.shell.launchpad.close();
                    }
                }
                return true;
            }
            _ => {}
        }

        // clique fora fecha Busca, menu e Central de Controle
        if self.shell.launcher.open && !matches!(hit, Hit::Launcher) {
            self.shell.launcher.close();
            return true;
        }
        if self.shell.menu.is_some() && !matches!(hit, Hit::Menu) {
            let origin = self.shell.menu.as_ref().and_then(|m| m.origin);
            self.shell.menu = None;
            self.shell.dock.menu_open = false;
            // clicar no mesmo título fecha; em outro título abre o dele
            if let Hit::Bar(Some(item)) = hit {
                if item.is_menu() && origin != Some(item) {
                    self.open_bar_menu(item);
                }
            }
            return true;
        }
        if self.shell.settings.open && !matches!(hit, Hit::Settings) && !matches!(hit, Hit::Surface(_)) {
            // clique fora dos Ajustes (na mesa) fecha; em janelas, só troca o foco
            self.shell.settings.open = false;
        }
        if self.shell.emoji.open && !matches!(hit, Hit::Emoji) {
            self.shell.emoji.open = false;
            return true;
        }
        if self.shell.activity.open && !matches!(hit, Hit::Activity) {
            self.shell.activity.open = false;
            if !matches!(hit, Hit::Surface(_)) {
                return true;
            }
        }
        if self.shell.nc.open && !matches!(hit, Hit::Nc) {
            self.shell.nc.open = false;
            if !matches!(hit, Hit::Surface(_)) {
                return true;
            }
        }
        if self.shell.cc.open && !matches!(hit, Hit::Cc) {
            self.shell.cc.toggle();
            if !matches!(hit, Hit::Surface(_)) {
                return true;
            }
        }

        match hit {
            Hit::Launcher => {
                if left {
                    if let Some(i) = self.shell.launcher.row_at(pos.x, pos.y) {
                        self.launcher_activate(Some(i));
                    }
                }
                true
            }
            Hit::Menu => {
                if left {
                    if let Some(i) = self.shell.menu.as_ref().and_then(|m| m.item_at(pos.x, pos.y)) {
                        self.menu_activate(i);
                    }
                }
                true
            }
            Hit::Cc => {
                if let (true, Some(h)) = (left, self.shell.cc.hit(pos.x, pos.y)) {
                    self.cc_action(h);
                }
                true
            }
            Hit::Settings => {
                if left {
                    if let Some(a) = self.shell.settings.hit(pos.x, pos.y) {
                        self.settings_action(a);
                    }
                }
                true
            }
            Hit::Emoji => {
                if left {
                    if let Some(e) = self.shell.emoji.at(pos.x, pos.y).and_then(|i| self.shell.emoji.emoji(i)) {
                        self.shell.emoji.open = false;
                        let term = self.focused_is_terminal();
                        self.insert_text(e, term);
                    }
                }
                true
            }
            Hit::Installer => {
                if left {
                    match self.shell.installer.hit(pos.x, pos.y) {
                        Some(h) => self.installer_action(h),
                        None => self.shell.installer.focus = None,
                    }
                }
                true
            }
            Hit::Welcome => {
                if left {
                    if let Some(h) = self.shell.welcome.hit(pos.x, pos.y) {
                        self.welcome_action(h);
                    }
                }
                true
            }
            Hit::AboutApp => {
                if left && self.shell.about_app.closes(pos.x, pos.y) {
                    self.shell.about_app.open = false;
                }
                true
            }
            Hit::ForceQuit => {
                use crate::shell::force_quit::FqHit;
                if left {
                    match self.shell.force_quit.hit(pos.x, pos.y) {
                        Some(FqHit::Row(i)) => self.shell.force_quit.selected = i,
                        Some(FqHit::Kill) => self.force_quit_selected(),
                        Some(FqHit::Close) => self.shell.force_quit.open = false,
                        None => {}
                    }
                }
                true
            }
            Hit::Activity => {
                use crate::shell::activity::ActHit;
                if left {
                    match self.shell.activity.hit(pos.x, pos.y) {
                        Some(ActHit::Row(i)) => self.shell.activity.selected = Some(i),
                        Some(ActHit::Close) => self.shell.activity.open = false,
                        Some(ActHit::Quit) | Some(ActHit::Kill) => {
                            let sig = if matches!(self.shell.activity.hit(pos.x, pos.y), Some(ActHit::Kill)) { "KILL" } else { "TERM" };
                            if let Some(pid) = self.shell.activity.selected_pid() {
                                if pid as u32 != std::process::id() {
                                    crate::apps::spawn(&format!("kill -{sig} {pid}"));
                                }
                            }
                            self.shell.activity.selected = None;
                        }
                        None => {}
                    }
                }
                true
            }
            Hit::Nc => {
                use crate::shell::notif_center::NcHit;
                if left {
                    match self.shell.nc.hit(pos.x, pos.y) {
                        Some(NcHit::Clear) => {
                            let ids: Vec<u32> = self.notifs.iter().map(|n| n.id).collect();
                            for id in ids {
                                self.dismiss_notif(id, 2);
                            }
                        }
                        Some(NcHit::Close(i)) => {
                            if let Some(id) = self.notifs.get(i).map(|n| n.id) {
                                self.dismiss_notif(id, 2);
                            }
                        }
                        Some(NcHit::Card(i)) => {
                            if let Some(n) = self.notifs.get(i).cloned() {
                                if n.actions.iter().any(|(k, _)| k == "default") {
                                    self.invoke_notif_action(n.id, "default");
                                }
                            }
                        }
                        None => {}
                    }
                }
                true
            }
            Hit::Notch => {
                use crate::shell::notch::NotchHit;
                if left {
                    match self.shell.notch.hit(pos.x, pos.y) {
                        Some(NotchHit::MediaPrev) => self.services.media("previous"),
                        Some(NotchHit::MediaPlay) => self.services.media("play-pause"),
                        Some(NotchHit::MediaNext) => self.services.media("next"),
                        Some(NotchHit::GameMode) => self.toggle_game_mode(),
                        Some(NotchHit::StopRecording) => self.stop_recording(),
                        Some(NotchHit::AuthOk) => self.auth_reply(true),
                        Some(NotchHit::AuthCancel) => self.auth_reply(false),
                        Some(NotchHit::NotifClose) => {
                            if let Some(id) = self.banners.front().map(|n| n.id) {
                                self.banners.pop_front();
                                self.banner_since = None;
                                let _ = id;
                            }
                        }
                        Some(NotchHit::NotifAction(i)) => {
                            if let Some(n) = self.banners.front().cloned() {
                                if let Some((k, _)) = n.actions.get(i) {
                                    self.invoke_notif_action(n.id, k);
                                }
                            }
                        }
                        Some(NotchHit::NotifBody) => {
                            if let Some(n) = self.banners.front().cloned() {
                                if n.summary == "Atualizações disponíveis" {
                                    self.banners.pop_front();
                                    self.open_internal("settings:updates");
                                    return true;
                                }
                                if n.actions.iter().any(|(k, _)| k == "default") {
                                    self.invoke_notif_action(n.id, "default");
                                } else {
                                    self.banners.pop_front();
                                    self.banner_since = None;
                                }
                            }
                        }
                        Some(NotchHit::TabHome) => self.shell.notch.tab = crate::shell::notch::Tab::Home,
                        Some(NotchHit::TabTray) => self.shell.notch.tab = crate::shell::notch::Tab::Tray,
                        Some(NotchHit::ShelfItem(i)) => {
                            // clique abre; arrastar leva o arquivo para outro app
                            self.press_drag = Some((crate::state::DragFrom::Shelf(i), pos, SERIAL_COUNTER.next_serial(), button));
                            self.pending_shelf_open = Some(i);
                        }
                        Some(NotchHit::ShelfClear) => self.shelf.clear(),
                        Some(NotchHit::CaptureOpen) => self.capture_toast_action("open"),
                        Some(NotchHit::CaptureCopy) => self.capture_toast_action("copy"),
                        Some(NotchHit::CaptureAnnotate) => self.capture_toast_action("annotate"),
                        Some(NotchHit::CaptureSaveAs) => self.capture_toast_action("saveas"),
                        Some(NotchHit::CaptureClose) => self.capture_toast = None,
                        _ => self.shell.notch.open = true,
                    }
                } else if button == BTN_RIGHT {
                    match self.shell.notch.hit(pos.x, pos.y) {
                        // botão direito: tira o arquivo da Bandeja
                        Some(NotchHit::ShelfItem(i)) if i < self.shelf.len() => {
                            self.shelf.remove(i);
                        }
                        // botão direito na notificação: passa para a próxima
                        Some(NotchHit::NotifBody) | Some(NotchHit::NotifClose) | Some(NotchHit::Body) if self.banners.len() > 1 => {
                            if let Some(n) = self.banners.pop_front() {
                                self.banners.push_back(n);
                            }
                            self.banner_since = None;
                        }
                        _ => {}
                    }
                }
                true
            }
            Hit::Bar(Some(BarItem::Tray(i))) => {
                self.tray_click(i, button, pos);
                true
            }
            Hit::Bar(item) => {
                if let (true, Some(item)) = (left, item) {
                    match item {
                        BarItem::Logo | BarItem::App | BarItem::File | BarItem::Edit | BarItem::View | BarItem::Window | BarItem::Go | BarItem::Help => self.open_bar_menu(item),
                        BarItem::Workspace(i) => self.switch_workspace(i),
                        BarItem::Battery => self.open_battery_menu(),
                        BarItem::ControlCenter | BarItem::Sound | BarItem::Network | BarItem::Bluetooth => self.shell.cc.toggle(),
                        BarItem::Recording => self.stop_recording(),
                        BarItem::Tray(i) => self.tray_click(i, button, pos),
                        BarItem::Clock => {
                            let was = self.shell.nc.open;
                            self.shell.close_popups();
                            if !was {
                                self.shell.nc.toggle();
                            }
                        }
                    }
                }
                true
            }
            Hit::Dock(idx) => {
                if let Some(i) = idx {
                    if left {
                        self.dock_activate(i);
                    } else if button == BTN_RIGHT {
                        self.open_dock_menu(i);
                    }
                }
                true
            }
            Hit::Titlebar { window, button: btn } => {
                self.focus_window(&window);
                if !left {
                    return true;
                }
                match btn {
                    Some(0) => self.close_window(&window),
                    Some(1) => self.minimize(&window),
                    Some(2) => self.toggle_maximize(&window),
                    _ => {
                        let id = Self::win_id(&window).unwrap();
                        let dbl = self.last_title_click.as_ref().is_some_and(|(t, i)| *i == id && t.elapsed().as_millis() < 400);
                        self.last_title_click = Some((std::time::Instant::now(), id));
                        if dbl {
                            self.toggle_maximize(&window);
                        } else if !meta(&window).lock().unwrap().maximized {
                            self.start_move(&window, button, pos);
                        }
                    }
                }
                true
            }
            Hit::Edge { window, edges } => {
                self.focus_window(&window);
                if left {
                    self.start_resize(&window, edges, button, pos);
                }
                true
            }
            Hit::Surface(window) => {
                if self.focused_window().as_ref() != Some(&window) || self.top_window().as_ref() != Some(&window) {
                    self.focus_window(&window);
                }
                false
            }
            Hit::StageThumb(w) => {
                if left {
                    self.focus_window(&w);
                }
                true
            }
            Hit::Desktop => {
                self.unfocus_all();
                self.shell.desktop.commit_rename();
                self.shell.desktop.selected = None;
                if button == BTN_RIGHT {
                    self.open_desktop_menu(pos, None);
                }
                true
            }
            Hit::DeskIcon(i) => {
                self.unfocus_all();
                let now = std::time::Instant::now();
                let double = self.shell.desktop.last_click.is_some_and(|(j, t)| j == i && t.elapsed().as_millis() < 450);
                self.shell.desktop.selected = Some(i);
                if left && double {
                    self.on_menu_action(MenuAction::DeskOpen(i));
                    self.shell.desktop.last_click = None;
                } else if left {
                    self.shell.desktop.last_click = Some((i, now));
                    if self.shell.desktop.renaming.as_ref().is_some_and(|r| r.0 != i) {
                        self.shell.desktop.commit_rename();
                    }
                    self.press_drag = Some((crate::state::DragFrom::Desk(i), pos, SERIAL_COUNTER.next_serial(), button));
                } else if button == BTN_RIGHT {
                    self.open_desktop_menu(pos, Some(i));
                }
                self.dirty = true;
                true
            }
            Hit::Switcher | Hit::Mission | Hit::Launchpad | Hit::Lock | Hit::Capture => true,
        }
    }

    pub fn start_move_pub(&mut self, window: &Window, button: u32, pos: Point<f64, Logical>) {
        self.start_move(window, button, pos);
    }

    pub fn start_resize_pub(&mut self, window: &Window, edges: ResizeEdge, button: u32, pos: Point<f64, Logical>) {
        self.start_resize(window, edges, button, pos);
    }

    fn start_move(&mut self, window: &Window, button: u32, pos: Point<f64, Logical>) {
        let pointer = self.seat.get_pointer().unwrap();
        let Some(initial) = self.space.element_location(window) else { return };
        let start_data = PointerGrabStartData { focus: None, button, location: pos };
        let grab = MoveSurfaceGrab { start_data, window: window.clone(), initial_window_location: initial };
        pointer.set_grab(self, grab, SERIAL_COUNTER.next_serial(), Focus::Clear);
    }

    fn start_resize(&mut self, window: &Window, edges: ResizeEdge, button: u32, pos: Point<f64, Logical>) {
        let pointer = self.seat.get_pointer().unwrap();
        let Some(loc) = self.space.element_location(window) else { return };
        let start_data = PointerGrabStartData { focus: None, button, location: pos };
        if let Some(t) = window.toplevel() {
            t.with_pending_state(|s| {
                s.states.set(xdg_toplevel::State::Resizing);
            });
            t.send_pending_configure();
        }
        let grab = ResizeSurfaceGrab::start(start_data, window.clone(), edges, Rectangle::new(loc, window.geometry().size));
        pointer.set_grab(self, grab, SERIAL_COUNTER.next_serial(), Focus::Clear);
    }

    // ---------------------------------------------------------------- abrir painéis

    /// Enter/Capturar na barra: tela inteira, janela sob o mouse ou área selecionada.
    fn capture_go(&mut self, _r: Option<(f32, f32, f32, f32)>) {
        use crate::shell::capture_ui::CaptureMode;
        match self.shell.capture.mode {
            CaptureMode::RecordScreen => self.start_recording(None),
            CaptureMode::RecordArea => {
                if let Some(r) = self.shell.capture.selection() {
                    self.start_recording(Some(r));
                } else {
                    self.shell.capture.bar = false;
                }
            }
            CaptureMode::Screen => self.capture_region(None),
            CaptureMode::Window => {
                let r = self.shell.capture.hover_rect.or_else(|| {
                    let w = self.focused_window()?;
                    let l = self.space.element_location(&w)?;
                    let g = w.geometry().size;
                    let tb = if self.is_ssd(&w) { metrics::TITLEBAR_H } else { 0 };
                    Some((l.x as f32, (l.y - tb) as f32, g.w as f32, (g.h + tb) as f32))
                });
                self.capture_region(r);
            }
            CaptureMode::Area => {
                if let Some(r) = self.shell.capture.selection() {
                    self.capture_region(Some(r));
                } else {
                    // sem seleção: espera o arrasto (como o macOS)
                    self.shell.capture.bar = false;
                    self.cursor_status = smithay::input::pointer::CursorImageStatus::Named(smithay::input::pointer::CursorIcon::Crosshair);
                }
            }
        }
    }

    pub fn toggle_launchpad(&mut self) {
        let was = self.shell.launchpad.open;
        self.shell.close_popups();
        if !was {
            self.shell.launchpad.toggle();
        }
    }

    pub fn open_mission(&mut self) {
        self.shell.close_popups();
        let only = self.expose_app.take();
        let wins: Vec<Window> = self
            .space
            .elements()
            .filter(|w| Self::is_normal(w))
            .filter(|w| match (&only, crate::apps::for_app_id(&self.shell.apps, &Self::app_id(w))) {
                (Some(id), Some(app)) => &app.id == id,
                (Some(_), None) => false,
                (None, _) => true,
            })
            .cloned()
            .collect();
        let sizes: Vec<(f32, f32)> = wins.iter().map(|w| (w.geometry().size.w as f32, w.geometry().size.h as f32)).collect();
        let slots = mission::layout(&sizes, self.screen.w as f32, self.screen.h as f32);
        let mut list = Vec::new();
        for (w, slot) in wins.iter().zip(slots) {
            let l = self.space.element_location(w).unwrap_or_default();
            let g = w.geometry().size;
            list.push(McWindow { real: (l.x as f32, l.y as f32, g.w as f32, g.h as f32), slot, title: Self::title(w), icon: self.window_icon(w) });
        }
        self.mc_windows = wins;
        self.shell.mission.windows = list;
        if !self.shell.mission.open {
            self.shell.mission.toggle();
        }
    }

    pub fn open_switcher(&mut self) {
        let apps = self.apps_by_mru();
        if apps.is_empty() {
            return;
        }
        self.shell.close_popups();
        self.switch_windows = apps.iter().map(|(_, w)| w.clone()).collect();
        self.shell.switcher.items = apps
            .iter()
            .map(|(_, w)| {
                let app = crate::apps::for_app_id(&self.shell.apps, &Self::app_id(w));
                SwitchItem {
                    name: app.map(|a| a.name.clone()).unwrap_or_else(|| Self::title(w)),
                    icon: self.window_icon(w),
                    minimized: self.minimized.contains(w),
                }
            })
            .collect();
        self.shell.switcher.selected = if self.shell.switcher.items.len() > 1 { 1 } else { 0 };
        self.shell.switcher.open = true;
    }

    fn open_bar_menu(&mut self, item: BarItem) {
        let x = self.shell.menubar.item_x(item).map(|(a, _)| a).unwrap_or(10.0);
        let app = self.focused_app_name();
        let dark = self.shell.theme.appearance == Appearance::Dark;
        let nested = self.backend == BackendKind::Nested;
        let items = match item {
            BarItem::Logo => vec![
                MenuItem::new("Sobre este Obelisk", MenuAction::About),
                MenuItem::sep(),
                MenuItem::new("Instalar o Obelisk…", MenuAction::Installer).enabled(crate::shell::installer::is_live()),
                MenuItem::new("Ajustes do Sistema…", MenuAction::Settings).key("⌘,"),
                MenuItem::new(&if self.setctl.updates > 0 { format!("App Store…  ({} atualizações)", self.setctl.updates) } else { "App Store…".to_string() }, MenuAction::Store),
                MenuItem::new("Central de Controle", MenuAction::ControlCenter),
                MenuItem::new("Forçar Encerrar…", MenuAction::ForceQuitMenu).key("⌥⌘⎋"),
                MenuItem::new("Monitor de Atividade", MenuAction::ActivityMonitor),
                MenuItem::new("Modo Console (Steam)", MenuAction::ConsoleMode).enabled(crate::apps::first_in_path(&["gamescope"]).is_some() && crate::apps::first_in_path(&["steam"]).is_some()),
                MenuItem::sep(),
                MenuItem::new("Aparência Escura", MenuAction::ToggleAppearance).check(dark),
                MenuItem::new("Aparência Automática", MenuAction::AutoAppearance).check(self.auto_appearance),
                MenuItem::new("Night Shift", MenuAction::NightShift).check(self.night_shift).enabled(!nested),
                MenuItem::sep(),
                MenuItem::new("Repouso", MenuAction::Sleep).enabled(!nested),
                MenuItem::new("Reiniciar…", MenuAction::Restart).enabled(!nested),
                MenuItem::new("Desligar…", MenuAction::Shutdown).enabled(!nested),
                MenuItem::sep(),
                MenuItem::new("Bloquear Tela", MenuAction::Lock).key("⌃⌘Q"),
                MenuItem::new(if nested { "Fechar o Obelisk" } else { "Encerrar Sessão…" }, MenuAction::Logout).key("⇧⌘E"),
            ],
            BarItem::App => vec![
                MenuItem::new(&format!("Sobre o {app}"), MenuAction::AboutApp).enabled(self.focused_window().is_some()),
                MenuItem::sep(),
                MenuItem::new(&format!("Ocultar {app}"), MenuAction::HideApp).key("⌘H").enabled(self.focused_window().is_some()),
                MenuItem::sep(),
                MenuItem::new(&format!("Encerrar {app}"), MenuAction::QuitApp).key("⌘Q").enabled(self.focused_window().is_some()),
            ],
            BarItem::Window => {
                let mut v = vec![
                    MenuItem::new("Minimizar", MenuAction::Minimize).key("⌘M"),
                    MenuItem::new("Zoom", MenuAction::Zoom).key("⌃⌘Z"),
                    MenuItem::new("Tela Cheia", MenuAction::Fullscreen).key("⌘F"),
                    MenuItem::sep(),
                    MenuItem::new("Trazer Tudo para a Frente", MenuAction::BringAllToFront),
                    MenuItem::new("Stage Manager", MenuAction::StageManager).check(self.stage_manager),
                    MenuItem::new("Manter na Frente", MenuAction::OnTop).check(self.focused_window().is_some_and(|w| meta(&w).lock().unwrap().on_top)),
                    MenuItem::sep(),
                ];
                let focused = self.focused_window();
                for (i, w) in self.space.elements().enumerate() {
                    let t = Self::title(w);
                    v.push(MenuItem::new(if t.is_empty() { "Sem título" } else { &t }, MenuAction::FocusWindow(i)).check(focused.as_ref() == Some(w)));
                }
                v
            }
            BarItem::File => {
                let has = self.focused_window().is_some();
                vec![
                    MenuItem::new("Nova Janela", MenuAction::Keys(vec![CTRL, SHIFT], K_N)).key("⌘N").enabled(has),
                    MenuItem::new("Nova Aba", MenuAction::Keys(vec![CTRL, SHIFT], K_T)).key("⌘T").enabled(has),
                    MenuItem::new("Abrir…", MenuAction::Keys(vec![CTRL], K_O)).key("⌘O").enabled(has),
                    MenuItem::sep(),
                    MenuItem::new("Salvar", MenuAction::Keys(vec![CTRL], K_S)).key("⌘S").enabled(has),
                    MenuItem::new("Imprimir…", MenuAction::Keys(vec![CTRL], K_P)).key("⌘P").enabled(has),
                    MenuItem::sep(),
                    MenuItem::new("Fechar Janela", MenuAction::QuitApp).key("⌘W").enabled(has),
                ]
            }
            BarItem::Edit => {
                let has = self.focused_window().is_some();
                vec![
                    MenuItem::new("Desfazer", MenuAction::Keys(vec![CTRL], K_Z)).key("⌘Z").enabled(has),
                    MenuItem::new("Refazer", MenuAction::Keys(vec![CTRL, SHIFT], K_Z)).key("⇧⌘Z").enabled(has),
                    MenuItem::sep(),
                    MenuItem::new("Recortar", MenuAction::Clip(K_X)).key("⌘X").enabled(has),
                    MenuItem::new("Copiar", MenuAction::Clip(K_C)).key("⌘C").enabled(has),
                    MenuItem::new("Colar", MenuAction::Clip(K_V)).key("⌘V").enabled(has),
                    MenuItem::new("Selecionar Tudo", MenuAction::Keys(vec![CTRL], K_A)).key("⌘A").enabled(has),
                    MenuItem::sep(),
                    MenuItem::new("Buscar…", MenuAction::Keys(vec![CTRL], K_F)).key("⌘F").enabled(has),
                    MenuItem::sep(),
                    MenuItem::new("Emojis e Símbolos", MenuAction::Emoji).key("⌃⌘Espaço"),
                ]
            }
            BarItem::View => {
                let has = self.focused_window().is_some();
                vec![
                    MenuItem::new("Ampliar", MenuAction::Keys(vec![CTRL], K_PLUS)).key("⌘+").enabled(has),
                    MenuItem::new("Reduzir", MenuAction::Keys(vec![CTRL], K_MINUS)).key("⌘−").enabled(has),
                    MenuItem::new("Tamanho Real", MenuAction::Keys(vec![CTRL], K_0)).key("⌘0").enabled(has),
                    MenuItem::sep(),
                    MenuItem::new("Recarregar", MenuAction::Keys(vec![CTRL], K_R)).key("⌘R").enabled(has),
                    MenuItem::sep(),
                    MenuItem::new("Entrar em Tela Cheia", MenuAction::Fullscreen).key("⌃⌘F").enabled(has),
                ]
            }
            BarItem::Help => vec![
                MenuItem::new("Buscar no Obelisk", MenuAction::Spotlight).key("⌘Espaço"),
                MenuItem::sep(),
                MenuItem::new("Atalhos de Teclado", MenuAction::SettingsPage("keyboard".into())),
                MenuItem::new("Sobre este Obelisk", MenuAction::SettingsPage("about".into())),
            ],
            BarItem::Go => {
                let home = std::env::var("HOME").unwrap_or_default();
                let dir = |names: &[&str]| names.iter().map(|n| format!("{home}/{n}")).find(|p| std::path::Path::new(p).is_dir()).unwrap_or_else(|| home.clone());
                let mut v: Vec<MenuItem> = (0..4)
                    .map(|i| MenuItem::new(&format!("Mesa {}", i + 1), MenuAction::Workspace(i)).key(&format!("⌘{}", i + 1)).check(self.workspace == i))
                    .collect();
                v.push(MenuItem::sep());
                v.push(MenuItem::new("Pasta Pessoal", MenuAction::OpenPath(home.clone())));
                v.push(MenuItem::new("Documentos", MenuAction::OpenPath(dir(&["Documentos", "Documents"]))));
                v.push(MenuItem::new("Downloads", MenuAction::OpenPath(dir(&["Downloads"]))));
                v.push(MenuItem::new("Aplicativos", MenuAction::Launchpad).key("⌘A"));
                v.push(MenuItem::new("Mission Control", MenuAction::MissionControl).key("⌘↑"));
                v.push(MenuItem::sep());
                v.push(MenuItem::new("Lixo", MenuAction::OpenPath("trash:///".into())));
                v
            }
            _ => return,
        };
        self.shell.cc.open = false;
        let mut menu = Menu::new(items, (x as i32, 35), 230.0);
        menu.origin = Some(item);
        self.shell.menu = Some(menu);
    }

    /// Clique num ícone da bandeja: esquerdo ativa o app (ou abre o menu, se o item é só menu),
    /// direito abre o menu do app (dbusmenu), meio faz a ação secundária.
    fn tray_click(&mut self, i: usize, button: u32, pos: Point<f64, Logical>) {
        let Some(item) = self.tray_items.get(i).cloned() else { return };
        let Some(tray) = &self.tray else { return };
        let (x, y) = (pos.x as i32, metrics::MENUBAR_H);
        match button {
            BTN_LEFT if !item.item_is_menu => tray.activate(&item, "Activate", x, y),
            BTN_LEFT | BTN_RIGHT => {
                if item.menu.is_some() {
                    tray.open_menu(&item);
                } else {
                    tray.activate(&item, "ContextMenu", x, y);
                }
            }
            _ => tray.activate(&item, "SecondaryActivate", x, y),
        }
    }

    /// Modo Console: Steam Big Picture em tela cheia dentro do gamescope (o Modo Jogo liga sozinho).
    fn console_mode(&mut self) {
        if crate::apps::first_in_path(&["gamescope"]).is_some() && crate::apps::first_in_path(&["steam"]).is_some() {
            crate::apps::spawn("gamescope -f -e -- steam -gamepadui");
        } else {
            self.notify_local("Modo Console", "Instale gamescope e steam para usar o Modo Console.", false);
        }
    }

    /// Menu da bateria (como no macOS): nível, tempo e Modo de Pouca Energia.
    fn open_battery_menu(&mut self) {
        let st = self.shell.status.clone();
        let Some((pct, charging)) = st.battery else { return };
        let x = self.shell.menubar.item_x(BarItem::Battery).map(|(a, _)| a).unwrap_or(0.0);
        let items = vec![
            MenuItem::new(&format!("Bateria  {pct}%"), MenuAction::About).enabled(false),
            MenuItem::new(if charging { "Carregando" } else { "Fonte: bateria" }, MenuAction::About).enabled(false),
            MenuItem::sep(),
            MenuItem::new("Modo de Pouca Energia", MenuAction::LowPower).check(self.low_power),
        ];
        let mut menu = Menu::new(items, (0, 35), 230.0);
        let (w, _) = menu.size();
        menu.pos = ((x as i32).min(self.screen.w - w - 6), 35);
        menu.origin = Some(BarItem::Battery);
        self.shell.menu = Some(menu);
    }

    /// Forçar Encerrar: lista os apps abertos; clicar mata o processo (SIGKILL).
    pub fn open_force_quit(&mut self) {
        self.shell.close_popups();
        let apps = self.apps_by_mru();
        self.switch_windows = apps.iter().map(|(_, w)| w.clone()).collect();
        let mut entries = Vec::new();
        for (_, w) in &apps {
            let app = crate::apps::for_app_id(&self.shell.apps, &Self::app_id(w)).cloned();
            let name = app.as_ref().map(|a| a.name.clone()).unwrap_or_else(|| Self::title(w));
            let icon = self.window_icon(w);
            let pm = self.shell.icons.get_theme(&icon, &name).clone();
            entries.push(crate::shell::force_quit::FqEntry { name, icon: pm, hung: false });
        }
        self.shell.force_quit.show(entries);
    }

    /// Encerra à força o app escolhido na janela de Forçar Encerrar.
    pub fn force_quit_selected(&mut self) {
        let i = self.shell.force_quit.selected;
        if self.switch_windows.get(i).is_some() {
            self.on_menu_action(MenuAction::ForceQuit(i));
        }
        self.shell.force_quit.open = false;
        self.dirty = true;
    }

    /// Teste: arrasto completo (pressiona, move, solta) sem mouse de verdade.
    fn debug_drag(&mut self, from: crate::state::DragFrom, to: Point<f64, Logical>) {
        let pointer = self.seat.get_pointer().unwrap();
        let start = pointer.current_location();
        self.start_drag_out(from, SERIAL_COUNTER.next_serial(), BTN_LEFT, start);
        // um passo a cada 30 ms e solta 400 ms depois (dá tempo ao app de aceitar, como num arrasto real)
        for k in 1..=10u32 {
            let p = start + (to - start).upscale(k as f64 / 10.0);
            let _ = self.loop_handle.insert_source(
                smithay::reexports::calloop::timer::Timer::from_duration(std::time::Duration::from_millis(30 * k as u64)),
                move |_, _, st| {
                    let pointer = st.seat.get_pointer().unwrap();
                    let under = st.surface_under(p);
                    st.last_pointer = p;
                    let t = st.start_time.elapsed().as_millis() as u32;
                    pointer.motion(st, under, &MotionEvent { location: p, serial: SERIAL_COUNTER.next_serial(), time: t });
                    pointer.frame(st);
                    smithay::reexports::calloop::timer::TimeoutAction::Drop
                },
            );
        }
        let _ = self.loop_handle.insert_source(smithay::reexports::calloop::timer::Timer::from_duration(std::time::Duration::from_millis(700)), |_, _, st| {
            let pointer = st.seat.get_pointer().unwrap();
            let t = st.start_time.elapsed().as_millis() as u32;
            pointer.button(st, &ButtonEvent { button: BTN_LEFT, state: ButtonState::Released, serial: SERIAL_COUNTER.next_serial(), time: t });
            pointer.frame(st);
            smithay::reexports::calloop::timer::TimeoutAction::Drop
        });
        let _ = pointer;
    }

    fn installer_action(&mut self, h: crate::shell::installer::InstHit) {
        use crate::shell::installer::InstHit;
        let inst = &mut self.shell.installer;
        match h {
            InstHit::Next if inst.step == 4 => self.start_install(),
            InstHit::Next => {
                inst.step += 1;
                inst.focus = if inst.step == 3 { Some(0) } else { None };
            }
            InstHit::Back => {
                inst.step = inst.step.saturating_sub(1);
                inst.focus = None;
            }
            InstHit::Lang(i) => inst.lang = i,
            InstHit::Keymap(i) => {
                inst.keymap = i;
                // o teclado muda já no live, para digitar a senha certa
                let xkb = ["br", "us", "us(intl)"][i];
                self.settings.set("kb_layout", xkb);
                self.apply_internal_settings();
            }
            InstHit::Zone(z) => {
                inst.zone = z;
                inst.zone_query.clear();
            }
            InstHit::Disk(i) => inst.disk = Some(i),
            InstHit::Encrypt => inst.encrypt = !inst.encrypt,
            InstHit::Field(i) => inst.focus = Some(i),
            InstHit::Network => {
                self.open_settings();
                self.settings_act(crate::shell::settings_model::Act::Page("wifi".into()));
            }
            InstHit::Reboot => crate::apps::spawn("systemctl reboot"),
            InstHit::Retry => {
                inst.step = 4;
                inst.error = None;
                inst.progress = (0.0, String::new());
            }
            InstHit::Try => inst.open = false,
            InstHit::Log => crate::apps::spawn("mousepad /var/log/obelisk-install.log || foot -e less /var/log/obelisk-install.log"),
        }
        self.shell.installer.focus = self.shell.installer.focus.filter(|_| matches!(self.shell.installer.step, 1 | 3));
        self.dirty = true;
    }

    fn welcome_action(&mut self, h: crate::shell::welcome::WelcomeHit) {
        use crate::shell::welcome::{WelcomeHit, STEPS};
        let w = &mut self.shell.welcome;
        match h {
            WelcomeHit::Next if w.step + 1 >= STEPS => w.finish(),
            WelcomeHit::Next => w.step += 1,
            WelcomeHit::Back => w.step = w.step.saturating_sub(1),
            WelcomeHit::Skip => w.finish(),
            WelcomeHit::Appearance(k) => self.settings_act(crate::shell::settings_model::Act::Set("appearance".into(), k.into())),
            WelcomeHit::GameAuto => {
                let on = !self.settings.game_auto;
                self.settings_act(crate::shell::settings_model::Act::Set("game_auto".into(), on.to_string()));
            }
            WelcomeHit::Wifi => {
                self.shell.welcome.open = false;
                self.open_settings();
                self.settings_act(crate::shell::settings_model::Act::Page("wifi".into()));
                // o assistente volta depois (a próxima sessão o mostra de novo se não terminar)
            }
            WelcomeHit::Steam => crate::apps::spawn("steam"),
        }
        self.dirty = true;
    }

    fn start_desk_rename(&mut self, i: usize) {
        if let Some(it) = self.shell.desktop.items.get(i) {
            self.shell.desktop.selected = Some(i);
            self.shell.desktop.renaming = Some((i, it.name.clone()));
            self.dirty = true;
        }
    }

    /// Clique direito na mesa (ou num ícone dela), como o menu do Finder na mesa.
    fn open_desktop_menu(&mut self, pos: Point<f64, Logical>, icon: Option<usize>) {
        self.shell.close_popups();
        let items = match icon {
            Some(i) => vec![
                MenuItem::new("Abrir", MenuAction::DeskOpen(i)),
                MenuItem::new("Mostrar no Arquivos", MenuAction::DeskShow(i)),
                MenuItem::new("Renomear", MenuAction::DeskRename(i)),
                MenuItem::sep(),
                MenuItem::new("Mover para o Lixo", MenuAction::DeskTrash(i)),
            ],
            None => vec![
                MenuItem::new("Nova Pasta", MenuAction::NewFolder),
                MenuItem::sep(),
                MenuItem::new("Mudar Papel de Parede…", MenuAction::SettingsPage("wallpaper".into())),
                MenuItem::new(if self.settings.widgets { "Ocultar Widgets" } else { "Mostrar Widgets" }, MenuAction::ToggleWidgets),
                MenuItem::sep(),
                MenuItem::new("Ajustes da Mesa e Dock…", MenuAction::SettingsPage("dock".into())),
            ],
        };
        let mut menu = Menu::new(items, (pos.x as i32, pos.y as i32), 220.0);
        let (w, h) = menu.size();
        menu.pos = ((pos.x as i32).min(self.screen.w - w - 4), (pos.y as i32).min(self.screen.h - h - 4));
        self.shell.menu = Some(menu);
    }

    fn open_dock_menu(&mut self, idx: usize) {
        let Some(entry) = self.shell.dock.entries.get(idx).cloned() else { return };
        let items = match &entry {
            Entry::App { pinned, running, .. } => {
                let mut v = vec![MenuItem::new(if *running { "Nova Janela" } else { "Abrir" }, MenuAction::DockOpen(idx))];
                if *running {
                    v.push(MenuItem::new("Mostrar Todas as Janelas", MenuAction::Expose(idx)));
                }
                v.push(MenuItem::sep());
                v.push(MenuItem::new("Manter no Dock", MenuAction::DockTogglePin(idx)).check(*pinned));
                if *running {
                    v.push(MenuItem::sep());
                    v.push(MenuItem::new("Encerrar", MenuAction::DockQuit(idx)));
                }
                v
            }
            Entry::Trash { .. } => vec![MenuItem::new("Abrir", MenuAction::OpenPath("trash:///".into()))],
            _ => return,
        };
        let x = self.shell.dock.slot_x(idx).unwrap_or(0.0) as i32;
        let mut menu = Menu::new(items, (0, 0), 200.0);
        let (_, h) = menu.size();
        // abre para cima, logo acima da placa do Dock
        menu.pos = (x, self.screen.h - 80 - h);
        self.shell.dock.menu_open = true;
        self.shell.menu = Some(menu);
    }

    // ---------------------------------------------------------------- ações

    fn menu_activate(&mut self, idx: usize) {
        let Some(menu) = &mut self.shell.menu else { return };
        let Some(item) = menu.items.get(idx).cloned() else { return };
        let Some(action) = item.action else { return };
        if matches!(action, MenuAction::Restart | MenuAction::Shutdown) && menu.confirming != Some(idx) {
            menu.confirming = Some(idx);
            menu.hover = None;
            return;
        }
        self.shell.menu = None;
        self.shell.dock.menu_open = false;
        self.on_menu_action(action);
    }

    pub fn on_menu_action(&mut self, action: MenuAction) {
        match action {
            MenuAction::About => self.on_menu_action(MenuAction::SettingsPage("about".into())),
            MenuAction::Settings => self.open_settings(),
            MenuAction::ControlCenter => self.shell.cc.toggle(),
            MenuAction::ToggleAppearance => self.toggle_appearance(),
            MenuAction::Sleep => crate::apps::spawn("systemctl suspend"),
            MenuAction::Restart => crate::apps::spawn("systemctl reboot"),
            MenuAction::Shutdown => crate::apps::spawn("systemctl poweroff"),
            MenuAction::Logout => self.loop_signal.stop(),
            MenuAction::HideApp => self.hide_focused_app(),
            MenuAction::QuitApp => {
                if let Some(w) = self.focused_window() {
                    let id = Self::app_id(&w);
                    for x in self.all_windows() {
                        if Self::app_id(&x) == id {
                            self.close_window(&x);
                        }
                    }
                }
            }
            MenuAction::Minimize => self.run_key_action(KeyAction::Minimize),
            MenuAction::Zoom => self.run_key_action(KeyAction::Zoom),
            MenuAction::Fullscreen => self.run_key_action(KeyAction::Fullscreen),
            MenuAction::BringAllToFront => {
                let ws: Vec<Window> = self.space.elements().cloned().collect();
                for w in ws {
                    self.space.raise_element(&w, false);
                }
            }
            MenuAction::FocusWindow(i) => {
                let w = self.space.elements().nth(i).cloned();
                if let Some(w) = w {
                    self.focus_window(&w);
                }
            }
            MenuAction::Workspace(n) => self.switch_workspace(n),
            MenuAction::OpenPath(p) => self.open_path(&p),
            MenuAction::Launchpad => self.toggle_launchpad(),
            MenuAction::MissionControl => self.open_mission(),
            MenuAction::DockOpen(i) => {
                if let Some(Entry::App { app, .. }) = self.shell.dock.entries.get(i).cloned() {
                    crate::apps::spawn(&app.exec);
                }
            }
            MenuAction::DockTogglePin(i) => {
                if let Some(Entry::App { app, pinned, .. }) = self.shell.dock.entries.get(i).cloned() {
                    if pinned {
                        self.shell.pinned.retain(|p| *p != app.id);
                    } else {
                        self.shell.pinned.push(app.id.clone());
                    }
                    self.shell.save_pins();
                }
            }
            MenuAction::ForceQuitMenu => self.open_force_quit(),
            MenuAction::Lock => self.lock_screen(),
            MenuAction::ActivityMonitor => self.shell.activity.toggle(),
            MenuAction::TrayMenu(i, id) => {
                if let (Some(item), Some(tray)) = (self.tray_items.get(i).cloned(), &self.tray) {
                    tray.menu_click(&item, id);
                }
            }
            MenuAction::OnTop => {
                if let Some(w) = self.focused_window() {
                    let on = {
                        let mut m = meta(&w).lock().unwrap();
                        m.on_top = !m.on_top;
                        m.on_top
                    };
                    if on {
                        self.space.raise_element(&w, true);
                    }
                }
            }
            MenuAction::ConsoleMode => self.console_mode(),
            MenuAction::StageManager => self.stage_manager = !self.stage_manager,
            MenuAction::LowPower => {
                let on = !self.low_power;
                self.set_low_power(on);
            }
            MenuAction::AutoAppearance => self.auto_appearance = !self.auto_appearance,
            MenuAction::NightShift => {
                self.night_shift = !self.night_shift;
                self.dirty = true;
            }
            MenuAction::Keys(mods, key) => self.send_chord(&mods, key),
            MenuAction::Clip(key) => {
                let mods = if self.focused_is_terminal() { vec![CTRL, SHIFT] } else { vec![CTRL] };
                self.send_chord(&mods, key);
            }
            MenuAction::SettingsPage(p) => {
                if !self.shell.settings.open {
                    self.open_settings();
                }
                self.settings_act(crate::shell::settings_model::Act::Page(p));
            }
            MenuAction::NewFolder => {
                let dir = crate::shell::desktop::desktop_dir();
                let _ = std::fs::create_dir_all(&dir);
                let mut name = "pasta sem título".to_string();
                let mut n = 2;
                while dir.join(&name).exists() {
                    name = format!("pasta sem título {n}");
                    n += 1;
                }
                let _ = std::fs::create_dir(dir.join(&name));
            }
            MenuAction::ToggleWidgets => {
                let on = !self.settings.widgets;
                self.settings.set("widgets", if on { "true" } else { "false" });
                self.settings.save();
                self.apply_settings();
            }
            MenuAction::DeskOpen(i) => {
                if let Some(it) = self.shell.desktop.items.get(i) {
                    let q = crate::shell::settings_model::q(&it.path);
                    if it.path.ends_with(".desktop") {
                        crate::apps::spawn(&format!("gio launch {q} || gtk-launch {q}"));
                    } else {
                        crate::apps::spawn(&format!("xdg-open {q}"));
                    }
                }
            }
            MenuAction::DeskShow(i) => {
                if let Some(it) = self.shell.desktop.items.get(i) {
                    crate::apps::spawn(&format!(
                        "dbus-send --session --dest=org.freedesktop.FileManager1 --type=method_call /org/freedesktop/FileManager1 org.freedesktop.FileManager1.ShowItems array:string:{} string:''",
                        crate::shell::settings_model::q(&format!("file://{}", it.path))
                    ));
                }
            }
            MenuAction::DeskRename(i) => self.start_desk_rename(i),
            MenuAction::DeskTrash(i) => {
                if let Some(it) = self.shell.desktop.items.get(i) {
                    crate::apps::spawn(&format!("gio trash {}", crate::shell::settings_model::q(&it.path)));
                    self.shell.desktop.selected = None;
                }
            }
            MenuAction::Installer => self.shell.installer.show(),
            MenuAction::Store => self.open_store(),
            MenuAction::AboutApp => {
                if let Some(w) = self.focused_window() {
                    let id = Self::app_id(&w);
                    let app = crate::apps::for_app_id(&self.shell.apps, &id).cloned();
                    let name = app.as_ref().map(|a| a.name.clone()).unwrap_or_else(|| self.focused_app_name());
                    let icon = self.window_icon(&w);
                    let pm = self.shell.icons.get_theme(&icon, &name).clone();
                    let (version, package) = app.as_ref().map(|a| crate::shell::about_app::app_version(&a.exec)).unwrap_or_default();
                    let comment = app.as_ref().map(|a| crate::apps::desktop_comment(&a.id)).unwrap_or_default();
                    let ab = &mut self.shell.about_app;
                    *ab = crate::shell::about_app::AboutApp::new(name, version, comment, package, Some(pm));
                }
            }
            MenuAction::Spotlight => {
                self.shell.close_popups();
                self.shell.launcher.open = true;
            }
            MenuAction::Emoji => self.shell.emoji.open = true,
            MenuAction::Expose(i) => {
                if let Some(Entry::App { app, .. }) = self.shell.dock.entries.get(i).cloned() {
                    self.expose_app = Some(app.id.clone());
                    self.open_mission();
                }
            }
            MenuAction::ForceQuit(i) => {
                if let Some(w) = self.switch_windows.get(i).cloned() {
                    match self.window_pid(&w) {
                        // nunca matar o próprio compositor (clientes internos)
                        Some(pid) if pid > 1 && pid as u32 != std::process::id() => crate::apps::spawn(&format!("kill -KILL {pid}")),
                        _ => self.close_window(&w),
                    }
                }
            }
            MenuAction::DockQuit(i) => {
                if let Some(Entry::App { app, .. }) = self.shell.dock.entries.get(i).cloned() {
                    for w in self.all_windows() {
                        if crate::apps::matches(&app, &Self::app_id(&w)) {
                            self.close_window(&w);
                        }
                    }
                }
            }
        }
    }

    fn cc_action(&mut self, h: CcHit) {
        let st = self.shell.status.clone();
        match h {
            CcHit::Wifi => self.services.set_wifi(!st.wifi_on),
            CcHit::Bluetooth => {
                if st.bt_available {
                    self.services.set_bluetooth(!st.bt_on)
                }
            }
            CcHit::Focus => self.shell.cc.focus_on = !self.shell.cc.focus_on,
            CcHit::Appearance => self.toggle_appearance(),
            CcHit::GameMode => self.toggle_game_mode(),
            CcHit::Tile(0) => {
                self.shell.cc.toggle();
                self.request_screenshot();
            }
            CcHit::Tile(1) => self.open_mission(),
            CcHit::Tile(2) => self.toggle_appearance(),
            CcHit::Tile(3) => {
                self.shell.cc.toggle();
                self.lock_screen();
            }
            CcHit::Tile(_) => {}
            CcHit::Brightness(v) => {
                self.shell.cc.dragging = Some(Slider::Brightness);
                self.services.set_brightness(v);
            }
            CcHit::Volume(v) => {
                self.shell.cc.dragging = Some(Slider::Volume);
                self.services.set_volume(v);
            }
            CcHit::Mute => self.services.toggle_mute(),
            CcHit::MediaPrev => self.services.media("previous"),
            CcHit::MediaPlay => self.services.media("play-pause"),
            CcHit::MediaNext => self.services.media("next"),
        }
    }

    fn dock_activate(&mut self, idx: usize) {
        let Some(entry) = self.shell.dock.entries.get(idx).cloned() else { return };
        match entry {
            Entry::Launchpad => self.toggle_launchpad(),
            Entry::Separator => {}
            Entry::Trash { .. } => self.open_path("trash:///"),
            Entry::Minimized { index, .. } => {
                if let Some(w) = self.minimized.get(index).cloned() {
                    self.restore(&w);
                }
            }
            Entry::App { app, .. } => {
                let visible = self.space.elements().rev().find(|w| crate::apps::matches(&app, &Self::app_id(w))).cloned();
                let hidden = self.minimized.iter().find(|w| crate::apps::matches(&app, &Self::app_id(w))).cloned();
                let elsewhere = self
                    .ws_store
                    .iter()
                    .enumerate()
                    .find_map(|(i, ws)| ws.iter().find(|(w, _)| crate::apps::matches(&app, &Self::app_id(w))).map(|(w, _)| (i, w.clone())));
                if let Some(w) = visible {
                    self.focus_window(&w);
                } else if let Some(w) = hidden {
                    self.restore(&w);
                } else if let Some((ws, w)) = elsewhere {
                    self.switch_workspace(ws);
                    self.focus_window(&w);
                } else {
                    self.shell.dock.start_bounce(idx);
                    crate::apps::spawn(&app.exec);
                }
            }
        }
    }

    fn hide_focused_app(&mut self) {
        if let Some(w) = self.focused_window() {
            let id = Self::app_id(&w);
            let same: Vec<Window> = self.space.elements().filter(|x| Self::app_id(x) == id).cloned().collect();
            for x in same {
                self.minimize(&x);
            }
        }
    }

    pub fn toggle_game_mode(&mut self) {
        let ids: Vec<String> = self.all_windows().iter().map(Self::app_id).collect();
        let running = ids.iter().any(|i| self.game.is_game(i));
        self.game.toggle(running);
        self.services.set_paused(self.game.active);
    }

    fn toggle_appearance(&mut self) {
        self.shell.theme = if self.shell.theme.appearance == Appearance::Dark { Theme::light() } else { Theme::dark() }.with_accent(&self.settings.accent);
        self.shell.decos.clear();
    }

    /// App interno: "settings[:página]", "store", "installer".
    pub fn open_internal(&mut self, what: &str) {
        let (app, page) = what.split_once(':').unwrap_or((what, ""));
        match app {
            "settings" => {
                self.shell.settings.store = false;
                if !self.shell.settings.open {
                    self.open_settings();
                }
                if !page.is_empty() {
                    self.settings_act(crate::shell::settings_model::Act::Page(page.into()));
                }
            }
            "store" => self.open_store(),
            "installer" => self.shell.installer.show(),
            _ => tracing::warn!("app interno desconhecido: {what}"),
        }
        self.dirty = true;
    }

    /// App Store: a janela dos Ajustes no modo loja.
    pub fn open_store(&mut self) {
        self.shell.close_popups();
        let s = &mut self.shell.settings;
        s.store = true;
        s.open = false;
        s.toggle();
        s.set_page("s:descobrir");
        self.setctl.focus = None;
        self.refresh_settings_page();
    }

    fn open_settings(&mut self) {
        let was = self.shell.settings.open;
        self.shell.close_popups();
        if !was {
            self.shell.settings.store = false;
            self.shell.settings.toggle();
            if self.shell.settings.page.starts_with("s:") {
                self.shell.settings.set_page("wifi");
            }
            self.setctl.focus = None;
            self.setctl.wallpapers = None;
            self.setctl.avatar = None;
            self.refresh_settings_page();
        }
    }

    fn open_path(&mut self, p: &str) {
        crate::apps::spawn(&format!("xdg-open {}", shell_quote(p)));
    }
}

fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

// keycodes xkb (evdev + 8) usados pelos menus Arquivo/Editar/Visualizar
const CTRL: u32 = 37;
const SHIFT: u32 = 50;
const K_N: u32 = 57;
const K_T: u32 = 28;
const K_O: u32 = 32;
const K_S: u32 = 39;
const K_P: u32 = 33;
const K_Z: u32 = 52;
const K_X: u32 = 53;
const K_C: u32 = 54;
const K_V: u32 = 55;
const K_A: u32 = 38;
const K_F: u32 = 41;
const K_R: u32 = 27;
const K_PLUS: u32 = 21;
const K_MINUS: u32 = 20;
const K_0: u32 = 19;
