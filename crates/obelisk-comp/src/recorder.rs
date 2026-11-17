//! Gravação de tela (⇧⌘5): o compositor lê cada quadro e envia RGBA cru para o ffmpeg (H.264).
//! Uma thread escreve no ffmpeg com fila curta: se o codificador atrasar, quadros são
//! descartados em vez de travar a composição.

use std::io::Write;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{sync_channel, SyncSender, TrySendError};
use std::time::Instant;

pub struct Recorder {
    pub path: PathBuf,
    pub started: Instant,
    /// recorte em px físicos (x, y, w, h), já com largura/altura pares
    pub region: (i32, i32, i32, i32),
    tx: Option<SyncSender<Vec<u8>>>,
    child: Child,
    pub dropped: u64,
    pub frames: u64,
    last_push: Option<Instant>,
}

impl Recorder {
    pub fn start(region: (i32, i32, i32, i32), flip_y: bool) -> Result<Self, String> {
        let (x, y, w, h) = region;
        let (w, h) = (w & !1, h & !1);
        if w < 16 || h < 16 {
            return Err("área pequena demais".into());
        }
        let home = PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| "/tmp".into()));
        let dir = ["Vídeos", "Videos"].iter().map(|d| home.join(d)).find(|p| p.is_dir()).unwrap_or(home.join("Vídeos"));
        let dir = dir.join("Gravações de Tela");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join(chrono::Local::now().format("Gravação de Tela %Y-%m-%d às %H.%M.%S.mp4").to_string());
        let mut cmd = Command::new("ffmpeg");
        cmd.args(["-hide_banner", "-loglevel", "error", "-y", "-f", "rawvideo", "-pix_fmt", "rgba"])
            .args(["-s", &format!("{w}x{h}"), "-use_wallclock_as_timestamps", "1", "-i", "-"]);
        if flip_y {
            cmd.args(["-vf", "vflip"]);
        }
        cmd.args(["-c:v", "libx264", "-preset", "ultrafast", "-crf", "23", "-pix_fmt", "yuv420p", "-fps_mode", "vfr"])
            .arg(&path)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        let mut child = cmd.spawn().map_err(|e| format!("ffmpeg: {e}"))?;
        let mut stdin = child.stdin.take().ok_or("stdin")?;
        let (tx, rx) = sync_channel::<Vec<u8>>(3);
        std::thread::Builder::new()
            .name("obelisk-rec".into())
            .spawn(move || {
                while let Ok(frame) = rx.recv() {
                    if stdin.write_all(&frame).is_err() {
                        break;
                    }
                }
            })
            .map_err(|e| e.to_string())?;
        Ok(Recorder { path, started: Instant::now(), region: (x, y, w, h), tx: Some(tx), child, dropped: 0, frames: 0, last_push: None })
    }

    /// Hora de um novo quadro? (limite de 60 por segundo)
    pub fn due(&self) -> bool {
        self.last_push.is_none_or(|t| t.elapsed().as_micros() >= 16_000)
    }

    pub fn push(&mut self, frame: Vec<u8>) {
        self.last_push = Some(Instant::now());
        self.frames += 1;
        if let Some(tx) = &self.tx {
            if let Err(TrySendError::Full(_)) = tx.try_send(frame) {
                self.dropped += 1;
            }
        }
    }

    /// Fecha a entrada do ffmpeg (ele finaliza o MP4) e espera em segundo plano.
    pub fn stop(mut self) -> PathBuf {
        self.tx = None;
        let path = self.path.clone();
        std::thread::spawn(move || {
            let _ = self.child.wait();
        });
        path
    }
}
