use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub(crate) const API_BASE: &str = "https://www.mp3quran.net/api/v3";
pub(crate) const HTTP_TIMEOUT_SECS: u64 = 10;

static ASYNC_HTTP_CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
static BLOCKING_HTTP_CLIENT: OnceLock<reqwest::blocking::Client> = OnceLock::new();
static RUNTIME_SESSION_TAG: OnceLock<String> = OnceLock::new();

fn runtime_session_tag() -> &'static str {
    RUNTIME_SESSION_TAG.get_or_init(|| {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        format!("{}-{}", std::process::id(), timestamp)
    })
}

fn runtime_resource_name(name: &str) -> String {
    format!("quran-tui-{}-{}", runtime_session_tag(), name)
}

pub(crate) fn async_http_client() -> &'static reqwest::Client {
    ASYNC_HTTP_CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(HTTP_TIMEOUT_SECS))
            .timeout(Duration::from_secs(HTTP_TIMEOUT_SECS))
            .build()
            .expect("async HTTP client")
    })
}

pub(crate) fn blocking_http_client() -> &'static reqwest::blocking::Client {
    BLOCKING_HTTP_CLIENT.get_or_init(|| {
        reqwest::blocking::Client::builder()
            .connect_timeout(Duration::from_secs(HTTP_TIMEOUT_SECS))
            .timeout(Duration::from_secs(HTTP_TIMEOUT_SECS * 6))
            .build()
            .expect("blocking HTTP client")
    })
}

pub(crate) fn runtime_dir() -> PathBuf {
    std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .filter(|path| path.is_dir())
        .unwrap_or_else(std::env::temp_dir)
}

pub(crate) fn runtime_path(name: &str) -> PathBuf {
    runtime_dir().join(runtime_resource_name(name))
}

#[cfg(unix)]
pub(crate) fn mpv_ipc_endpoint() -> String {
    runtime_dir()
        .join(format!("{}.sock", runtime_resource_name("mpv")))
        .display()
        .to_string()
}

#[cfg(windows)]
pub(crate) fn mpv_ipc_endpoint() -> String {
    format!(r"\\.\pipe\{}", runtime_resource_name("mpv"))
}

pub(crate) fn data_dir() -> PathBuf {
    let base = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/share")))
        .unwrap_or_else(std::env::temp_dir);
    let path = base.join("quran-tui");
    let _ = std::fs::create_dir_all(&path);
    path
}

pub(crate) fn data_path(name: &str) -> PathBuf {
    data_dir().join(name)
}

pub(crate) fn downloads_root() -> PathBuf {
    let path = data_dir().join("downloads");
    let _ = std::fs::create_dir_all(&path);
    path
}

pub(crate) fn is_allowed_remote_url(url: &str) -> bool {
    let Ok(parsed) = reqwest::Url::parse(url) else {
        return false;
    };

    if parsed.scheme() != "https" {
        return false;
    }

    let Some(host) = parsed.host_str() else {
        return false;
    };

    host == "mp3quran.net" || host == "www.mp3quran.net" || host.ends_with(".mp3quran.net")
}

#[cfg(test)]
mod tests {
    use super::runtime_resource_name;

    #[test]
    fn runtime_resource_name_uses_safe_characters() {
        let value = runtime_resource_name("mpv");
        assert!(value.starts_with("quran-tui-"));
        assert!(
            value
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || ch == '-')
        );
    }
}
