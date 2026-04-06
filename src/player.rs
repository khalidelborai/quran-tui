use std::io;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::Duration;

#[cfg(windows)]
use std::future::Future;
#[cfg(unix)]
use std::io::{BufRead, Read, Write as IoWrite};
#[cfg(windows)]
use tokio::net::windows::named_pipe::{ClientOptions, NamedPipeClient};
use tracing::{debug, info, warn};

use crate::config::mpv_ipc_endpoint;

#[cfg(windows)]
const ERROR_PIPE_BUSY_OS_CODE: i32 = 231;

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
        assert!(error.contains("definitely-not-a-real-mpv-binary"));
        assert_eq!(player.startup_error(), Some(error.as_str()));
    }

    #[test]
    fn stub_player_tracks_stream_record_path() {
        let player = MpvPlayer::stub_available();
        let path = std::env::temp_dir().join("quran-tui-record-test.mp3");
        player.set_stream_record(&path);
        assert_eq!(player.test_record_path(), Some(path));
    }

    #[cfg(windows)]
    #[test]
    #[ignore = "requires mpv installed and on PATH"]
    fn windows_pipe_smoke_test() {
        let mut player = MpvPlayer::new();
        player.start().expect("mpv should start on Windows CI");

        let response = player
            .send_command(&serde_json::json!({
                "command": ["get_property", "pause"]
            }))
            .expect("mpv IPC should answer a property request");

        assert_eq!(
            response.get("error").and_then(|value| value.as_str()),
            Some("success")
        );
        assert!(
            response
                .get("data")
                .and_then(|value| value.as_bool())
                .is_some(),
            "pause property should be returned as a boolean"
        );

        player.stop();
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
    ipc_endpoint: String,
    command: String,
}

impl RealBackend {
    fn new() -> Self {
        Self {
            process: None,
            ipc_endpoint: mpv_ipc_endpoint(),
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

    fn start(&mut self) -> Result<(), String> {
        info!(endpoint = %self.ipc_endpoint, command = %self.command, "Starting mpv");
        self.cleanup_endpoint();

        let child = Command::new(&self.command)
            .args([
                "--idle",
                "--no-video",
                "--no-terminal",
                "--really-quiet",
                &format!("--input-ipc-server={}", self.ipc_endpoint),
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn();

        match child {
            Ok(child) => {
                self.process = Some(child);
                for _ in 0..20 {
                    if self.endpoint_ready() {
                        info!(endpoint = %self.ipc_endpoint, "mpv started");
                        return Ok(());
                    }
                    thread::sleep(Duration::from_millis(100));
                }

                let message = format!(
                    "{} started but IPC endpoint never became ready",
                    self.command
                );
                warn!(endpoint = %self.ipc_endpoint, error = %message, "mpv IPC startup failed");
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

    fn is_available(&self) -> bool {
        self.process.is_some()
    }

    fn cleanup(&mut self) {
        if let Some(mut child) = self.process.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        self.cleanup_endpoint();
    }

    #[cfg(unix)]
    fn endpoint_ready(&self) -> bool {
        Path::new(&self.ipc_endpoint).exists()
    }

    #[cfg(windows)]
    fn endpoint_ready(&self) -> bool {
        match self.try_connect_stream_once() {
            Ok(_) => true,
            Err(error) => self.is_retryable_pipe_connect(&error),
        }
    }

    #[cfg(unix)]
    fn cleanup_endpoint(&self) {
        let _ = std::fs::remove_file(&self.ipc_endpoint);
    }

    #[cfg(windows)]
    fn cleanup_endpoint(&self) {}

    #[cfg(unix)]
    fn read_response<T>(&self, mut stream: T, cmd: &serde_json::Value) -> Option<serde_json::Value>
    where
        T: Read + IoWrite,
    {
        let message = format!("{cmd}\n");
        stream.write_all(message.as_bytes()).ok()?;

        let mut reader = io::BufReader::new(stream);
        let mut response = String::new();
        reader.read_line(&mut response).ok()?;

        let parsed: Option<serde_json::Value> = serde_json::from_str(&response).ok();
        debug!(response = %response.trim(), "mpv IPC response");
        parsed
    }

    #[cfg(unix)]
    fn connect_stream(&self) -> io::Result<std::os::unix::net::UnixStream> {
        std::os::unix::net::UnixStream::connect(&self.ipc_endpoint)
    }

    #[cfg(windows)]
    fn with_runtime_io<F, T>(&self, future: F) -> io::Result<T>
    where
        F: Future<Output = io::Result<T>>,
    {
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            tokio::task::block_in_place(|| handle.block_on(future))
        } else {
            tokio::runtime::Builder::new_current_thread()
                .enable_io()
                .build()?
                .block_on(future)
        }
    }

    #[cfg(windows)]
    fn try_connect_stream_once(&self) -> io::Result<NamedPipeClient> {
        self.with_runtime_io(async { ClientOptions::new().open(&self.ipc_endpoint) })
    }

    #[cfg(windows)]
    fn is_retryable_pipe_connect(&self, error: &io::Error) -> bool {
        error.kind() == io::ErrorKind::NotFound
            || error.raw_os_error() == Some(ERROR_PIPE_BUSY_OS_CODE)
    }

    #[cfg(windows)]
    fn connect_stream(&self) -> io::Result<NamedPipeClient> {
        for _ in 0..10 {
            match self.try_connect_stream_once() {
                Ok(stream) => return Ok(stream),
                Err(error) if self.is_retryable_pipe_connect(&error) => {
                    thread::sleep(Duration::from_millis(50));
                }
                Err(error) => return Err(error),
            }
        }

        self.try_connect_stream_once()
    }

    #[cfg(windows)]
    async fn write_pipe_message(&self, client: &NamedPipeClient, message: &[u8]) -> io::Result<()> {
        let mut written = 0;
        while written < message.len() {
            client.writable().await?;
            match client.try_write(&message[written..]) {
                Ok(0) => {
                    return Err(io::Error::new(
                        io::ErrorKind::WriteZero,
                        "mpv IPC pipe closed while sending command",
                    ));
                }
                Ok(count) => written += count,
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => continue,
                Err(error) => return Err(error),
            }
        }
        Ok(())
    }

    #[cfg(windows)]
    async fn read_pipe_response(&self, client: &NamedPipeClient) -> io::Result<serde_json::Value> {
        let mut buffer = Vec::new();

        loop {
            client.readable().await?;

            let mut chunk = [0u8; 1024];
            match client.try_read(&mut chunk) {
                Ok(0) if buffer.is_empty() => {
                    return Err(io::Error::new(
                        io::ErrorKind::UnexpectedEof,
                        "mpv IPC pipe closed without a response",
                    ));
                }
                Ok(0) => break,
                Ok(count) => {
                    buffer.extend_from_slice(&chunk[..count]);
                    if let Some(newline_index) = buffer.iter().position(|byte| *byte == b'\n') {
                        buffer.truncate(newline_index);
                        break;
                    }
                }
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => continue,
                Err(error) => return Err(error),
            }
        }

        let response = String::from_utf8_lossy(&buffer);
        let parsed = serde_json::from_str(response.trim()).map_err(|error| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("invalid mpv IPC response: {error}"),
            )
        })?;
        debug!(response = %response.trim(), "mpv IPC response");
        Ok(parsed)
    }

    #[cfg(unix)]
    fn send_command(&self, cmd: &serde_json::Value) -> Option<serde_json::Value> {
        debug!(cmd = %cmd, "mpv IPC command");
        let stream = match self.connect_stream() {
            Ok(stream) => stream,
            Err(error) => {
                warn!(endpoint = %self.ipc_endpoint, error = %error, "mpv IPC connection failed");
                return None;
            }
        };

        stream
            .set_read_timeout(Some(Duration::from_millis(500)))
            .ok()?;

        self.read_response(stream, cmd)
    }

    #[cfg(windows)]
    fn send_command(&self, cmd: &serde_json::Value) -> Option<serde_json::Value> {
        debug!(cmd = %cmd, "mpv IPC command");
        let stream = match self.connect_stream() {
            Ok(stream) => stream,
            Err(error) => {
                warn!(endpoint = %self.ipc_endpoint, error = %error, "mpv IPC pipe connection failed");
                return None;
            }
        };

        let message = format!("{cmd}\n");
        self.with_runtime_io(async {
            self.write_pipe_message(&stream, message.as_bytes()).await?;
            self.read_pipe_response(&stream).await
        })
        .map_err(|error| {
            warn!(endpoint = %self.ipc_endpoint, error = %error, "mpv IPC command failed");
            error
        })
        .ok()
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
