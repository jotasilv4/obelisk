//! Montagem da cena, frente → trás:
//! cursor · ⌘Tab · menu · Central de Controle · Busca · [Launchpad | Mission Control] ·
//! barra de menus · Dock · janelas (com decoração) · papel de parede.

#[allow(unused_imports)]
use smithay::wayland::seat::WaylandFocus;
use std::sync::Mutex;
use std::time::Instant;

use smithay::{
    backend::renderer::{
        element::{
            memory::MemoryRenderBufferRenderElement,
            surface::{render_elements_from_surface_tree, WaylandSurfaceRenderElement},
            utils::RescaleRenderElement,
            AsRenderElements, Kind,
        },
        gles::GlesRenderer,
    },
    desktop::Window,
    input::pointer::{CursorImageAttributes, CursorImageStatus},
    utils::{Physical, Point, Scale},
    wayland::compositor::with_states,
};

use crate::shell::menubar::MenuBarInfo;
use crate::state::{meta, Obelisk};
use crate::ui::canvas::Canvas;
use crate::ui::text_glyph;
use crate::ui::theme::metrics;

smithay::backend::renderer::element::render_elements! {
    pub ObeliskElement<=GlesRenderer>;
    Surface=WaylandSurfaceRenderElement<GlesRenderer>,
    Memory=MemoryRenderBufferRenderElement<GlesRenderer>,
    Rounded=crate::rounded::RoundedSurface,
    Scaled=RescaleRenderElement<WaylandSurfaceRenderElement<GlesRenderer>>,
    ScaledRounded=RescaleRenderElement<crate::rounded::RoundedSurface>,
    ScaledMemory=RescaleRenderElement<MemoryRenderBufferRenderElement<GlesRenderer>>,
}

/// Aplica escala (em torno de `origin`, px físicos) a um elemento já montado.
fn rescale(e: ObeliskElement, origin: Point<i32, Physical>, s: f64) -> ObeliskElement {
    match e {
        ObeliskElement::Surface(x) => ObeliskElement::Scaled(RescaleRenderElement::from_element(x, origin, s)),
        ObeliskElement::Rounded(x) => ObeliskElement::ScaledRounded(RescaleRenderElement::from_element(x, origin, s)),
        ObeliskElement::Memory(x) => ObeliskElement::ScaledMemory(RescaleRenderElement::from_element(x, origin, s)),
        other => other,
    }
}

impl Obelisk {
    pub fn build_elements(&mut self, renderer: &mut GlesRenderer) -> Vec<ObeliskElement> {
        let scale = self.scale;
        // escala da tela (pode ser fracionária: 1,25, 1,5…); a UI é desenhada em `scale` (inteiro, arredondado
        // para cima) e a GPU reduz, como o macOS faz com "mais espaço"
        let fs = self.render_scale.unwrap_or(self.fscale);
        let (sw, sh) = (self.screen.w, self.screen.h);
        let screen = (sw as f32, sh as f32);
        let phys = |x: i32, y: i32| -> Point<f64, Physical> { Point::from((x as f64 * fs, y as f64 * fs)) };
        let now = Instant::now();
        let mut out: Vec<ObeliskElement> = Vec::new();

        let pointer_loc = self.seat.get_pointer().map(|p| p.current_location()).unwrap_or_default();
        let fullscreen = self.any_fullscreen_on_top();
        let app_name = self.focused_app_name();
        let focused = self.focused_window();
        let theme = self.shell.theme;
        let has_window = focused.is_some();
        let workspace = self.workspace;
        let used = self.ws_store.iter().rposition(|w| !w.is_empty()).map(|i| i + 1).unwrap_or(0);
        let workspaces = used.max(workspace + 1).max(4).min(9);
        let open_bar = self.shell.menu.as_ref().and_then(|m| m.origin).or(if self.shell.cc.open {
            Some(crate::shell::menubar::BarItem::ControlCenter)
        } else {
            None
        });

        let game = self.game.active;

        // ---- tela bloqueada: só ela (e o cursor)
        if self.lock.active {
            let bd = self.shell.wallpaper.blurred.clone();
            self.shell.wallpaper.update(sw, sh, scale, theme.appearance);
            let power = self.backend == crate::state::BackendKind::Drm;
            self.lock.update(sw, sh, scale, bd.as_ref(), power);
            self.push_cursor(renderer, pointer_loc, fs, &mut out);
            out.extend(self.lock.layer.element(renderer, phys(0, 0), 1.0).map(Into::into));
            return out;
        }

        // ---- jogo em tela cheia: só ele (e o cursor). Menos composição e, no backend
        // nativo, permite que o buffer do jogo vá direto para a tela (direct scanout).
        if fullscreen {
            self.push_cursor(renderer, pointer_loc, fs, &mut out);
            if let Some(w) = self.top_window() {
                self.push_window(renderer, &w, true, fs, &mut out);
            }
            return out;
        }

        // ---- camadas de UI (CPU → textura)
        self.shell.wallpaper.update(sw, sh, scale, theme.appearance);
        let notch_drop = self.notch_drop_frac(pointer_loc);
        self.update_dock_visibility();
        let special = if self.shell.settings.open { Some(self.settings_special()) } else { None };
        let shell = &mut self.shell;
        // Modo Jogo: sem vidro (o desfoque é o efeito mais caro)
        let backdrop = if game { None } else { shell.wallpaper.blurred.clone() };
        let bd = backdrop.as_ref();
        if !fullscreen {
            let clock = shell.clock.clone();
            let info = MenuBarInfo {
                app_name: &app_name,
                has_window,
                workspace,
                workspaces,
                status: &shell.status,
                open: open_bar,
                luminance: shell.wallpaper.top_luminance,
                recording: None,
                tray: self.tray_items.iter().map(|t| t.icon.clone()).collect(),
            };
            shell.menubar.update(&theme, sw, scale, &clock, &info);
            // Modo Jogo: Dock sem magnificação
            let ptr = if game || !self.settings.dock_magnify { None } else { Some((pointer_loc.x, pointer_loc.y)) };
            shell.dock.update(&theme, &mut shell.icons, sw, sh, scale, ptr, bd, now);
            let st = shell.status.clone();
            let banner = self.banners.front().map(|n| (n, self.banners.len() - 1));
            let title = focused.as_ref().map(Self::title).unwrap_or_default();
            let idle = Some((self.user_host.as_str(), title.as_str()));
            let rec = self.recorder.as_ref().map(|r| r.started.elapsed().as_secs());
            let auth = self.auth.as_ref().map(|a| (a.message.as_str(), self.auth_password.chars().count()));
            // selo do player na capa: ícone do app (spotify, firefox…) se ele tiver .desktop
            let badge = st.media.as_ref().filter(|m| !m.player.is_empty()).and_then(|m| {
                let app = crate::apps::for_app_id(&shell.apps, &m.player).cloned()?;
                Some(shell.icons.get_theme(&app.icon, &app.name).clone())
            });
            let extra = crate::shell::notch::NotchExtra {
                capture: self.capture_toast.as_ref().map(|t| (t.0.as_str(), t.1.as_ref(), t.2)),
                drop: notch_drop,
                shelf: &self.shelf,
                badge: badge.as_ref(),
            };
            shell.notch.update(&theme, &st, sw, scale, game, true, banner, &mut shell.icons, idle, rec, auth, &extra);
        }
        if shell.launcher.open {
            shell.launcher.update(&theme, &mut shell.icons, sw, sh, scale, bd);
        }
        if shell.launchpad.open {
            shell.launchpad.update(&shell.apps, &mut shell.icons, sw, sh, scale, bd);
        }
        if shell.switcher.open {
            shell.switcher.update(&theme, &mut shell.icons, sw, sh, scale, bd);
        }
        if shell.cc.open {
            let st = shell.status.clone();
            shell.cc.game_on = game;
            shell.cc.update(&theme, &st, sw, sh, scale, bd);
        }
        if let Some(m) = &mut shell.menu {
            m.update(&theme, scale, bd, screen);
        }
        if shell.settings.open {
            let special = special.unwrap_or_default();
            let page = shell.settings.page;
            let empty = Vec::new();
            let groups = self.setctl.models.get(page).unwrap_or(&empty);
            shell.settings.update(&theme, groups, &self.setctl.fields, self.setctl.focus.as_deref(), &special, sw, sh, scale, bd);
        }
        let show_widgets = self.settings.widgets && !game;
        // ícones da mesa: à esquerda dos widgets
        {
            let right = if show_widgets { shell.widgets.pos.0 as f32 - 8.0 } else { sw as f32 - 12.0 };
            shell.desktop.update(&theme, sw, sh, scale, right, 32.0 + 14.0);
        }
        if show_widgets {
            let st = shell.status.clone();
            shell.widgets.update(&theme, &st, sw, scale, bd, screen, has_window);
        }
        if shell.emoji.open {
            shell.emoji.update(&theme, sw, sh, scale, bd);
        }
        if shell.force_quit.open {
            shell.force_quit.update(&theme, sw, sh, scale, bd);
        }
        if shell.about_app.open {
            shell.about_app.update(&theme, sw, sh, scale, bd);
        }
        if shell.activity.open {
            shell.activity.update(&theme, sw, sh, scale, bd);
        }
        if shell.nc.open {
            shell.nc.update(&theme, &mut shell.icons, &self.notifs, sw, sh, scale, bd);
        }
        if !shell.cursor.is_set() {
            let mut c = Canvas::new(24, 24, scale as f32).unwrap();
            text_glyph::arrow_cursor(&mut c, 0.0, 0.0);
            shell.cursor.set(&c);
        }

        // ---- cursor (fora da captura de tela) e sobreposição das capturas
        let shooting = self.screenshot.is_some();
        if !shooting {
            self.push_cursor(renderer, pointer_loc, fs, &mut out);
        }
        // ---- instalador (sistema live): por cima de tudo (menos o cursor)
        if self.shell.installer.open {
            let st = self.shell.status.clone();
            let network = st.ssid.clone().or_else(|| st.wired.then(|| "Cabo de rede".to_string()));
            let bd = self.shell.wallpaper.blurred.clone();
            self.shell.installer.update(&theme, network.as_deref(), sw, sh, scale, bd.as_ref());
            out.extend(self.shell.installer.layer.element(renderer, phys(0, 0), 1.0).map(Into::into));
        }
        // ---- assistente de primeiro uso: por cima de tudo (menos o cursor)
        if self.shell.welcome.open && !self.shell.installer.open {
            let st = self.shell.status.clone();
            let network = st.ssid.clone().or_else(|| st.wired.then(|| "cabo de rede".to_string()));
            let info = crate::shell::welcome::WelcomeInfo {
                user: &self.lock_display_name(),
                appearance: &self.settings.appearance.clone(),
                network,
                game_auto: self.settings.game_auto,
                steam: crate::apps::first_in_path(&["steam"]).is_some(),
            };
            let bd = self.shell.wallpaper.blurred.clone();
            self.shell.welcome.update(&theme, &info, sw, sh, scale, bd.as_ref());
            out.extend(self.shell.welcome.layer.element(renderer, phys(0, 0), 1.0).map(Into::into));
        }
        if !shooting {
            // arquivo sendo arrastado pelo compositor: fantasma sob o cursor
            if self.drag_out.is_some() && self.seat.get_pointer().is_some_and(|p| p.is_grabbed()) {
                let (gx, gy) = (pointer_loc.x as i32 - 20, pointer_loc.y as i32 - 12);
                out.extend(self.drag_ghost.element(renderer, phys(gx, gy), 1.0).map(Into::into));
            }
        }
        if (self.shell.capture.active || self.shell.capture.countdown.is_some()) && !shooting {
            self.shell.capture.update(&theme, sw, sh, scale, backdrop.as_ref());
            out.extend(self.shell.capture.layer.element(renderer, phys(0, 0), 1.0).map(Into::into));
        }

        // ---- painéis
        let sh_ = &self.shell;
        if sh_.switcher.open {
            let (x, y) = sh_.switcher.pos;
            out.extend(sh_.switcher.layer.element(renderer, phys(x, y), 1.0).map(Into::into));
        }
        if let Some(m) = &sh_.menu {
            let (x, y) = m.buf_pos();
            out.extend(m.layer.element(renderer, phys(x, y), m.alpha()).map(Into::into));
        }
        if sh_.settings.open {
            let (x, y) = ((sh_.settings.pos.0 - 44.0) as i32, (sh_.settings.pos.1 - 44.0) as i32);
            out.extend(sh_.settings.layer.element(renderer, phys(x, y), 1.0).map(Into::into));
        }
        if sh_.emoji.open {
            let (x, y) = ((sh_.emoji.pos.0 - 40.0) as i32, (sh_.emoji.pos.1 - 40.0) as i32);
            out.extend(sh_.emoji.layer.element(renderer, phys(x, y), 1.0).map(Into::into));
        }
        if sh_.about_app.open {
            let (x, y) = ((sh_.about_app.pos.0 - 44.0) as i32, (sh_.about_app.pos.1 - 44.0) as i32);
            out.extend(sh_.about_app.layer.element(renderer, phys(x, y), 1.0).map(Into::into));
        }
        if sh_.force_quit.open {
            let (x, y) = ((sh_.force_quit.pos.0 - 44.0) as i32, (sh_.force_quit.pos.1 - 44.0) as i32);
            out.extend(sh_.force_quit.layer.element(renderer, phys(x, y), 1.0).map(Into::into));
        }
        if sh_.activity.open {
            let (x, y) = ((sh_.activity.pos.0 - 40.0) as i32, (sh_.activity.pos.1 - 40.0) as i32);
            out.extend(sh_.activity.layer.element(renderer, phys(x, y), 1.0).map(Into::into));
        }
        if sh_.nc.open {
            let (x, y) = sh_.nc.buf_pos();
            out.extend(sh_.nc.layer.element(renderer, phys(x, y), 1.0).map(Into::into));
        }
        if sh_.cc.open {
            let (x, y) = sh_.cc.buf_pos();
            out.extend(sh_.cc.layer.element(renderer, phys(x, y), sh_.cc.alpha()).map(Into::into));
        }
        if sh_.launcher.open {
            let (x, y) = sh_.launcher.buf_pos();
            out.extend(sh_.launcher.layer.element(renderer, phys(x, y), 1.0).map(Into::into));
        }
        if sh_.launchpad.open {
            let (alpha, _) = sh_.launchpad.intro();
            out.extend(sh_.launchpad.layer.element(renderer, phys(0, 0), alpha).map(Into::into));
            if alpha >= 1.0 {
                return out; // cobre a tela inteira
            }
        }
        if self.shell.mission.open {
            self.push_mission(renderer, fs, &mut out);
            return out;
        }

        // ---- notch, barra e Dock
        if !fullscreen {
            let nx = self.shell.notch.buf_x as i32;
            out.extend(self.shell.notch.layer.element(renderer, phys(nx, 0), 1.0).map(Into::into));
            out.extend(self.shell.menubar.layer.element(renderer, phys(0, 0), 1.0).map(Into::into));
            let (dx, dy) = self.shell.dock.buf_pos;
            out.extend(self.shell.dock.layer.element(renderer, phys(dx, dy), 1.0).map(Into::into));
        }

        // ---- Stage Manager: miniaturas ao vivo dos outros apps à esquerda
        for (w, (x, y, tw, _)) in self.stage_strip() {
            let geo = w.geometry();
            let sc = tw as f64 / geo.size.w.max(1) as f64;
            let origin: Point<i32, Physical> = Point::<f64, smithay::utils::Logical>::from((x as f64, y as f64)).to_physical_precise_round(fs);
            let render_loc = origin - geo.loc.to_physical_precise_round(fs);
            let elems: Vec<WaylandSurfaceRenderElement<GlesRenderer>> = w.render_elements(renderer, render_loc, Scale::from(fs), 0.95);
            out.extend(elems.into_iter().map(|e| ObeliskElement::Scaled(RescaleRenderElement::from_element(e, origin, sc))));
        }

        // ---- janelas (de cima para baixo)
        let windows: Vec<Window> = self.space.elements().cloned().collect();
        for w in windows.iter().rev() {
            if !self.on_stage(w) {
                continue;
            }
            self.push_window(renderer, w, focused.as_ref() == Some(w), fs, &mut out);
        }

        // ---- widgets (acima do papel de parede, abaixo das janelas)
        if show_widgets {
            let (x, y) = self.shell.widgets.pos;
            out.extend(self.shell.widgets.layer.element(renderer, phys(x, y), 1.0).map(Into::into));
        }
        if !game {
            let (x, y) = self.shell.desktop.pos;
            out.extend(self.shell.desktop.layer.element(renderer, phys(x as i32, y as i32), 1.0).map(Into::into));
        }

        // ---- papel de parede
        out.extend(self.shell.wallpaper.layer.element(renderer, phys(0, 0), 1.0).map(Into::into));

        // ---- Zoom de acessibilidade: amplia tudo em torno do cursor (o ponto sob o cursor não muda,
        // então os cliques continuam batendo onde se vê)
        if self.zoom > 1.0 {
            let o: Point<i32, Physical> = pointer_loc.to_physical_precise_round(fs);
            out = out.into_iter().map(|e| rescale(e, o, self.zoom)).collect();
        }
        out
    }

    fn push_cursor(&mut self, renderer: &mut GlesRenderer, pointer_loc: Point<f64, smithay::utils::Logical>, fs: f64, out: &mut Vec<ObeliskElement>) {
        let scale = self.scale;
        match &self.cursor_status {
            CursorImageStatus::Hidden => {}
            CursorImageStatus::Surface(surface) => {
                let hotspot = with_states(surface, |s| {
                    s.data_map.get::<Mutex<CursorImageAttributes>>().map(|a| a.lock().unwrap().hotspot).unwrap_or_default()
                });
                let p = (pointer_loc - hotspot.to_f64()).to_physical_precise_round(fs);
                let elems: Vec<WaylandSurfaceRenderElement<GlesRenderer>> =
                    render_elements_from_surface_tree(renderer, surface, p, Scale::from(fs), 1.0, Kind::Cursor);
                out.extend(elems.into_iter().map(ObeliskElement::from));
            }
            CursorImageStatus::Named(icon) => {
                let name = icon.name().to_string();
                let key = (name.clone(), scale);
                if !self.shell.cursor_layers.contains_key(&key) {
                    if let Some((pm, hot)) = self.shell.cursors.get(&name, scale as u32) {
                        let mut l = crate::shell::layer::Layer::default();
                        l.set_pixmap(pm, scale);
                        self.shell.cursor_layers.insert(key.clone(), (l, *hot));
                    }
                }
                let el = match self.shell.cursor_layers.get(&key) {
                    Some((l, hot)) => {
                        let p = Point::<f64, Physical>::from((pointer_loc.x * fs - hot.0 as f64, pointer_loc.y * fs - hot.1 as f64));
                        l.element(renderer, p, 1.0)
                    }
                    None => {
                        let p = Point::<f64, Physical>::from(((pointer_loc.x - 1.0) * fs, (pointer_loc.y - 1.0) * fs));
                        self.shell.cursor.element(renderer, p, 1.0)
                    }
                };
                out.extend(el.map(Into::into));
            }
        }
    }

    /// Tela extra (não a principal): papel de parede, janelas que caem nela e o cursor.
    /// A barra de menus, o Dock e o notch ficam só na tela principal.
    pub fn build_elements_secondary(&mut self, renderer: &mut GlesRenderer, geo: smithay::utils::Rectangle<i32, smithay::utils::Logical>, fs: f64) -> Vec<ObeliskElement> {
        let mut out: Vec<ObeliskElement> = Vec::new();
        let scale = fs.ceil() as i32;
        if self.lock.active {
            return out;
        }
        self.view_origin = geo.loc;
        let pointer_loc = self.seat.get_pointer().map(|p| p.current_location()).unwrap_or_default();
        if geo.to_f64().contains(pointer_loc) {
            let local = pointer_loc - geo.loc.to_f64();
            self.push_cursor(renderer, local, fs, &mut out);
        }
        let focused = self.focused_window();
        let windows: Vec<Window> = self.space.elements().cloned().collect();
        for w in windows.iter().rev() {
            if !self.on_stage(w) {
                continue;
            }
            let Some(l) = self.space.element_location(w) else { continue };
            let r = smithay::utils::Rectangle::new(l - smithay::utils::Point::from((0, metrics::TITLEBAR_H)), w.geometry().size + (0, metrics::TITLEBAR_H).into());
            if r.overlaps(geo) {
                self.push_window(renderer, w, focused.as_ref() == Some(w), fs, &mut out);
            }
        }
        self.view_origin = (0, 0).into();
        let theme = self.shell.theme;
        let custom = self.shell.wallpaper.custom.clone();
        let key = format!("{}x{}", geo.size.w, geo.size.h);
        let wp = self.sec_wallpapers.entry(key).or_insert_with(crate::shell::wallpaper::Wallpaper::new);
        if wp.custom != custom {
            wp.set_custom(custom);
        }
        wp.update(geo.size.w, geo.size.h, scale, theme.appearance);
        out.extend(wp.layer.element(renderer, Point::from((0.0, 0.0)), 1.0).map(Into::into));
        out
    }

    /// Janela com cantos arredondados (shader), barra de título e sombra.
    fn push_window(&mut self, renderer: &mut GlesRenderer, w: &Window, is_focused: bool, fs: f64, out: &mut Vec<ObeliskElement>) {
        // animação de abrir (escala 0,94→1 e opacidade, 220 ms) e de minimizar (encolhe até o Dock, 300 ms)
        let Some(loc0) = self.space.element_location(w).map(|l| l - self.view_origin) else { return };
        let anim = {
            let m = meta(w).lock().unwrap();
            let geo = w.geometry();
            let center = Point::<f64, smithay::utils::Logical>::from((loc0.x as f64 + geo.size.w as f64 / 2.0, loc0.y as f64 + geo.size.h as f64 / 2.0));
            if let Some(t0) = m.closing_at {
                let p = (t0.elapsed().as_secs_f64() / 0.15).min(1.0);
                let e = 1.0 - (1.0 - p).powi(2);
                Some((1.0 - 0.06 * e, (1.0 - e) as f32, center))
            } else if let Some((t0, target)) = m.minimizing {
                let p = (t0.elapsed().as_secs_f64() / 0.3).min(1.0);
                let e = p * p; // acelera para dentro do Dock
                Some((1.0 - 0.9 * e, 1.0 - e as f32, target))
            } else if let Some(t0) = m.opened_at.filter(|_| !self.game.active && self.settings.animations) {
                let p = (t0.elapsed().as_secs_f64() / 0.22).min(1.0);
                let e = 1.0 - (1.0 - p).powi(3);
                (p < 1.0).then_some((0.94 + 0.06 * e, e as f32, center))
            } else {
                None
            }
        };
        let start = out.len();
        let alpha = anim.map(|a| a.1).unwrap_or(1.0);
        self.push_window_inner(renderer, w, is_focused, fs, alpha, out);
        if let Some((s, _, origin)) = anim {
            let o: Point<i32, Physical> = origin.to_physical_precise_round(fs);
            let tail: Vec<ObeliskElement> = out.drain(start..).collect();
            out.extend(tail.into_iter().map(|e| rescale(e, o, s)));
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn push_window_inner(&mut self, renderer: &mut GlesRenderer, w: &Window, is_focused: bool, fs: f64, alpha: f32, out: &mut Vec<ObeliskElement>) {
        let scale = self.scale;
        let Some(loc) = self.space.element_location(w).map(|l| l - self.view_origin) else { return };
        let geo = w.geometry();
        let render_loc = (loc - geo.loc).to_physical_precise_round(fs);
        let ssd = self.is_ssd(w);
        let (maximized, is_fs) = {
            let m = meta(w).lock().unwrap();
            (m.maximized, m.fullscreen)
        };
        let round = ssd && !maximized && !is_fs;
        if round && self.round_program.is_none() {
            match crate::rounded::compile(renderer) {
                Ok(p) => self.round_program = Some(p),
                Err(e) => tracing::warn!("shader de cantos arredondados indisponível: {e:?}"),
            }
        }
        match (&self.round_program, round, w.toplevel()) {
            (Some(prog), true, Some(top)) => {
                let wl = top.wl_surface();
                let tb = metrics::TITLEBAR_H;
                let f = fs as f32;
                let frame_rect = [loc.x as f32 * f, (loc.y - tb) as f32 * f, geo.size.w as f32 * f, (geo.size.h + tb) as f32 * f];
                let radius = metrics::WINDOW_RADIUS * f;
                for (popup, popup_off) in smithay::desktop::PopupManager::popups_for_surface(wl) {
                    let offset = (geo.loc + popup_off - popup.geometry().loc).to_physical_precise_round(fs);
                    let pe: Vec<WaylandSurfaceRenderElement<GlesRenderer>> =
                        render_elements_from_surface_tree(renderer, popup.wl_surface(), render_loc + offset, Scale::from(fs), alpha, Kind::Unspecified);
                    out.extend(pe.into_iter().map(ObeliskElement::from));
                }
                let tree: Vec<WaylandSurfaceRenderElement<GlesRenderer>> =
                    render_elements_from_surface_tree(renderer, wl, render_loc, Scale::from(fs), alpha, Kind::Unspecified);
                for e in tree {
                    out.push(crate::rounded::RoundedSurface::new(e, prog.clone(), frame_rect, radius, Scale::from(fs)).into());
                }
            }
            _ => {
                let elems: Vec<WaylandSurfaceRenderElement<GlesRenderer>> = w.render_elements(renderer, render_loc, Scale::from(fs), alpha);
                out.extend(elems.into_iter().map(ObeliskElement::from));
            }
        }
        if !ssd {
            return;
        }
        let Some(id) = Self::win_id(w) else { return };
        let (hover_group, hover_btn) = match &self.shell.hover_title {
            Some((hid, b)) if *hid == id => (true, *b),
            _ => (false, None),
        };
        let title = Self::title(w);
        let tb = metrics::TITLEBAR_H;
        let theme = self.shell.theme;
        let deco = self.shell.decos.entry(id).or_default();
        deco.update_title(&theme, geo.size.w, scale, &title, is_focused, hover_group, hover_btn, maximized);
        deco.update_shadow(geo.size.w, geo.size.h + tb, scale, is_focused, maximized);
        let p = |x: i32, y: i32| Point::<f64, Physical>::from((x as f64 * fs, y as f64 * fs));
        out.extend(deco.title_layer.element(renderer, p(loc.x, loc.y - tb), alpha).map(Into::into));
        let pad = metrics::SHADOW_PAD;
        out.extend(deco.shadow_layer.element(renderer, p(loc.x - pad, loc.y - tb - pad), alpha).map(Into::into));
    }

    /// Mission Control: janelas reais reescaladas para a grade, sobre o fundo do overlay.
    fn push_mission(&mut self, renderer: &mut GlesRenderer, fs: f64, out: &mut Vec<ObeliskElement>) {
        let theme = self.shell.theme;
        let (sw, sh, scale) = (self.screen.w, self.screen.h, self.scale);
        // contornos das mesas para a barra de mesas
        let mut ws: Vec<Vec<(f32, f32, f32, f32)>> = Vec::new();
        let n = self.ws_store.iter().rposition(|w| !w.is_empty()).map(|i| i + 1).unwrap_or(0).max(self.workspace + 1).max(2).min(9);
        for i in 0..n {
            let rects = if i == self.workspace {
                self.space
                    .elements()
                    .filter_map(|w| self.space.element_location(w).map(|l| (l.x as f32, l.y as f32, w.geometry().size.w as f32, w.geometry().size.h as f32)))
                    .collect()
            } else {
                self.ws_store[i].iter().map(|(w, l)| (l.x as f32, l.y as f32, w.geometry().size.w as f32, w.geometry().size.h as f32)).collect()
            };
            ws.push(rects);
        }
        let backdrop = self.shell.wallpaper.blurred.clone();
        let shell = &mut self.shell;
        shell.mission.update(&theme, &mut shell.icons, sw, sh, scale, backdrop.as_ref(), self.workspace, &ws);

        let wins = self.mc_windows.clone();
        for (i, w) in wins.iter().enumerate().rev() {
            if i >= self.shell.mission.windows.len() {
                continue;
            }
            let (rx, ry, rw, _) = self.shell.mission.rect(i);
            let geo = w.geometry();
            if geo.size.w <= 0 {
                continue;
            }
            let sc = rw as f64 / geo.size.w as f64;
            let origin: Point<i32, Physical> = Point::<f64, smithay::utils::Logical>::from((rx as f64, ry as f64)).to_physical_precise_round(fs);
            let render_loc = origin - geo.loc.to_physical_precise_round(fs);
            let elems: Vec<WaylandSurfaceRenderElement<GlesRenderer>> = w.render_elements(renderer, render_loc, Scale::from(fs), 1.0);
            for e in elems {
                out.push(ObeliskElement::Scaled(RescaleRenderElement::from_element(e, origin, sc)));
            }
        }
        out.extend(self.shell.mission.layer.element(renderer, Point::from((0.0, 0.0)), 1.0).map(Into::into));
    }
}

