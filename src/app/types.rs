use std::path::PathBuf;

use crate::api::AyahText;

pub(super) struct AyahTextLoadResult {
    pub(super) request_id: u64,
    pub(super) surah: u32,
    pub(super) ayahs: Vec<AyahText>,
}

#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub(crate) enum Mode {
    Browse,
    Listen,
    Study,
    Settings,
}

impl Mode {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Browse => "browse",
            Self::Listen => "listen",
            Self::Study => "study",
            Self::Settings => "settings",
        }
    }

    pub(super) fn from_str(value: Option<&str>) -> Self {
        match value {
            Some("listen") => Self::Listen,
            Some("study") => Self::Study,
            Some("settings") => Self::Settings,
            _ => Self::Browse,
        }
    }
}

#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub(crate) enum Focus {
    Reciters,
    Surahs,
}

impl Focus {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Reciters => "reciters",
            Self::Surahs => "surahs",
        }
    }

    pub(super) fn from_str(value: Option<&str>) -> Self {
        match value {
            Some("surahs") => Self::Surahs,
            _ => Self::Reciters,
        }
    }
}

#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub(crate) enum BrowseFilter {
    All,
    Favorites,
    Downloaded,
    HasTiming,
    Recent,
}

impl BrowseFilter {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::Favorites => "favorites",
            Self::Downloaded => "downloaded",
            Self::HasTiming => "timing",
            Self::Recent => "recent",
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::All => "All",
            Self::Favorites => "Favorites",
            Self::Downloaded => "Downloaded",
            Self::HasTiming => "Has timing",
            Self::Recent => "Recent",
        }
    }

    pub(crate) fn next(self) -> Self {
        match self {
            Self::All => Self::Favorites,
            Self::Favorites => Self::Downloaded,
            Self::Downloaded => Self::HasTiming,
            Self::HasTiming => Self::Recent,
            Self::Recent => Self::All,
        }
    }

    pub(super) fn from_str(value: Option<&str>) -> Self {
        match value {
            Some("favorites") => Self::Favorites,
            Some("downloaded") => Self::Downloaded,
            Some("timing") => Self::HasTiming,
            Some("recent") => Self::Recent,
            _ => Self::All,
        }
    }
}

#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub(crate) enum RepeatMode {
    Off,
    One,
    All,
}

impl RepeatMode {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::One => "one",
            Self::All => "all",
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Off => "Repeat off",
            Self::One => "Repeat one",
            Self::All => "Repeat all",
        }
    }

    pub(crate) fn next(self) -> Self {
        match self {
            Self::Off => Self::One,
            Self::One => Self::All,
            Self::All => Self::Off,
        }
    }

    pub(super) fn from_str(value: Option<&str>) -> Self {
        match value {
            Some("one") => Self::One,
            Some("all") => Self::All,
            _ => Self::Off,
        }
    }
}

#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub(crate) enum SettingsField {
    PreferOffline,
    CacheStreams,
    DownloadDirectory,
    DownloadConcurrency,
}

impl SettingsField {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::PreferOffline => "prefer_offline",
            Self::CacheStreams => "cache_streams",
            Self::DownloadDirectory => "download_directory",
            Self::DownloadConcurrency => "download_concurrency",
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::PreferOffline => "Prefer offline playback",
            Self::CacheStreams => "Save streams while playing",
            Self::DownloadDirectory => "Download directory",
            Self::DownloadConcurrency => "Concurrent downloads",
        }
    }

    pub(crate) fn next(self) -> Self {
        match self {
            Self::PreferOffline => Self::CacheStreams,
            Self::CacheStreams => Self::DownloadDirectory,
            Self::DownloadDirectory => Self::DownloadConcurrency,
            Self::DownloadConcurrency => Self::PreferOffline,
        }
    }

    pub(crate) fn previous(self) -> Self {
        match self {
            Self::PreferOffline => Self::DownloadConcurrency,
            Self::CacheStreams => Self::PreferOffline,
            Self::DownloadDirectory => Self::CacheStreams,
            Self::DownloadConcurrency => Self::DownloadDirectory,
        }
    }

    pub(super) fn from_str(value: Option<&str>) -> Self {
        match value {
            Some("cache_streams") => Self::CacheStreams,
            Some("download_directory") => Self::DownloadDirectory,
            Some("download_concurrency") => Self::DownloadConcurrency,
            _ => Self::PreferOffline,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AppSettings {
    pub(crate) prefer_offline_playback: bool,
    pub(crate) cache_streams_while_playing: bool,
    pub(crate) download_directory: String,
    pub(crate) download_concurrency: usize,
}

#[derive(Debug, Clone)]
pub(super) struct QueueItem {
    pub(super) reciter_index: usize,
    pub(super) reciter_id: u32,
    pub(super) surah_number: u32,
    pub(super) server: String,
}

#[derive(Debug, Clone)]
pub(super) struct ActiveStreamRecording {
    pub(super) reciter_id: u32,
    pub(super) reciter_name: String,
    pub(super) surah_id: u32,
    pub(super) server: String,
    pub(super) path: PathBuf,
}
