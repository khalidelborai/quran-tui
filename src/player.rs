use std::io::{self, BufRead, Write as IoWrite};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::Duration;

use tracing::{debug, info, warn};

use crate::config::runtime_path;

pub(crate) struct MpvPlayer {
    backend: Backend,
    startup_error: Option<String>,
}

impl MpvPlayer {
    pub(crate) fn new() -> Self {
        Self {
            backend: Backend::real(),
            startup_error: None,
        }
    }

    #[cfg(test)]
    pub(crate) fn with_command(command: impl Into<String>) -> Self {
        Self {
            backend: Backend::real_with_command(command.into()),
            startup_error: None,
        }
    }

    #[cfg(test)]
    pub(crate) fn stub_unavailable() -> Self {
        Self {
            backend: Backend::test(),
            startup_error: Some("test stub player unavailable".to_string()),
        }
    }

    #[cfg(test)]
    pub(crate) fn stub_available() -> Self {
        Self {
            backend: Backend::test(),
            startup_error: None,
        }
    }

    pub(crate) fn start(&mut self) -> Result<(), String> {
        self.startup_error = None;
        match &mut self.backend {
            Backend::Real(real) => match real.start() {
                Ok(()) => Ok(()),
                Err(message) => {
                    self.startup_error = Some(message.clone());
                    Err(message)
                }
            },
            #[cfg(test)]
            Backend::Test(_) => Ok(()),
        }
    }

    pub(crate) fn startup_error(&self) -> Option<&str> {
        self.startup_error.as_deref()
    }

    pub(crate) fn is_available(&self) -> bool {
        self.startup_error.is_none() && self.backend.is_available()
    }

    fn send_command(&self, cmd: &serde_json::Value) -> Option<serde_json::Value> {
        if !self.is_available() {
            return None;
        }
        self.backend.send_command(cmd)
    }

    pub(crate) fn play_url(&self, url: &str) {
        match &self.backend {
            Backend::Real(_) => {
                info!(url, "mpv play_url");
                self.send_command(&serde_json::json!({
                    "command": ["loadfile", url]
                }));
            }
            #[cfg(test)]
            Backend::Test(state) => state.mark_play_url(url),
        }
    }

    pub(crate) fn play_path(&self, path: &Path) {
        match &self.backend {
            Backend::Real(_) => {
                let path = path.display().to_string();
                info!(path, "mpv play_path");
                self.send_command(&serde_json::json!({
                    "command": ["loadfile", path]
                }));
            }
            #[cfg(test)]
            Backend::Test(state) => state.mark_play_path(path),
        }
    }

    pub(crate) fn set_stream_record(&self, path: &Path) {
        match &self.backend {
            Backend::Real(_) => {
                self.send_command(&serde_json::json!({
                    "command": ["set_property", "stream-record", path.display().to_string()]
                }));
            }
            #[cfg(test)]
            Backend::Test(state) => state.set_stream_record(path),
        }
    }

    pub(crate) fn clear_stream_record(&self) {
        match &self.backend {
            Backend::Real(_) => {
                self.send_command(&serde_json::json!({
                    "command": ["set_property", "stream-record", ""]
                }));
            }
            #[cfg(test)]
            Backend::Test(state) => state.clear_stream_record(),
        }
    }

    pub(crate) fn toggle_pause(&self) {
        match &self.backend {
            Backend::Real(_) => {
                self.send_command(&serde_json::json!({
                    "command": ["cycle", "pause"]
                }));
            }
            #[cfg(test)]
            Backend::Test(state) => state.toggle_pause(),
        }
    }

    pub(crate) fn seek(&self, seconds: f64) {
        match &self.backend {
            Backend::Real(_) => {
                self.send_command(&serde_json::json!({
                    "command": ["seek", seconds, "relative"]
                }));
            }
            #[cfg(test)]
            Backend::Test(state) => state.seek_relative(seconds),
        }
    }

    pub(crate) fn seek_absolute(&self, seconds: f64) {
        match &self.backend {
            Backend::Real(_) => {
                self.send_command(&serde_json::json!({
                    "command": ["seek", seconds, "absolute"]
                }));
            }
            #[cfg(test)]
            Backend::Test(state) => state.seek_absolute(seconds),
        }
    }

    fn get_property_f64(&self, prop: &str) -> Option<f64> {
        let response = self.send_command(&serde_json::json!({
            "command": ["get_property", prop]
        }))?;
        response.get("data")?.as_f64()
    }

    fn get_property_bool(&self, prop: &str) -> Option<bool> {
        let response = self.send_command(&serde_json::json!({
            "command": ["get_property", prop]
        }))?;
        response.get("data")?.as_bool()
    }

    pub(crate) fn get_position(&self) -> f64 {
        match &self.backend {
            Backend::Real(_) => self.get_property_f64("time-pos").unwrap_or(0.0),
            #[cfg(test)]
            Backend::Test(state) => state.position(),
        }
    }

    pub(crate) fn get_duration(&self) -> f64 {
        match &self.backend {
            Backend::Real(_) => self.get_property_f64("duration").unwrap_or(0.0),
            #[cfg(test)]
            Backend::Test(state) => state.duration(),
        }
    }

    pub(crate) fn eof_reached(&self) -> bool {
        match &self.backend {
            Backend::Real(_) => self.get_property_bool("eof-reached").unwrap_or(false),
            #[cfg(test)]
            Backend::Test(state) => state.eof_reached(),
        }
    }

    pub(crate) fn is_paused(&self) -> bool {
        match &self.backend {
            Backend::Real(_) => self.get_property_bool("pause").unwrap_or(true),
            #[cfg(test)]
            Backend::Test(state) => state.paused(),
        }
    }

    pub(crate) fn set_speed(&self, speed: f64) {
        match &self.backend {
            Backend::Real(_) => {
                self.send_command(&serde_json::json!({
                    "command": ["set_property", "speed", speed]
                }));
            }
            #[cfg(test)]
            Backend::Test(state) => state.set_speed(speed),
        }
    }

    pub(crate) fn stop(&mut self) {
        self.clear_stream_record();
        match &mut self.backend {
            Backend::Real(real) => {
                if self.startup_error.is_none() && real.is_available() {
                    real.send_command(&serde_json::json!({
                        "command": ["stop"]
                    }));
                }
                real.cleanup();
            }
            #[cfg(test)]
            Backend::Test(state) => state.stop(),
        }
    }

    #[cfg(test)]
    pub(crate) fn test_record_path(&self) -> Option<std::path::PathBuf> {
        match &self.backend {
            Backend::Real(_) => None,
            #[cfg(test)]
            Backend::Test(state) => state.stream_record.borrow().clone(),
        }
    }
}

impl Drop for MpvPlayer {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::MpvPlayer;

    #[test]
    fn start_reports_missing_binary() {
        let mut player = MpvPlayer::with_command("definitely-not-a-real-mpv-binary");
        let error = player.start().expect_err("missing binary should fail");
        assert!(error.contains("Failed to start"));
        assert_eq!(player.startup_error(), Some(error.as_str()));
    }

    #[test]
    fn stub_player_tracks_stream_record_path() {
        let player = MpvPlayer::stub_available();
        let path = std::env::temp_dir().join("quran-tui-record-test.mp3");
        player.set_stream_record(&path);
        assert_eq!(player.test_record_path(), Some(path));
    }
}

#[cfg(test)]
#[derive(Default)]
struct TestState {
    played_url: std::cell::RefCell<Option<String>>,
    played_path: std::cell::RefCell<Option<std::path::PathBuf>>,
    stream_record: std::cell::RefCell<Option<std::path::PathBuf>>,
    position: std::cell::Cell<f64>,
    duration: std::cell::Cell<f64>,
    paused: std::cell::Cell<bool>,
    eof_reached: std::cell::Cell<bool>,
    speed: std::cell::Cell<f64>,
    stopped: std::cell::Cell<bool>,
}

enum Backend {
    Real(RealBackend),
    #[cfg(test)]
    Test(TestState),
}

impl Backend {
    fn real() -> Self {
        Self::Real(RealBackend::new())
    }

    #[cfg(test)]
    fn real_with_command(command: String) -> Self {
        Self::Real(RealBackend::with_command(command))
    }

    #[cfg(test)]
    fn test() -> Self {
        Self::Test(TestState::default())
    }

    fn is_available(&self) -> bool {
        match self {
            Self::Real(real) => real.is_available(),
            #[cfg(test)]
            Self::Test(_) => true,
        }
    }

    fn send_command(&self, cmd: &serde_json::Value) -> Option<serde_json::Value> {
        match self {
            Self::Real(real) => real.send_command(cmd),
            #[cfg(test)]
            Self::Test(_) => Some(serde_json::json!({ "data": null })),
        }
    }
}

struct RealBackend {
    process: Option<Child>,
    socket_path: String,
    command: String,
}

impl RealBackend {
    fn new() -> Self {
        Self {
            process: None,
            socket_path: runtime_path("mpv.sock").display().to_string(),
            command: "mpv".to_string(),
        }
    }

    #[cfg(test)]
    fn with_command(command: String) -> Self {
        Self {
            command,
            ..Self::new()
        }
    }

    #[cfg(unix)]
    fn start(&mut self) -> Result<(), String> {
        info!(socket = %self.socket_path, command = %self.command, "Starting mpv");
        let _ = std::fs::remove_file(&self.socket_path);

        let child = Command::new(&self.command)
            .args([
                "--idle",
                "--no-video",
                "--no-terminal",
                "--really-quiet",
                &format!("--input-ipc-server={}", self.socket_path),
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn();

        match child {
            Ok(child) => {
                self.process = Some(child);
                for _ in 0..20 {
                    if Path::new(&self.socket_path).exists() {
                        info!(socket = %self.socket_path, "mpv started");
                        return Ok(());
                    }
                    thread::sleep(Duration::from_millis(100));
                }

                let message = format!("{} started but IPC socket never became ready", self.command);
                warn!(socket = %self.socket_path, error = %message, "mpv IPC startup failed");
                self.cleanup();
                Err(message)
            }
            Err(error) => {
                let message = format!("Failed to start {}: {}", self.command, error);
                warn!(error = %message, "mpv failed to start");
                Err(message)
            }
        }
    }

    #[cfg(not(unix))]
    fn start(&mut self) -> Result<(), String> {
        let message = format!(
            "{} IPC control is currently supported only on Unix-like platforms",
            self.command
        );
        warn!(error = %message, "mpv backend unsupported on this platform");
        self.cleanup();
        Err(message)
    }

    fn is_available(&self) -> bool {
        self.process.is_some()
    }

    fn cleanup(&mut self) {
        if let Some(mut child) = self.process.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        let _ = std::fs::remove_file(&self.socket_path);
    }

    #[cfg(unix)]
    fn send_command(&self, cmd: &serde_json::Value) -> Option<serde_json::Value> {
        use std::os::unix::net::UnixStream;

        debug!(cmd = %cmd, "mpv IPC command");
        let mut stream = match UnixStream::connect(&self.socket_path) {
            Ok(stream) => stream,
            Err(error) => {
                warn!(socket = %self.socket_path, error = %error, "mpv IPC connection failed");
                return None;
            }
        };

        stream
            .set_read_timeout(Some(Duration::from_millis(500)))
            .ok()?;

        let message = format!("{cmd}\n");
        stream.write_all(message.as_bytes()).ok()?;

        let mut reader = io::BufReader::new(&stream);
        let mut response = String::new();
        reader.read_line(&mut response).ok()?;

        let parsed: Option<serde_json::Value> = serde_json::from_str(&response).ok();
        debug!(response = %response.trim(), "mpv IPC response");
        parsed
    }

    #[cfg(not(unix))]
    fn send_command(&self, cmd: &serde_json::Value) -> Option<serde_json::Value> {
        debug!(cmd = %cmd, "mpv IPC command ignored on unsupported platform");
        None
    }
}

#[cfg(test)]
impl TestState {
    fn mark_play_url(&self, url: &str) {
        self.played_url.replace(Some(url.to_string()));
        self.played_path.replace(None);
        self.paused.replace(false);
        self.eof_reached.replace(false);
        self.position.replace(0.0);
    }

    fn mark_play_path(&self, path: &Path) {
        self.played_path.replace(Some(path.to_path_buf()));
        self.played_url.replace(None);
        self.paused.replace(false);
        self.eof_reached.replace(false);
        self.position.replace(0.0);
    }

    fn set_stream_record(&self, path: &Path) {
        self.stream_record.replace(Some(path.to_path_buf()));
    }

    fn clear_stream_record(&self) {
        self.stream_record.replace(None);
    }

    fn toggle_pause(&self) {
        let paused = self.paused.get();
        self.paused.replace(!paused);
    }

    fn seek_relative(&self, seconds: f64) {
        self.position
            .replace((self.position.get() + seconds).max(0.0));
    }

    fn seek_absolute(&self, seconds: f64) {
        self.position.replace(seconds.max(0.0));
    }

    fn position(&self) -> f64 {
        self.position.get()
    }

    fn duration(&self) -> f64 {
        self.duration.get()
    }

    fn eof_reached(&self) -> bool {
        self.eof_reached.get()
    }

    fn paused(&self) -> bool {
        self.paused.get()
    }

    fn set_speed(&self, speed: f64) {
        self.speed.replace(speed);
    }

    fn stop(&self) {
        self.stopped.replace(true);
        self.paused.replace(true);
    }
}
