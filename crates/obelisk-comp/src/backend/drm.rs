//! Backend nativo: fala direto com DRM/KMS (GBM + EGL) e libinput via libseat.
//!
//! - Várias telas: cada conector ligado vira uma `Output`. A principal (com barra, Dock e notch)
//!   fica em (0,0); as outras à direita, só com papel de parede e janelas.
//! - Plugar/desplugar monitor (udev) recria as telas; janelas de uma tela removida voltam à principal.
//! - Resolução/taxa por tela vêm dos Ajustes (`mode:<conector>` = "LxA@Hz"); escala da principal
//!   pode ser fracionária (`scale`), as outras usam escala automática pela densidade.
//! - VRR só com jogo em tela cheia; Night Shift por rampa de gamma; presentation-time para os apps.
//!
//! Modelado no `anvil` do Smithay. Uma GPU (a primária).

use std::{cell::RefCell, collections::HashMap, rc::Rc, time::Duration};

use smithay::{
    backend::{
        allocator::{
            gbm::{GbmAllocator, GbmBufferFlags, GbmDevice},
            Fourcc,
        },
        drm::{
            compositor::FrameFlags,
            exporter::gbm::GbmFramebufferExporter,
            output::{DrmOutput, DrmOutputManager, DrmOutputRenderElements},
            DrmDevice, DrmDeviceFd, DrmEvent, DrmNode, NodeType,
        },
        egl::{EGLContext, EGLDisplay},
        libinput::{LibinputInputBackend, LibinputSessionInterface},
        renderer::gles::GlesRenderer,
        session::{libseat::LibSeatSession, Event as SessionEvent, Session},
        udev::{primary_gpu, UdevBackend, UdevEvent},
    },
    desktop::utils::{surface_presentation_feedback_flags_from_states, OutputPresentationFeedback},
    output::{Mode as WlMode, Output, PhysicalProperties, Scale, Subpixel},
    reexports::{
        calloop::{
            timer::{TimeoutAction, Timer},
            EventLoop,
        },
        drm::control::{self, connector, crtc, Device as ControlDevice, ModeTypeFlags},
        input::Libinput,
        rustix::fs::OFlags,
        wayland_protocols::wp::presentation_time::server::wp_presentation_feedback,
        wayland_server::backend::GlobalId,
    },
    utils::{DeviceFd, Rectangle, Transform},
    wayland::presentation::Refresh,
};

use crate::render::ObeliskElement;
use crate::state::Obelisk;

const FORMATS: &[Fourcc] = &[Fourcc::Abgr8888, Fourcc::Argb8888];

type Feedback = Option<OutputPresentationFeedback>;
type Out = DrmOutput<GbmAllocator<DrmDeviceFd>, GbmFramebufferExporter<DrmDeviceFd>, Feedback, DrmDeviceFd>;
type Manager = DrmOutputManager<GbmAllocator<DrmDeviceFd>, GbmFramebufferExporter<DrmDeviceFd>, Feedback, DrmDeviceFd>;

/// Uma tela ligada.
struct Surface {
    out: Out,
    output: Output,
    global: GlobalId,
    connector: connector::Handle,
    name: String,
    modes: Vec<control::Mode>,
    mode: control::Mode,
    scale: f64,
    vrr: bool,
    night: Option<bool>,
    pending: bool,
    /// precisa redesenhar (o `dirty` global é repassado a todas as telas)
    needs: bool,
    /// já há um timer de "tentar de novo" agendado
    retry: bool,
}

struct Drm {
    renderer: GlesRenderer,
    manager: Manager,
    libinput: Libinput,
    surfaces: HashMap<crtc::Handle, Surface>,
    /// ordem das telas (a primeira é a principal)
    order: Vec<crtc::Handle>,
    active: bool,
}

fn mode_string(m: &control::Mode) -> String {
    format!("{}x{}@{}", m.size().0, m.size().1, m.vrefresh())
}

/// Escolhe o modo: o salvo nos Ajustes ("LxA@Hz"), senão o preferido, senão o maior.
fn pick_mode(conn: &connector::Info, wanted: Option<&str>) -> control::Mode {
    if let Some(w) = wanted {
        if let Some(m) = conn.modes().iter().find(|m| mode_string(m) == w) {
            return *m;
        }
    }
    conn.modes()
        .iter()
        .find(|m| m.mode_type().contains(ModeTypeFlags::PREFERRED))
        .copied()
        .unwrap_or_else(|| *conn.modes().iter().max_by_key(|m| (m.size().0 as u32 * m.size().1 as u32, m.vrefresh())).unwrap())
}

/// Escala automática pela densidade (≥ ~170 dpi → 2; ≥ ~135 dpi → 1,5).
fn auto_scale(mode: &control::Mode, size_mm: (u32, u32)) -> f64 {
    if size_mm.1 == 0 {
        return 1.0;
    }
    let dpi = mode.size().1 as f64 / (size_mm.1 as f64 / 25.4);
    if dpi > 170.0 {
        2.0
    } else if dpi > 135.0 {
        1.5
    } else {
        1.0
    }
}

fn output_scale(f: f64) -> Scale {
    if f.fract() == 0.0 {
        Scale::Integer(f as i32)
    } else {
        Scale::Fractional(f)
    }
}

pub fn init(event_loop: &mut EventLoop<'static, Obelisk>, state: &mut Obelisk) -> Result<(), Box<dyn std::error::Error>> {
    let (session, notifier) = LibSeatSession::new().map_err(|e| format!("libseat: {e:?}"))?;
    let seat = session.seat();

    // ---- entrada (libinput)
    let mut libinput = Libinput::new_with_udev::<LibinputSessionInterface<LibSeatSession>>(session.clone().into());
    libinput.udev_assign_seat(&seat).map_err(|_| "libinput: falha ao atribuir o seat")?;
    event_loop
        .handle()
        .insert_source(LibinputInputBackend::new(libinput.clone()), |event, _, state| {
            state.dirty = true;
            if let smithay::backend::input::InputEvent::DeviceAdded { device } = &event {
                state.input_devices.push(device.clone());
                state.apply_pointer_settings();
            }
            state.process_input_event(event)
        })
        .map_err(|e| format!("libinput source: {e}"))?;

    // ---- GPU
    let path = primary_gpu(&seat)?.ok_or("nenhuma GPU encontrada")?;
    let node = DrmNode::from_path(&path)?;
    let render_node = node.node_with_type(NodeType::Render).and_then(|r| r.ok()).unwrap_or(node);
    tracing::info!("GPU: {path:?}");

    let fd = session
        .clone()
        .open(&path, OFlags::RDWR | OFlags::CLOEXEC | OFlags::NOCTTY | OFlags::NONBLOCK)
        .map_err(|e| format!("abrir DRM: {e:?}"))?;
    let fd = DrmDeviceFd::new(DeviceFd::from(fd));
    let (drm, drm_notifier) = DrmDevice::new(fd.clone(), true)?;
    let gbm: GbmDevice<DrmDeviceFd> = GbmDevice::new(fd)?;

    // ---- renderer GLES sobre GBM
    let egl = unsafe { EGLDisplay::new(gbm.clone())? };
    let ctx = EGLContext::new(&egl)?;
    let renderer = unsafe { GlesRenderer::new(ctx)? };
    state.shm_state.update_formats(smithay::backend::renderer::ImportMemWl::shm_formats(&renderer));

    let allocator = GbmAllocator::new(gbm.clone(), GbmBufferFlags::RENDERING | GbmBufferFlags::SCANOUT);
    let exporter = GbmFramebufferExporter::new(gbm.clone(), render_node.into());
    let render_formats = smithay::backend::renderer::ImportDma::dmabuf_formats(&renderer);
    state.dmabuf_state.create_global::<Obelisk>(&state.display_handle, render_formats.clone());
    let manager: Manager = DrmOutputManager::new(drm, allocator, exporter, Some(gbm), FORMATS.iter().copied(), render_formats);

    let drm_state = Rc::new(RefCell::new(Drm { renderer, manager, libinput, surfaces: HashMap::new(), order: Vec::new(), active: true }));
    scan_connectors(&drm_state, state);
    if drm_state.borrow().surfaces.is_empty() {
        return Err("nenhuma tela conectada".into());
    }

    // ---- vblank → feedback de apresentação e próximo quadro daquela tela
    {
        let st = drm_state.clone();
        event_loop
            .handle()
            .insert_source(drm_notifier, move |event, meta, state| match event {
                DrmEvent::VBlank(crtc) => {
                    {
                        let mut d = st.borrow_mut();
                        if let Some(s) = d.surfaces.get_mut(&crtc) {
                            s.pending = false;
                            let refresh_ns = 1_000_000_000_000u64 / (s.mode.vrefresh().max(1) as u64 * 1000);
                            if let Ok(Some(Some(mut fb))) = s.out.frame_submitted() {
                                let (time, seq) = match meta {
                                    Some(m) => {
                                        let t = match m.time {
                                            smithay::backend::drm::DrmEventTime::Monotonic(t) => t,
                                            smithay::backend::drm::DrmEventTime::Realtime(_) => state.clock.now().into(),
                                        };
                                        (t, m.sequence as u64)
                                    }
                                    None => (state.clock.now().into(), 0),
                                };
                                let flags = wp_presentation_feedback::Kind::Vsync | wp_presentation_feedback::Kind::HwClock | wp_presentation_feedback::Kind::HwCompletion;
                                fb.presented::<Duration, smithay::utils::Monotonic>(time, Refresh::fixed(Duration::from_nanos(refresh_ns)), seq, flags);
                            }
                        }
                    }
                    render(&st, crtc, state);
                }
                DrmEvent::Error(e) => tracing::error!("drm: {e:?}"),
            })
            .map_err(|e| format!("drm source: {e}"))?;
    }

    // ---- plugar/desplugar monitor
    {
        let st = drm_state.clone();
        let dev_id = node.dev_id();
        let udev = UdevBackend::new(&seat)?;
        event_loop
            .handle()
            .insert_source(udev, move |event, _, state| {
                if let UdevEvent::Changed { device_id } = event {
                    if device_id == dev_id {
                        tracing::info!("telas mudaram: relendo conectores");
                        scan_connectors(&st, state);
                    }
                }
            })
            .map_err(|e| format!("udev source: {e}"))?;
    }

    // ---- troca de VT / suspensão
    {
        let st = drm_state.clone();
        event_loop
            .handle()
            .insert_source(notifier, move |event, _, state| match event {
                SessionEvent::PauseSession => {
                    let mut d = st.borrow_mut();
                    d.active = false;
                    d.libinput.suspend();
                    d.manager.pause();
                }
                SessionEvent::ActivateSession => {
                    let crtcs: Vec<crtc::Handle> = {
                        let mut d = st.borrow_mut();
                        d.active = true;
                        let _ = d.libinput.resume();
                        let _ = d.manager.activate(false);
                        for s in d.surfaces.values_mut() {
                            s.pending = false;
                            s.needs = true;
                            s.night = None;
                        }
                        d.order.clone()
                    };
                    // ao voltar (ex.: depois de suspender) os monitores podem ter mudado
                    scan_connectors(&st, state);
                    for c in crtcs {
                        render(&st, c, state);
                    }
                }
            })
            .map_err(|e| format!("session source: {e}"))?;
    }

    // ---- primeiro quadro de cada tela
    let st = drm_state.clone();
    event_loop.handle().insert_idle(move |state| {
        let crtcs = st.borrow().order.clone();
        for c in crtcs {
            render(&st, c, state);
        }
    });
    Ok(())
}

/// Lê os conectores: liga telas novas, desliga as removidas e reposiciona tudo.
fn scan_connectors(st: &Rc<RefCell<Drm>>, state: &mut Obelisk) {
    let mut d = st.borrow_mut();
    let d = &mut *d;
    let Ok(res) = d.manager.device().resource_handles() else { return };
    let connected: Vec<connector::Info> = res
        .connectors()
        .iter()
        .filter_map(|h| d.manager.device().get_connector(*h, true).ok())
        .filter(|i| i.state() == connector::State::Connected && !i.modes().is_empty())
        .collect();

    // removidas
    let gone: Vec<crtc::Handle> = d.surfaces.iter().filter(|(_, s)| !connected.iter().any(|c| c.handle() == s.connector)).map(|(c, _)| *c).collect();
    for c in gone {
        if let Some(s) = d.surfaces.remove(&c) {
            tracing::info!("tela removida: {}", s.name);
            state.space.unmap_output(&s.output);
            state.display_handle.remove_global::<Obelisk>(s.global);
        }
        d.order.retain(|x| *x != c);
    }

    // novas
    for conn in &connected {
        if d.surfaces.values().any(|s| s.connector == conn.handle()) {
            continue;
        }
        let used: Vec<crtc::Handle> = d.surfaces.keys().copied().collect();
        let crtc = conn
            .encoders()
            .iter()
            .filter_map(|e| d.manager.device().get_encoder(*e).ok())
            .flat_map(|enc| res.filter_crtcs(enc.possible_crtcs()))
            .find(|c| !used.contains(c));
        let Some(crtc) = crtc else {
            tracing::warn!("sem CRTC livre para {:?}", conn.interface());
            continue;
        };
        let name = format!("{}-{}", conn.interface().as_str(), conn.interface_id());
        let mode = pick_mode(conn, state.settings.get(&format!("mode:{name}")));
        let (pw, ph) = conn.size().unwrap_or((0, 0));
        let output = Output::new(
            name.clone(),
            PhysicalProperties { size: (pw as i32, ph as i32).into(), subpixel: Subpixel::Unknown, make: "Obelisk".into(), model: name.clone() },
        );
        let global = output.create_global::<Obelisk>(&state.display_handle);
        let wl_mode = WlMode::from(mode);
        let primary = d.order.is_empty();
        let scale = if primary {
            state.settings.get("scale").and_then(|v| v.parse::<f64>().ok()).unwrap_or_else(|| auto_scale(&mode, (pw, ph)))
        } else {
            auto_scale(&mode, (pw, ph))
        };
        output.change_current_state(Some(wl_mode), Some(Transform::Normal), Some(output_scale(scale)), None);
        output.set_preferred(WlMode::from(pick_mode(conn, None)));
        let out = match d.manager.initialize_output::<_, ObeliskElement>(
            crtc,
            mode,
            &[conn.handle()],
            &output,
            None,
            &mut d.renderer,
            &DrmOutputRenderElements::default(),
        ) {
            Ok(o) => o,
            Err(e) => {
                tracing::warn!("não deu para ligar {name}: {e:?}");
                state.display_handle.remove_global::<Obelisk>(global);
                continue;
            }
        };
        tracing::info!("tela ligada: {name} {} escala {scale}", mode_string(&mode));
        d.surfaces.insert(
            crtc,
            Surface {
                out,
                output,
                global,
                connector: conn.handle(),
                name,
                modes: conn.modes().to_vec(),
                mode,
                scale,
                vrr: false,
                night: None,
                pending: false,
                needs: true,
                retry: false,
            },
        );
        d.order.push(crtc);
    }
    layout(d, state);
}

/// Posiciona as telas como o usuário arrumou nos Ajustes (`pos:<tela>` relativo à principal,
/// `primary`, `mirror`); telas novas vão à direita. A principal fica sempre em (0,0).
fn layout(d: &mut Drm, state: &mut Obelisk) {
    // a principal escolhida vai para o começo
    if let Some(p) = state.settings.get("primary").map(str::to_string) {
        if let Some(i) = d.order.iter().position(|c| d.surfaces.get(c).is_some_and(|s| s.name == p)) {
            let c = d.order.remove(i);
            d.order.insert(0, c);
        }
    }
    let mirror = state.settings.get("mirror") == Some("true") && d.order.len() > 1;
    state.mirror = mirror;
    for s in d.surfaces.values() {
        state.space.unmap_output(&s.output);
    }
    let mut placed: Vec<Rectangle<i32, smithay::utils::Logical>> = Vec::new();
    state.displays.clear();
    for (i, c) in d.order.iter().enumerate() {
        let Some(s) = d.surfaces.get(c) else { continue };
        let logical = WlMode::from(s.mode).size.to_f64().to_logical(s.scale).to_i32_round::<i32>();
        if i == 0 {
            state.output = Some(s.output.clone());
            state.fscale = s.scale;
            state.scale = s.scale.ceil() as i32;
            state.screen = logical;
        } else if mirror {
            // espelhada: não entra no espaço; mostra a principal reduzida/ampliada
            state.displays.push(crate::state::DisplayInfo { name: s.name.clone(), rect: Rectangle::from_size(logical), primary: false });
            continue;
        }
        let saved = state.settings.get(&format!("pos:{}", s.name)).and_then(|v| {
            let (x, y) = v.split_once(',')?;
            Some((x.trim().parse::<i32>().ok()?, y.trim().parse::<i32>().ok()?))
        });
        let mut loc = if i == 0 { (0, 0) } else { saved.unwrap_or((i32::MIN, 0)) };
        let mut r = Rectangle::new(loc.into(), logical);
        if i > 0 && (loc.0 == i32::MIN || placed.iter().any(|p| p.overlaps(r))) {
            // sem posição salva (ou sobrepondo): à direita de tudo
            let right = placed.iter().map(|p| p.loc.x + p.size.w).max().unwrap_or(0);
            loc = (right, 0);
            r = Rectangle::new(loc.into(), logical);
        }
        s.output.change_current_state(None, None, None, Some(loc.into()));
        state.space.map_output(&s.output, loc);
        placed.push(r);
        state.displays.push(crate::state::DisplayInfo { name: s.name.clone(), rect: r, primary: i == 0 });
    }
    // janelas que ficaram fora de todas as telas voltam para a principal
    let bounds = state.desktop_bounds();
    let lost: Vec<_> = state.space.elements().filter(|w| state.space.element_location(w).is_some_and(|l| !bounds.contains(l))).cloned().collect();
    for w in lost {
        state.space.map_element(w, (120, 120), false);
    }
    state.output_modes = d
        .order
        .iter()
        .filter_map(|c| d.surfaces.get(c))
        .map(|s| {
            let mut modes: Vec<String> = s.modes.iter().map(mode_string).collect();
            modes.dedup();
            (s.name.clone(), modes, mode_string(&s.mode), s.scale)
        })
        .collect();
    state.shell.cursor = Default::default();
    state.shell.cursor_layers.clear();
    state.shell.decos.clear();
    state.dirty = true;
}

/// Mudanças pedidas pelos Ajustes (resolução/taxa, escala da principal).
fn apply_requests(d: &mut Drm, state: &mut Obelisk) {
    if !std::mem::take(&mut state.outputs_changed) {
        return;
    }
    let mut changed = false;
    let order = d.order.clone();
    for (i, c) in order.iter().enumerate() {
        let Some(s) = d.surfaces.get_mut(c) else { continue };
        if let Some(want) = state.settings.get(&format!("mode:{}", s.name)) {
            if let Some(m) = s.modes.iter().find(|m| mode_string(m) == want).copied() {
                if m != s.mode {
                    match s.out.use_mode::<_, ObeliskElement>(m, &mut d.renderer, &DrmOutputRenderElements::default()) {
                        Ok(()) => {
                            tracing::info!("{}: modo {}", s.name, mode_string(&m));
                            s.mode = m;
                            s.output.change_current_state(Some(WlMode::from(m)), None, None, None);
                            changed = true;
                        }
                        Err(e) => tracing::warn!("{}: modo {want} recusado: {e:?}", s.name),
                    }
                }
            }
        }
        if i == 0 && (state.fscale - s.scale).abs() > 0.001 {
            s.scale = state.fscale;
            s.output.change_current_state(None, None, Some(output_scale(s.scale)), None);
            changed = true;
        }
    }
    let _ = changed;
    // posição, tela principal ou espelho também mudam o arranjo
    layout(d, state);
}

/// Renderiza e enfileira um quadro da tela `crtc`. Sem mudanças, reagenda em ~16 ms.
fn render(st: &Rc<RefCell<Drm>>, crtc: crtc::Handle, state: &mut Obelisk) {
    let mut guard = st.borrow_mut();
    let d = &mut *guard;
    if !d.active {
        return;
    }
    let primary = d.order.first() == Some(&crtc);
    if primary {
        state.tick();
        apply_requests(d, state);
    }
    // o "sujo" global vale para todas as telas
    if state.dirty {
        for s in d.surfaces.values_mut() {
            s.needs = true;
        }
        state.dirty = false;
    }
    let animating = state.animating();
    let Some(s) = d.surfaces.get_mut(&crtc) else { return };
    s.retry = false;
    if s.pending {
        return;
    }
    if !s.needs && !animating {
        schedule_retry(st, crtc, s, state);
        return;
    }
    s.needs = false;

    if primary {
        // VRR só com jogo em tela cheia
        let want_vrr = state.want_vrr();
        if want_vrr != s.vrr {
            match s.out.with_compositor(|c| c.use_vrr(want_vrr)) {
                Ok(()) => tracing::info!("VRR {}", if want_vrr { "ligado" } else { "desligado" }),
                Err(e) => tracing::debug!("VRR indisponível: {e:?}"),
            }
            s.vrr = want_vrr;
        }
    }
    // Night Shift: rampa de gamma quente (~4000 K) ou neutra, em cada tela
    if s.night != Some(state.night_shift) {
        s.night = Some(state.night_shift);
        let len = d.manager.device().get_crtc(crtc).map(|i| i.gamma_length() as usize).unwrap_or(0);
        if len > 1 {
            let (mr, mg, mb) = if state.night_shift { (1.0, 0.80, 0.58) } else { (1.0, 1.0, 1.0) };
            let ramp = |m: f64| (0..len).map(|i| ((i as f64 / (len - 1) as f64) * m * 65535.0) as u16).collect::<Vec<u16>>();
            if let Err(e) = d.manager.device().set_gamma(crtc, &ramp(mr), &ramp(mg), &ramp(mb)) {
                tracing::warn!("Night Shift: {e}");
            }
        }
    }
    let s = d.surfaces.get_mut(&crtc).unwrap();
    let elements = if primary {
        state.build_elements(&mut d.renderer)
    } else if state.mirror {
        // espelho: a cena da principal na escala que cabe nesta tela
        let h = WlMode::from(s.mode).size.h as f64;
        state.render_scale = Some(h / state.screen.h.max(1) as f64);
        let e = state.build_elements(&mut d.renderer);
        state.render_scale = None;
        e
    } else {
        let geo = state.space.output_geometry(&s.output).unwrap_or_else(|| Rectangle::from_size(state.screen));
        state.build_elements_secondary(&mut d.renderer, geo, s.scale)
    };
    let clear = if state.shell.theme.appearance == crate::ui::theme::Appearance::Dark { [0.04, 0.05, 0.12, 1.0] } else { [0.9, 0.93, 1.0, 1.0] };
    let output = s.output.clone();
    let mut feedback: Feedback = None;
    let rendered = match s.out.render_frame(&mut d.renderer, &elements, clear, FrameFlags::DEFAULT) {
        Ok(r) => {
            if !r.is_empty {
                let mut fb = OutputPresentationFeedback::new(&output);
                for w in state.space.elements() {
                    w.take_presentation_feedback(&mut fb, |_, _| Some(output.clone()), |surface, _| surface_presentation_feedback_flags_from_states(surface, &r.states));
                }
                feedback = Some(fb);
            }
            !r.is_empty
        }
        Err(e) => {
            tracing::warn!("render_frame: {e:?}");
            false
        }
    };
    // capturas pedidas por apps (wlr-screencopy) e gravação: lidas desta tela
    if primary {
        let size = WlMode::from(s.mode).size;
        state.after_render(&mut d.renderer, &elements, size, s.scale, &output);
    }
    drop(elements);

    if rendered {
        match s.out.queue_frame(feedback) {
            Ok(()) => s.pending = true,
            Err(e) => tracing::warn!("queue_frame: {e:?}"),
        }
    }

    // frame callbacks só para as janelas desta tela
    let t = state.start_time.elapsed();
    let geo = state.space.output_geometry(&output);
    for w in state.space.elements() {
        let on_it = match (geo, state.space.element_bbox(w)) {
            (Some(g), Some(b)) => g.overlaps(b),
            _ => primary,
        };
        if on_it {
            w.send_frame(&output, t, Some(Duration::ZERO), |_, _| Some(output.clone()));
        }
    }
    let _ = state.display_handle.flush_clients();

    let s = d.surfaces.get_mut(&crtc).unwrap();
    if !s.pending {
        schedule_retry(st, crtc, s, state);
    }
}

fn schedule_retry(st: &Rc<RefCell<Drm>>, crtc: crtc::Handle, s: &mut Surface, state: &mut Obelisk) {
    if s.retry {
        return;
    }
    s.retry = true;
    let st2 = st.clone();
    let _ = state.loop_handle.insert_source(Timer::from_duration(Duration::from_millis(16)), move |_, _, state| {
        render(&st2, crtc, state);
        TimeoutAction::Drop
    });
}
