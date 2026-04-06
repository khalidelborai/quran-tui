use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::thread;

use tracing::{info, trace, warn};

use self::types::{ActiveStreamRecording, AyahTextLoadResult, QueueItem};
pub(crate) use self::types::{AppSettings, BrowseFilter, Focus, Mode, RepeatMode, SettingsField};
use crate::api::{AyahText, Moshaf, Reciter, Surah, fetch_surah_text, parse_surah_list};
use crate::ayah_panel::AyahTextPanel;
use crate::config::downloads_root;
use crate::downloads::{DownloadManager, DownloadRequest, LocalFileRecord};
use crate::mushaf::MushafWidget;
use crate::persistence::{
    AppPersistence, FavoriteData, PersistedSnapshot, RecentEntry, SettingsSnapshot, unix_timestamp,
};
use crate::player::MpvPlayer;
use crate::shaping::{normalize_for_display, shape};

pub(crate) struct App {
    pub(crate) reciters: Vec<Reciter>,
    pub(crate) surahs: Vec<Surah>,
    reciter_display_names: Vec<String>,
    reciter_surah_lists: Vec<Vec<u32>>,
    surah_display_names: HashMap<u32, String>,
    pub(crate) selected_reciter: usize,
    pub(crate) selected_surah: usize,
    pub(crate) reciter_scroll: usize,
    pub(crate) surah_scroll: usize,
    pub(crate) reciter_viewport_height: usize,
    pub(crate) surah_viewport_height: usize,
    pub(crate) mode: Mode,
    pub(crate) focus: Focus,
    pub(crate) settings_field: SettingsField,
    pub(crate) player: MpvPlayer,
    pub(crate) playing_reciter: Option<usize>,
    pub(crate) playing_surah: Option<u32>,
    pub(crate) position: f64,
    pub(crate) duration: f64,
    pub(crate) speed: f64,
    pub(crate) speeds: Vec<f64>,
    pub(crate) speed_idx: usize,
    pub(crate) loading: bool,
    pub(crate) error: Option<String>,
    pub(crate) player_error: Option<String>,
    pub(crate) notice: Option<String>,
    pub(crate) mushaf: MushafWidget,
    pub(crate) ayah_text_panel: AyahTextPanel,
    ayah_texts: Vec<AyahText>,
    ayah_display_texts: Vec<String>,
    ayah_text_status: Option<String>,
    ayah_text_surah: Option<u32>,
    ayah_text_request_id: u64,
    latest_ayah_text_request_id: Option<u64>,
    ayah_text_rx: Option<mpsc::Receiver<AyahTextLoadResult>>,
    pub(crate) search_query: String,
    pub(crate) search_mode: bool,
    pub(crate) browse_filter: BrowseFilter,
    pub(crate) repeat_mode: RepeatMode,
    pub(crate) settings: AppSettings,
    pub(crate) settings_edit_mode: bool,
    pub(crate) settings_buffer: String,
    favorites: FavoriteData,
    recent: Vec<RecentEntry>,
    queue: Vec<QueueItem>,
    queue_index: Option<usize>,
    current_source_label: Option<String>,
    persistence: AppPersistence,
    restored_snapshot: PersistedSnapshot,
    pub(crate) downloads: DownloadManager,
    local_media_index: HashMap<(u32, u32), PathBuf>,
    active_stream_recording: Option<ActiveStreamRecording>,
    pub(crate) selected_ayah_index: usize,
    pub(crate) study_scroll: usize,
    pub(crate) study_viewport_height: usize,
    pub(crate) repeat_current_ayah: bool,
    pub(crate) loop_range: Option<(u32, u32)>,
    loop_latch_end_ms: Option<u32>,
    last_saved_position_bucket: i64,
    pending_g: bool,
}

pub(crate) fn adjust_scroll(selected: usize, scroll: usize, visible: usize) -> usize {
    if visible == 0 {
        scroll
    } else if selected < scroll {
        selected
    } else if selected >= scroll.saturating_add(visible) {
        selected + 1 - visible
    } else {
        scroll
    }
}

mod browser;
mod core;
mod navigation;
mod playback;
mod settings;
mod state;
mod study;
#[cfg(test)]
mod tests;
mod types;

fn nearest_speed_index(speeds: &[f64], speed: f64) -> usize {
    speeds
        .iter()
        .enumerate()
        .min_by(|(_, left), (_, right)| {
            let left_distance = (*left - speed).abs();
            let right_distance = (*right - speed).abs();
            left_distance
                .partial_cmp(&right_distance)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|(index, _)| index)
        .unwrap_or(1)
}

fn normalize_query(value: &str) -> String {
    normalize_for_display(value).to_lowercase()
}

fn normalize_media_component(value: &str) -> String {
    let mut normalized = String::new();
    let mut last_was_sep = false;
    for ch in value.chars() {
        if ch.is_alphanumeric() {
            normalized.extend(ch.to_lowercase());
            last_was_sep = false;
        } else if !last_was_sep {
            normalized.push('-');
            last_was_sep = true;
        }
    }
    normalized.trim_matches('-').to_string()
}

fn parse_surah_id_from_path(path: &Path) -> Option<u32> {
    let stem = path.file_stem()?.to_string_lossy();
    let digits: String = stem.chars().take_while(|ch| ch.is_ascii_digit()).collect();
    if digits.is_empty() {
        None
    } else {
        digits.parse().ok()
    }
}

fn on_off_label(value: bool) -> &'static str {
    if value { "on" } else { "off" }
}
