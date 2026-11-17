//! Monitor de Atividade: processos do usuário com CPU % e memória, ordenados por CPU.
//! Lê o /proc só enquanto está aberto (1×/s). Encerrar (SIGTERM) ou Forçar (SIGKILL).

use std::collections::HashMap;
use std::time::Instant;

use resvg::tiny_skia::Pixmap;

use super::layer::Layer;
use crate::ui::canvas::Canvas;
use crate::ui::text::Weight;
use crate::ui::theme::Theme;

#[derive(Clone, Debug)]
pub struct Proc {
    pub pid: i32,
    pub name: String,
    pub cpu: f32,
    pub mem_mb: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ActHit {
    Row(usize),
    Quit,
    Kill,
    Close,
}

const W: f32 = 560.0;
const ROWS: usize = 14;
const ROW_H: f32 = 26.0;
const SHADOW: f32 = 40.0;

#[derive(Default)]
pub struct Activity {
    pub open: bool,
    pub selected: Option<usize>,
    pub procs: Vec<Proc>,
    prev: HashMap<i32, u64>,
    prev_total: u64,
    sampled: Option<Instant>,
    pub layer: Layer,
    pub pos: (f32, f32),
    pub size: (f32, f32),
    hits: Vec<(ActHit, (f32, f32, f32, f32))>,
    key: Option<String>,
}

fn total_jiffies() -> u64 {
    std::fs::read_to_string("/proc/stat")
        .ok()
        .and_then(|s| s.lines().next().map(|l| l.split_whitespace().skip(1).filter_map(|v| v.parse::<u64>().ok()).sum()))
        .unwrap_or(0)
}

impl Activity {
    pub fn toggle(&mut self) {
        self.open = !self.open;
        self.selected = None;
        self.sampled = None;
        self.key = None;
    }

    pub fn contains(&self, x: f64, y: f64) -> bool {
        self.open && (x as f32) >= self.pos.0 && (x as f32) < self.pos.0 + self.size.0 && (y as f32) >= self.pos.1 && (y as f32) < self.pos.1 + self.size.1
    }

    pub fn hit(&self, x: f64, y: f64) -> Option<ActHit> {
        let (x, y) = (x as f32, y as f32);
        self.hits.iter().find(|(_, r)| x >= r.0 && x < r.0 + r.2 && y >= r.1 && y < r.1 + r.3).map(|(h, _)| *h)
    }

    pub fn selected_pid(&self) -> Option<i32> {
        self.selected.and_then(|i| self.procs.get(i)).map(|p| p.pid)
    }

    /// Amostra o /proc (no máximo 1×/s). Devolve `true` se mudou.
    pub fn sample(&mut self) -> bool {
        if self.sampled.is_some_and(|t| t.elapsed().as_millis() < 1000) {
            return false;
        }
        self.sampled = Some(Instant::now());
        let uid = unsafe { libc::getuid() };
        let total = total_jiffies();
        let dt = total.saturating_sub(self.prev_total).max(1) as f32;
        let ncpu = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1) as f32;
        let mut list = Vec::new();
        let mut now = HashMap::new();
        let Ok(rd) = std::fs::read_dir("/proc") else { return false };
        for e in rd.flatten() {
            let Ok(pid) = e.file_name().to_string_lossy().parse::<i32>() else { continue };
            let base = e.path();
            let Ok(status) = std::fs::read_to_string(base.join("status")) else { continue };
            let owner = status.lines().find(|l| l.starts_with("Uid:")).and_then(|l| l.split_whitespace().nth(1)).and_then(|v| v.parse::<u32>().ok());
            if owner != Some(uid) {
                continue;
            }
            let Ok(stat) = std::fs::read_to_string(base.join("stat")) else { continue };
            // nome entre parênteses pode conter espaços
            let (Some(a), Some(b)) = (stat.find('('), stat.rfind(')')) else { continue };
            let name = stat[a + 1..b].to_string();
            let f: Vec<&str> = stat[b + 2..].split_whitespace().collect();
            let ticks = f.get(11).and_then(|v| v.parse::<u64>().ok()).unwrap_or(0) + f.get(12).and_then(|v| v.parse::<u64>().ok()).unwrap_or(0);
            let rss_pages = f.get(21).and_then(|v| v.parse::<u64>().ok()).unwrap_or(0);
            let cpu = self.prev.get(&pid).map(|p| ticks.saturating_sub(*p) as f32 / dt * 100.0 * ncpu).unwrap_or(0.0);
            now.insert(pid, ticks);
            list.push(Proc { pid, name, cpu, mem_mb: rss_pages as f32 * 4096.0 / 1048576.0 });
        }
        list.sort_by(|a, b| b.cpu.partial_cmp(&a.cpu).unwrap_or(std::cmp::Ordering::Equal).then(b.mem_mb.partial_cmp(&a.mem_mb).unwrap_or(std::cmp::Ordering::Equal)));
        list.truncate(ROWS);
        let sel_pid = self.selected_pid();
        self.procs = list;
        self.selected = sel_pid.and_then(|p| self.procs.iter().position(|x| x.pid == p));
        self.prev = now;
        self.prev_total = total;
        true
    }

    pub fn update(&mut self, theme: &Theme, sw: i32, sh: i32, scale: i32, backdrop: Option<&Pixmap>) {
        if self.procs.is_empty() {
            self.sample();
        }
        let h = 56.0 + 28.0 + ROWS as f32 * ROW_H + 56.0;
        self.size = (W, h);
        self.pos = (((sw as f32 - W) / 2.0).round(), ((sh as f32 - h) / 2.0).round());
        let key = format!("{:?}|{:?}|{}|{sw}", self.sampled, self.selected, theme.appearance as u8);
        if self.key.as_deref() == Some(key.as_str()) && self.layer.is_set() {
            return;
        }
        self.key = Some(key);
        self.hits.clear();
        let (sx, sy) = (self.pos.0 - SHADOW, self.pos.1 - SHADOW);
        let to_screen = |r: (f32, f32, f32, f32)| (r.0 + sx, r.1 + sy, r.2, r.3);
        let mut c = Canvas::new((W + 2.0 * SHADOW) as i32, (h + 2.0 * SHADOW) as i32, scale as f32).unwrap();
        let (ox, oy) = (SHADOW, SHADOW);
        c.shadow_rrect(ox, oy + 12.0, W, h, 18.0, 40.0, 128);
        if let Some(bd) = backdrop {
            c.fill_rrect_backdrop(ox, oy, W, h, 18.0, bd, (sx, sy), (sw as f32, sh as f32));
        }
        c.fill_rrect(ox, oy, W, h, 18.0, theme.menu_bg);
        c.stroke_rrect(ox, oy, W, h, 18.0, 1.0, theme.menu_border);
        c.text_vcenter("Monitor de Atividade", ox + 20.0, oy, 56.0, 16.0, Weight::Bold, theme.label);
        c.symbol("close", ox + W - 28.0, oy + 28.0, 18.0, theme.secondary);
        self.hits.push((ActHit::Close, to_screen((ox + W - 44.0, oy + 12.0, 32.0, 32.0))));

        let cols = [("Processo", 20.0), ("PID", 300.0), ("% CPU", 380.0), ("Memória", 460.0)];
        let hy = oy + 56.0;
        for (t, x) in cols {
            c.text_vcenter(t, ox + x, hy, 28.0, 11.0, Weight::Bold, theme.tertiary);
        }
        c.fill_rrect(ox + 12.0, hy + 27.0, W - 24.0, 1.0, 0.0, theme.separator);
        let mut y = hy + 28.0;
        for (i, p) in self.procs.iter().enumerate() {
            let sel = self.selected == Some(i);
            if sel {
                c.fill_rrect(ox + 8.0, y + 1.0, W - 16.0, ROW_H - 2.0, 7.0, theme.accent);
            } else if i % 2 == 1 {
                c.fill_rrect(ox + 8.0, y + 1.0, W - 16.0, ROW_H - 2.0, 7.0, theme.fg(0.03));
            }
            let col = if sel { theme.text_on_accent } else { theme.label };
            let name = c.ellipsized(&p.name, 13.0, Weight::Regular, 260.0);
            c.text_vcenter(&name, ox + 20.0, y, ROW_H, 13.0, Weight::Regular, col);
            c.text_vcenter(&p.pid.to_string(), ox + 300.0, y, ROW_H, 12.0, Weight::Regular, col);
            c.text_vcenter(&format!("{:.1}", p.cpu).replace('.', ","), ox + 380.0, y, ROW_H, 12.0, Weight::Regular, col);
            c.text_vcenter(&format!("{:.0} MB", p.mem_mb), ox + 460.0, y, ROW_H, 12.0, Weight::Regular, col);
            self.hits.push((ActHit::Row(i), to_screen((ox + 8.0, y, W - 16.0, ROW_H))));
            y += ROW_H;
        }
        // botões
        let by = oy + h - 44.0;
        let enabled = self.selected.is_some();
        for (label, hit, x, accent) in [("Encerrar", ActHit::Quit, ox + W - 20.0 - 200.0, false), ("Forçar Encerrar", ActHit::Kill, ox + W - 20.0 - 110.0, true)] {
            let bw = if accent { 110.0 } else { 84.0 };
            let bg = if accent { theme.red } else { theme.fg(0.12) };
            let a = if enabled { 255 } else { 90 };
            c.fill_rrect(x, by, bw, 30.0, 8.0, bg.with_alpha(((bg.3 as u32 * a) / 255) as u8));
            let tw = c.measure(label, 12.0, Weight::Semibold);
            let tc = if accent { theme.text_on_accent } else { theme.label };
            c.text_vcenter(label, x + (bw - tw) / 2.0, by, 30.0, 12.0, Weight::Semibold, tc.with_alpha(((tc.3 as u32 * a) / 255) as u8));
            if enabled {
                self.hits.push((hit, to_screen((x, by, bw, 30.0))));
            }
        }
        self.layer.set(&c);
    }
}
