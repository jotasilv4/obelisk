//! wlr-screencopy (v3): apps leem a tela. É o que o xdg-desktop-portal-wlr usa para o
//! compartilhamento de tela (Discord, OBS, Meet…) e o que o `grim` usa para capturas.
//!
//! O app pede um quadro, recebe o formato (XRGB8888 em memória compartilhada) e manda o buffer;
//! no próximo quadro desenhado a cena é renderizada fora da tela e copiada para ele.
//! Apps em sandbox (security-context, ex.: Flatpak) não veem este global.

use std::sync::Mutex;

use smithay::{
    output::Output,
    reexports::wayland_server::{
        protocol::{wl_buffer::WlBuffer, wl_shm},
        Client, DataInit, Dispatch, DisplayHandle, GlobalDispatch, New, Resource,
    },
    utils::{Physical, Rectangle},
};
use wayland_protocols_wlr::screencopy::v1::server::{
    zwlr_screencopy_frame_v1::{self, ZwlrScreencopyFrameV1},
    zwlr_screencopy_manager_v1::{self, ZwlrScreencopyManagerV1},
};

use crate::state::{ClientState, Obelisk};

pub struct FrameData {
    pub output: Output,
    /// região pedida, em px físicos da tela
    pub region: Rectangle<i32, Physical>,
    pub copied: Mutex<bool>,
}

/// Cópia pendente: feita depois do próximo quadro desenhado.
pub struct PendingCopy {
    pub frame: ZwlrScreencopyFrameV1,
    pub buffer: WlBuffer,
    pub with_damage: bool,
}

pub fn init(dh: &DisplayHandle) {
    dh.create_global::<Obelisk, ZwlrScreencopyManagerV1, _>(3, ());
}

impl GlobalDispatch<ZwlrScreencopyManagerV1, ()> for Obelisk {
    fn bind(_: &mut Self, _: &DisplayHandle, _: &Client, resource: New<ZwlrScreencopyManagerV1>, _: &(), data_init: &mut DataInit<'_, Self>) {
        data_init.init(resource, ());
    }

    /// Apps em sandbox não podem ler a tela.
    fn can_view(client: Client, _: &()) -> bool {
        client.get_data::<ClientState>().is_none_or(|c| c.security_context.is_none())
    }
}

impl Dispatch<ZwlrScreencopyManagerV1, ()> for Obelisk {
    fn request(
        state: &mut Self,
        _: &Client,
        _: &ZwlrScreencopyManagerV1,
        request: zwlr_screencopy_manager_v1::Request,
        _: &(),
        _: &DisplayHandle,
        data_init: &mut DataInit<'_, Self>,
    ) {
        let (frame, output, region) = match request {
            zwlr_screencopy_manager_v1::Request::CaptureOutput { frame, overlay_cursor: _, output } => (frame, output, None),
            zwlr_screencopy_manager_v1::Request::CaptureOutputRegion { frame, overlay_cursor: _, output, x, y, width, height } => {
                (frame, output, Some((x, y, width, height)))
            }
            _ => return,
        };
        let Some(output) = Output::from_resource(&output) else {
            let f = data_init.init(frame, FrameData { output: state.output.clone().unwrap(), region: Rectangle::default(), copied: Mutex::new(true) });
            f.failed();
            return;
        };
        let mode = output.current_mode().map(|m| m.size).unwrap_or_default();
        let scale = output.current_scale().fractional_scale();
        let full = Rectangle::<i32, Physical>::from_size(mode);
        let region = match region {
            Some((x, y, w, h)) => {
                let r = Rectangle::<i32, Physical>::new(
                    ((x as f64 * scale).round() as i32, (y as f64 * scale).round() as i32).into(),
                    ((w as f64 * scale).round() as i32, (h as f64 * scale).round() as i32).into(),
                );
                r.intersection(full).unwrap_or(full)
            }
            None => full,
        };
        let f = data_init.init(frame, FrameData { output, region, copied: Mutex::new(false) });
        let (w, h) = (region.size.w as u32, region.size.h as u32);
        f.buffer(wl_shm::Format::Xrgb8888, w, h, w * 4);
        if f.version() >= 3 {
            f.buffer_done();
        }
    }
}

impl Dispatch<ZwlrScreencopyFrameV1, FrameData> for Obelisk {
    fn request(
        state: &mut Self,
        _: &Client,
        frame: &ZwlrScreencopyFrameV1,
        request: zwlr_screencopy_frame_v1::Request,
        data: &FrameData,
        _: &DisplayHandle,
        _: &mut DataInit<'_, Self>,
    ) {
        let (buffer, with_damage) = match request {
            zwlr_screencopy_frame_v1::Request::Copy { buffer } => (buffer, false),
            zwlr_screencopy_frame_v1::Request::CopyWithDamage { buffer } => (buffer, true),
            _ => return,
        };
        let mut copied = data.copied.lock().unwrap();
        if *copied {
            frame.post_error(zwlr_screencopy_frame_v1::Error::AlreadyUsed, "quadro já copiado");
            return;
        }
        *copied = true;
        state.screencopy.push(PendingCopy { frame: frame.clone(), buffer, with_damage });
        // garante um quadro novo para atender o pedido
        state.dirty = true;
    }
}

/// Atende os pedidos desta tela com a imagem já renderizada (RGBA da região inteira da tela).
pub fn serve(state: &mut Obelisk, output: &Output, read: &mut dyn FnMut(Rectangle<i32, Physical>) -> Option<Vec<u8>>) {
    if state.screencopy.is_empty() {
        return;
    }
    let pending = std::mem::take(&mut state.screencopy);
    for p in pending {
        let Some(data) = p.frame.data::<FrameData>() else { continue };
        if &data.output != output {
            state.screencopy.push(p);
            continue;
        }
        let region = data.region;
        // o buffer segue a orientação da tela (o app aplica a transformação do wl_output);
        // no modo aninhado a tela é "Flipped180": lê a região espelhada e inverte as linhas
        let flipped = output.current_transform() == smithay::utils::Transform::Flipped180;
        let mode = output.current_mode().map(|m| m.size).unwrap_or_default();
        let src = if flipped {
            Rectangle::new((region.loc.x, mode.h - region.loc.y - region.size.h).into(), region.size)
        } else {
            region
        };
        let Some(mut rgba) = read(src) else {
            p.frame.failed();
            continue;
        };
        if flipped {
            let row = region.size.w as usize * 4;
            let h = region.size.h as usize;
            for y in 0..h / 2 {
                let (top, bottom) = rgba.split_at_mut((h - 1 - y) * row);
                top[y * row..(y + 1) * row].swap_with_slice(&mut bottom[..row]);
            }
        }
        let ok = smithay::wayland::shm::with_buffer_contents_mut(&p.buffer, |ptr, len, info| {
            if info.width != region.size.w || info.height != region.size.h || info.format != wl_shm::Format::Xrgb8888 && info.format != wl_shm::Format::Argb8888 {
                return false;
            }
            let dst = unsafe { std::slice::from_raw_parts_mut(ptr.add(info.offset as usize), len - info.offset as usize) };
            let w = region.size.w as usize;
            for y in 0..region.size.h as usize {
                let row = &rgba[y * w * 4..(y + 1) * w * 4];
                let out = &mut dst[y * info.stride as usize..y * info.stride as usize + w * 4];
                for (s, d) in row.chunks_exact(4).zip(out.chunks_exact_mut(4)) {
                    // RGBA → BGRX (XRGB8888 em little-endian)
                    d[0] = s[2];
                    d[1] = s[1];
                    d[2] = s[0];
                    d[3] = 255;
                }
            }
            true
        });
        if !matches!(ok, Ok(true)) {
            p.frame.failed();
            continue;
        }
        p.frame.flags(zwlr_screencopy_frame_v1::Flags::empty());
        if p.with_damage {
            p.frame.damage(0, 0, region.size.w as u32, region.size.h as u32);
        }
        let d: std::time::Duration = state.clock.now().into();
        p.frame.ready((d.as_secs() >> 32) as u32, d.as_secs() as u32, d.subsec_nanos());
    }
}
