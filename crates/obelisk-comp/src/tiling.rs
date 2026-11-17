//! Encaixe de janelas (macOS Sequoia / kyro): arrastar até a borda ocupa metade, canto ocupa um
//! quarto, topo maximiza; ⌘⌥ + setas, ⌘⌥ 1–4 (quartos) e ⌘⌥ ⌫ (restaura).

use smithay::{
    desktop::Window,
    reexports::wayland_protocols::xdg::shell::server::xdg_toplevel,
    utils::{Logical, Point, Rectangle},
};

use crate::state::{meta, Obelisk};
use crate::ui::theme::metrics;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Tile {
    Left,
    Right,
    Top,
    Bottom,
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
    Fill,
}

const EDGE: f64 = 3.0;

impl Obelisk {
    /// Zona de encaixe para o ponteiro em `p` (fim de arrasto).
    pub fn snap_zone(&self, p: Point<f64, Logical>) -> Option<Tile> {
        // bordas da tela onde está o ponteiro (cada monitor tem as suas)
        let o = self.output_rect_at(p).to_f64();
        let p = Point::<f64, Logical>::from((p.x - o.loc.x, p.y - o.loc.y));
        let (w, h) = (o.size.w, o.size.h);
        let (l, r) = (p.x <= EDGE, p.x >= w - 1.0 - EDGE);
        let top = p.y <= metrics::MENUBAR_H as f64 + EDGE;
        let bottom = p.y >= h - 1.0 - EDGE;
        let corner = 120.0;
        Some(match (l, r, top, bottom) {
            (true, _, _, _) if p.y < metrics::MENUBAR_H as f64 + corner => Tile::TopLeft,
            (true, _, _, _) if p.y > h - corner => Tile::BottomLeft,
            (_, true, _, _) if p.y < metrics::MENUBAR_H as f64 + corner => Tile::TopRight,
            (_, true, _, _) if p.y > h - corner => Tile::BottomRight,
            (true, _, _, _) => Tile::Left,
            (_, true, _, _) => Tile::Right,
            (_, _, true, _) => Tile::Fill,
            _ => return None,
        })
    }

    pub fn snap_on_drop(&mut self, window: &Window, at: Point<f64, Logical>) {
        if self.game.active || !self.snapping {
            return;
        }
        if let Some(t) = self.snap_zone(at) {
            self.tile(window, t);
        }
    }

    /// Retângulo do conteúdo (sem a barra de título) para a área `t`.
    fn tile_rect(&self, window: &Window, t: Tile) -> Rectangle<i32, Logical> {
        let area = self.work_area_for(window);
        let gap = 6;
        let (ax, ay, aw, ah) = (area.loc.x + gap, area.loc.y + gap, area.size.w - 2 * gap, area.size.h - 2 * gap);
        let (hw, hh) = ((aw - gap) / 2, (ah - gap) / 2);
        let (x, y, w, h) = match t {
            Tile::Left => (ax, ay, hw, ah),
            Tile::Right => (ax + hw + gap, ay, aw - hw - gap, ah),
            Tile::Top => (ax, ay, aw, hh),
            Tile::Bottom => (ax, ay + hh + gap, aw, ah - hh - gap),
            Tile::TopLeft => (ax, ay, hw, hh),
            Tile::TopRight => (ax + hw + gap, ay, aw - hw - gap, hh),
            Tile::BottomLeft => (ax, ay + hh + gap, hw, ah - hh - gap),
            Tile::BottomRight => (ax + hw + gap, ay + hh + gap, aw - hw - gap, ah - hh - gap),
            Tile::Fill => (ax, ay, aw, ah),
        };
        let tb = if self.is_ssd(window) { metrics::TITLEBAR_H } else { 0 };
        Rectangle::new((x, y + tb).into(), (w, h - tb).into())
    }

    pub fn tile(&mut self, window: &Window, t: Tile) {
        let Some(loc) = self.space.element_location(window) else { return };
        {
            let mut m = meta(window).lock().unwrap();
            if m.fullscreen {
                return;
            }
            if m.tiled_from.is_none() {
                m.tiled_from = Some(Rectangle::new(loc, window.geometry().size));
            }
        }
        let r = self.tile_rect(window, t);
        if let Some(x) = window.x11_surface() {
            let _ = x.configure(r);
            self.space.map_element(window.clone(), r.loc, true);
            return;
        }
        let Some(top) = window.toplevel().cloned() else { return };
        top.with_pending_state(|s| {
            s.size = Some(r.size);
            s.states.set(xdg_toplevel::State::TiledLeft);
            s.states.set(xdg_toplevel::State::TiledRight);
            s.states.set(xdg_toplevel::State::TiledTop);
            s.states.set(xdg_toplevel::State::TiledBottom);
        });
        top.send_pending_configure();
        self.space.map_element(window.clone(), r.loc, true);
    }

    /// ⌘⌥ ⌫: volta ao tamanho e posição de antes do encaixe.
    pub fn untile(&mut self, window: &Window) {
        let Some(prev) = meta(window).lock().unwrap().tiled_from.take() else { return };
        if let Some(x) = window.x11_surface() {
            let _ = x.configure(prev);
            self.space.map_element(window.clone(), prev.loc, true);
            return;
        }
        let Some(top) = window.toplevel().cloned() else { return };
        top.with_pending_state(|s| {
            s.size = Some(prev.size);
            for st in [xdg_toplevel::State::TiledLeft, xdg_toplevel::State::TiledRight, xdg_toplevel::State::TiledTop, xdg_toplevel::State::TiledBottom] {
                s.states.unset(st);
            }
        });
        top.send_pending_configure();
        self.space.map_element(window.clone(), prev.loc, true);
    }

    /// Canto ativo (superior esquerdo) abre o Mission Control, como os Hot Corners do macOS.
    pub fn hot_corner(&mut self, p: Point<f64, Logical>) {
        let inside = p.x <= 1.0 && p.y <= 1.0;
        if inside && !self.in_hot_corner && !self.game.active && self.hot_corners {
            if self.shell.mission.open {
                self.shell.mission.toggle();
            } else {
                self.open_mission();
            }
        }
        self.in_hot_corner = inside;
    }
}
