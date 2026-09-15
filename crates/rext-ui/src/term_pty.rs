use crate::term_grid::SharedGrid;
use crate::term_vte::GridPerform;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};

fn wake_ui() {
    #[cfg(target_arch = "wasm32")]
    repose_core::request_frame();
    #[cfg(not(target_arch = "wasm32"))]
    repose_platform::wake_event_loop();
}

pub struct PtySession {
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
    master: Arc<Mutex<Box<dyn portable_pty::MasterPty + Send>>>,
    killer: Arc<Mutex<Option<Box<dyn portable_pty::ChildKiller + Send + Sync>>>>,
    pub grid: SharedGrid,
    pub title: Arc<Mutex<String>>,
    pub alive: Arc<AtomicBool>,
}

impl PtySession {
    pub fn spawn(
        grid: SharedGrid,
        cwd: PathBuf,
        cols: u16,
        rows: u16,
        shell: Option<String>,
    ) -> anyhow::Result<Self> {
        use portable_pty::{CommandBuilder, PtySize, native_pty_system};

        let pty_system = native_pty_system();
        let pair = pty_system.openpty(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })?;

        let shell_path =
            shell.unwrap_or_else(|| std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".into()));
        let mut cmd = CommandBuilder::new(&shell_path);
        cmd.cwd(cwd);
        cmd.env("TERM", "xterm-256color");
        cmd.env("COLORTERM", "truecolor");
        if shell_path.ends_with("bash") {
            cmd.arg("-i");
        }

        let mut child = pair.slave.spawn_command(cmd)?;
        drop(pair.slave);
        let killer = Arc::new(Mutex::new(Some(child.clone_killer())));
        let alive = Arc::new(AtomicBool::new(true));

        let mut reader = pair
            .master
            .try_clone_reader()
            .map_err(|e| anyhow::anyhow!("pty reader: {e}"))?;
        let writer = pair.master.take_writer()?;
        let writer = Arc::new(Mutex::new(writer));
        let master = Arc::new(Mutex::new(pair.master));

        let performer_grid = grid.clone();
        let alive_r = alive.clone();
        let killer_r = killer.clone();
        std::thread::Builder::new()
            .name("rext-pty-reader".into())
            .spawn(move || {
                let mut performer = GridPerform::new(performer_grid);
                let mut buf = [0u8; 8192];
                loop {
                    match reader.read(&mut buf) {
                        Ok(0) => break,
                        Ok(n) => {
                            performer.feed(&buf[..n]);
                            wake_ui();
                        }
                        Err(_) => break,
                    }
                }
                alive_r.store(false, Ordering::SeqCst);
                let _ = child.try_wait();
                *killer_r.lock().unwrap() = None;
                wake_ui();
            })?;

        Ok(Self {
            writer,
            master,
            killer,
            grid,
            title: Arc::new(Mutex::new(shell_path)),
            alive,
        })
    }

    pub fn write_input(&self, bytes: &[u8]) {
        if let Ok(mut w) = self.writer.lock() {
            let _ = w.write_all(bytes);
            let _ = w.flush();
        }
    }

    pub fn resize(&self, cols: u16, rows: u16) {
        if let Ok(m) = self.master.lock() {
            let _ = m.resize(portable_pty::PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            });
        }
        if let Ok(mut g) = self.grid.lock() {
            g.resize(cols as usize, rows as usize);
        }
    }

    pub fn is_alive(&self) -> bool {
        self.alive.load(Ordering::SeqCst)
    }

    pub fn kill(&self) {
        if let Some(mut k) = self.killer.lock().unwrap().take() {
            let _ = k.kill();
        }
    }
}
