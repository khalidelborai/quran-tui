use std::sync::mpsc;
use std::thread;

use tracing::{debug, info};

use crate::api::{AyahTiming, TimingRead, fetch_ayah_timing};

pub(crate) fn find_current_ayah(timings: &[AyahTiming], position_ms: u32) -> Option<&AyahTiming> {
    timings
        .iter()
        .find(|timing| timing.start_time <= position_ms && position_ms < timing.end_time)
}

struct TimingLoadResult {
    surah: u32,
    read_id: u32,
    timings: Vec<AyahTiming>,
}

pub(crate) struct MushafWidget {
    timings: Vec<AyahTiming>,
    pub(crate) timing_reads: Vec<TimingRead>,
    current_ayah: Option<u32>,
    current_read_id: Option<u32>,
    loaded_surah: Option<u32>,
    timing_rx: Option<mpsc::Receiver<TimingLoadResult>>,
    pub(crate) timing_status: Option<String>,
}

impl MushafWidget {
    pub(crate) fn new() -> Self {
        Self {
            timings: vec![],
            timing_reads: vec![],
            current_ayah: None,
            current_read_id: None,
            loaded_surah: None,
            timing_rx: None,
            timing_status: None,
        }
    }

    pub(crate) fn poll_background_results(&mut self) {
        if let Some(rx) = self.timing_rx.take() {
            match rx.try_recv() {
                Ok(result) => {
                    info!(
                        surah = result.surah,
                        read_id = result.read_id,
                        count = result.timings.len(),
                        "Timing result received"
                    );
                    self.timings = result.timings;
                    self.loaded_surah = Some(result.surah);
                    self.current_read_id = Some(result.read_id);
                    self.current_ayah = None;
                    self.timing_status = if self.timings.is_empty() {
                        Some("Timing data unavailable".to_string())
                    } else {
                        Some("Timing ready".to_string())
                    };
                }
                Err(mpsc::TryRecvError::Empty) => {
                    self.timing_rx = Some(rx);
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.timing_status = Some("Timing worker stopped".to_string());
                }
            }
        }
    }

    pub(crate) fn set_timing_reads(&mut self, reads: Vec<TimingRead>) {
        self.timing_reads = reads;
    }

    pub(crate) fn current_ayah(&self) -> Option<u32> {
        self.current_ayah
    }

    pub(crate) fn ayah_bounds(&self, ayah: u32) -> Option<(u32, u32)> {
        self.timings
            .iter()
            .find(|timing| timing.ayah == ayah)
            .map(|timing| (timing.start_time, timing.end_time))
    }

    pub(crate) fn find_read_id(&self, server_url: &str) -> Option<u32> {
        let server = normalize_url_parts(server_url)?;
        self.timing_reads
            .iter()
            .find(|read| {
                normalize_url_parts(&read.folder_url)
                    .map(|folder| folder.matches(&server))
                    .unwrap_or(false)
            })
            .map(|read| read.id)
    }

    pub(crate) fn load_timing_async(&mut self, surah: u32, read_id: u32) {
        if self.loaded_surah == Some(surah) && self.current_read_id == Some(read_id) {
            debug!(surah, read_id, "Timing already loaded, skipping");
            return;
        }

        info!(surah, read_id, "Loading ayah timing in background");
        self.current_ayah = None;
        self.timings.clear();
        self.loaded_surah = None;
        self.current_read_id = Some(read_id);
        self.timing_status = Some("Loading timing…".to_string());

        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            let timings = fetch_ayah_timing(surah, read_id);
            let _ = tx.send(TimingLoadResult {
                surah,
                read_id,
                timings,
            });
        });
        self.timing_rx = Some(rx);
    }

    pub(crate) fn clear_timing(&mut self, status: &str) {
        self.timings.clear();
        self.loaded_surah = None;
        self.current_read_id = None;
        self.current_ayah = None;
        self.timing_status = Some(status.to_string());
    }

    pub(crate) fn update_position(&mut self, position_secs: f64) {
        let position_ms = (position_secs * 1000.0) as u32;
        let next_ayah = find_current_ayah(&self.timings, position_ms).map(|timing| timing.ayah);

        if self.current_ayah != next_ayah {
            debug!(
                position_ms,
                previous_ayah = ?self.current_ayah,
                next_ayah = ?next_ayah,
                "Ayah changed"
            );
            self.current_ayah = next_ayah;
        }
    }

    #[cfg(test)]
    pub(crate) fn set_current_ayah_for_test(&mut self, ayah: u32) {
        self.current_ayah = Some(ayah);
    }
}

#[derive(Debug, PartialEq, Eq)]
struct NormalizedUrlParts {
    host: String,
    path_segments: Vec<String>,
}

impl NormalizedUrlParts {
    fn matches(&self, other: &Self) -> bool {
        self.host == other.host && path_segments_overlap(&self.path_segments, &other.path_segments)
    }
}

fn normalize_url_parts(value: &str) -> Option<NormalizedUrlParts> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }

    let normalized = if value.contains("://") {
        value.to_string()
    } else {
        format!("https://{value}")
    };
    let parsed = reqwest::Url::parse(&normalized).ok()?;
    let host = parsed.host_str()?.to_ascii_lowercase();
    let path_segments = parsed
        .path_segments()
        .map(|segments| {
            segments
                .filter(|segment| !segment.is_empty())
                .map(|segment| segment.to_string())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    Some(NormalizedUrlParts {
        host,
        path_segments,
    })
}

fn path_segments_overlap(left: &[String], right: &[String]) -> bool {
    if left.is_empty() || right.is_empty() {
        return false;
    }

    let (shorter, longer) = if left.len() <= right.len() {
        (left, right)
    } else {
        (right, left)
    };

    shorter
        .iter()
        .zip(longer.iter())
        .all(|(left_segment, right_segment)| left_segment == right_segment)
}

#[cfg(test)]
mod tests {
    use super::{MushafWidget, normalize_url_parts, path_segments_overlap};
    use crate::api::TimingRead;

    #[test]
    fn strict_read_matching_rejects_false_positive_paths() {
        let mut mushaf = MushafWidget::new();
        mushaf.set_timing_reads(vec![TimingRead {
            id: 10,
            _name: "Reader".to_string(),
            folder_url: "server6.mp3quran.net/ak".to_string(),
        }]);

        assert_eq!(
            mushaf.find_read_id("https://server6.mp3quran.net/akdr/"),
            None
        );
    }

    #[test]
    fn normalize_url_parts_supports_missing_scheme() {
        let parts = normalize_url_parts("server6.mp3quran.net/akdr/").expect("normalized url");
        assert_eq!(parts.host, "server6.mp3quran.net");
        assert_eq!(parts.path_segments, vec!["akdr"]);
    }

    #[test]
    fn path_segments_overlap_uses_segment_boundaries() {
        let left = vec!["ak".to_string()];
        let right = vec!["akdr".to_string()];
        assert!(!path_segments_overlap(&left, &right));
    }
}
