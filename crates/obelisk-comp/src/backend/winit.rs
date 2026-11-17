//! Backend aninhado: roda o Obelisk como uma janela dentro de outro compositor/X11.
//! É o modo de desenvolvimento (não toca na sua sessão atual).

use std::{cell::RefCell, rc::Rc, time::Duration};

use smithay::{
    backend::{
        renderer::{damage::OutputDamageTracker, gles::GlesRenderer},
        winit::{self, WinitEvent},
    },
    output::{Mode, Output, PhysicalProperties, Scale, Subpixel},
    reexports::calloop::EventLoop,
    utils::Transform,
};

use crate::state::Obelisk;

pub fn init(event_loop: &mut EventLoop<'static, Obelisk>, state: &mut Obelisk) -> Result<(), Box<dyn std::error::Error>> {
    let (mut backend, winit_loop) = winit::init::<GlesRenderer>().map_err(|e| format!("winit: {e:?}"))?;
    backend.window().set_title("Obelisk (modo aninhado)");
    backend.window().set_cursor_visible(false);

    let scale = state.scale;
    // OBELISK_VIRTUAL_SIZE=LxA: tela de tamanho fixo, independente da janela (capturas do README)
    let virtual_size = std::env::var("OBELISK_VIRTUAL_SIZE").ok().and_then(|v| {
        let (w, h) = v.split_once('x')?;
        Some(smithay::utils::Size::<i32, smithay::utils::Physical>::from((w.parse().ok()?, h.parse().ok()?)))
    });
    let mode = Mode { size: virtual_size.unwrap_or_else(|| backend.window_size()), refresh: 60_000 };
    let output = Output::new(
        "obelisk-winit".to_string(),
        PhysicalProperties { size: (0, 0).into(), subpixel: Subpixel::Unknown, make: "Obelisk".into(), model: "Aninhado".into() },
    );
    let _global = output.create_global::<Obelisk>(&state.display_handle);
    output.change_current_state(Some(mode), Some(Transform::Flipped180), Some(Scale::Integer(scale)), Some((0, 0).into()));
    output.set_preferred(mode);
    state.space.map_output(&output, (0, 0));
    state.screen = mode.size.to_logical(scale);
    state.output = Some(output.clone());

    let mut damage_tracker = OutputDamageTracker::from_output(&output);

    // linux-dmabuf: apps OpenGL/Vulkan (jogos) entregam buffers de GPU direto
    {
        use smithay::backend::renderer::ImportDma;
        let formats = backend.renderer().dmabuf_formats();
        state.dmabuf_state.create_global::<Obelisk>(&state.display_handle, formats);
    }
    let backend = Rc::new(RefCell::new(backend));

    // Parado, nenhum quadro é pedido: este timer (16 ms) só acorda o redraw quando algo mudou,
    // e relê o estado do sistema 4×/s. Sem ele, pular quadros viraria um laço ocupado.
    {
        let backend = backend.clone();
        let mut n: u32 = 0;
        event_loop.handle().insert_source(
            smithay::reexports::calloop::timer::Timer::from_duration(Duration::from_millis(16)),
            move |_, _, state| {
                n = n.wrapping_add(1);
                if n % 15 == 0 {
                    state.tick();
                }
                if state.dirty || state.animating() {
                    backend.borrow().window().request_redraw();
                }
                smithay::reexports::calloop::timer::TimeoutAction::ToDuration(Duration::from_millis(16))
            },
        )?;
    }

    event_loop.handle().insert_source(winit_loop, move |event, _, state| {
        let mut backend = backend.borrow_mut();
        match event {
        WinitEvent::Resized { size, .. } if virtual_size.is_none() => {
            output.change_current_state(Some(Mode { size, refresh: 60_000 }), None, None, None);
            state.screen = size.to_f64().to_logical(state.fscale).to_i32_round();
            state.dirty = true;
        }
        WinitEvent::Input(event) => {
            state.dirty = true;
            state.process_input_event(event);
            backend.window().request_redraw();
        }
        WinitEvent::Redraw => {
            // o tamanho real da janela pode mudar sem um evento de resize (re-tile do host)
            let size = virtual_size.unwrap_or_else(|| backend.window_size());
            if size != output.current_mode().map(|m| m.size).unwrap_or(size) {
                output.change_current_state(Some(Mode { size, refresh: 60_000 }), None, None, None);
                state.screen = size.to_f64().to_logical(state.fscale).to_i32_round();
                state.dirty = true;
            }
            state.tick();
            if state.recorder.is_some() {
                tracing::debug!("redraw durante gravação");
            }
            // nada mudou e nada anima: não monta a cena e não pede outro quadro
            if !state.dirty && !state.animating() {
                return;
            }
            state.dirty = false;
            let (damage, elements, shot) = {
                let age = backend.buffer_age().unwrap_or(0);
                let (renderer, mut framebuffer) = backend.bind().expect("bind");
                let elements = state.build_elements(renderer);
                // testes: captura depois de N quadros
                if let Some((n, path, quit)) = state.debug_capture.take() {
                    if n == 0 {
                        state.screenshot = Some(path);
                        if quit {
                            state.extra_exit = true;
                        }
                    } else {
                        state.debug_capture = Some((n - 1, path, quit));
                    }
                }
                let clear = if state.shell.theme.appearance == crate::ui::theme::Appearance::Dark { [0.04, 0.05, 0.12, 1.0] } else { [0.9, 0.93, 1.0, 1.0] };
                let shot = state.screenshot.take();
                // com captura pendente, redesenha tudo (age 0) para o framebuffer estar completo
                let age = if shot.is_some() || state.recorder.is_some() { 0 } else { age };
                let damage = match damage_tracker.render_output(renderer, &mut framebuffer, age, &elements, clear) {
                    Ok(r) => r.damage.cloned(),
                    Err(e) => {
                        tracing::error!("falha ao renderizar: {e:?}");
                        None
                    }
                };
                (damage, elements, shot)
            };
            if let Err(e) = backend.submit(damage.as_deref()) {
                tracing::error!("falha no submit: {e:?}");
            }

            // captura e gravação: DEPOIS de apresentar, renderiza de novo numa textura fora da tela
            // e lê dela (ler/religar a superfície da janela antes do submit quebra a EGLSurface)
            if state.recorder.as_ref().is_some_and(|r| r.due()) || shot.is_some() || !state.screencopy.is_empty() {
                state.screenshot = shot;
                let size = output.current_mode().map(|m| m.size).unwrap_or_default();
                let fsc = state.fscale;
                state.after_render(backend.renderer(), &elements, size, fsc, &output);
            }
            drop(elements);

            let t = state.start_time.elapsed();
            for w in state.space.elements() {
                w.send_frame(&output, t, Some(Duration::ZERO), |_, _| Some(output.clone()));
            }
            let _ = state.display_handle.flush_clients();
            if state.dirty || state.animating() {
                backend.window().request_redraw();
            }
        }
        WinitEvent::CloseRequested => state.loop_signal.stop(),
        _ => {}
        }
    })?;
    Ok(())
}
