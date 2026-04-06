use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc};
use std::thread;

use tracing::info;

use crate::config::{blocking_http_client, is_allowed_remote_url};
use crate::persistence::{StoredDownload, unix_timestamp};

const DEFAULT_CONCURRENCY: usize = 4;
const DOWNLOAD_BUFFER_SIZE: usize = 16 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DownloadStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
}

impl DownloadStatus {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Running => "running",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }

    pub(crate) fn from_str(value: &str) -> Self {
        match value {
            "running" => Self::Running,
            "completed" => Self::Completed,
            "failed" => Self::Failed,
            "cancelled" => Self::Cancelled,
            _ => Self::Pending,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DownloadRequest {
    pub(crate) reciter_id: u32,
    pub(crate) reciter_name: String,
    pub(crate) surah_id: u32,
    pub(crate) server: String,
    pub(crate) local_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LocalFileRecord {
    pub(crate) reciter_id: u32,
    pub(crate) reciter_name: String,
    pub(crate) surah_id: u32,
    pub(crate) server: String,
    pub(crate) local_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DownloadJob {
    pub(crate) reciter_id: u32,
    pub(crate) reciter_name: String,
    pub(crate) surah_id: u32,
    pub(crate) server: String,
    pub(crate) local_path: PathBuf,
    pub(crate) status: DownloadStatus,
    pub(crate) bytes_downloaded: u64,
    pub(crate) total_bytes: Option<u64>,
    pub(crate) error: Option<String>,
    pub(crate) updated_at: i64,
}

#[derive(Debug)]
enum DownloadEvent {
    Started(JobKey),
    Progress(JobKey, u64, Option<u64>),
    Finished(JobKey, PathBuf, u64),
    Failed(JobKey, String),
    Cancelled(JobKey),
    Skipped(JobKey, PathBuf, u64),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct JobKey {
    reciter_id: u32,
    surah_id: u32,
}

#[derive(Debug)]
pub(crate) struct DownloadManager {
    jobs: Vec<DownloadJob>,
    concurrency: usize,
    tx: mpsc::Sender<DownloadEvent>,
    rx: mpsc::Receiver<DownloadEvent>,
    active_jobs: HashMap<JobKey, Arc<AtomicBool>>,
    dirty: bool,
}

impl DownloadManager {
    pub(crate) fn with_downloads(downloads: Vec<StoredDownload>) -> Self {
        let (tx, rx) = mpsc::channel();
        let jobs = downloads
            .into_iter()
            .map(|download| {
                let mut status = DownloadStatus::from_str(&download.status);
                if matches!(status, DownloadStatus::Pending | DownloadStatus::Running) {
                    status = DownloadStatus::Cancelled;
                }
                if matches!(status, DownloadStatus::Completed) && !download.local_path.exists() {
                    status = DownloadStatus::Failed;
                }
                DownloadJob {
                    reciter_id: download.reciter_id,
                    reciter_name: download.reciter_name,
                    surah_id: download.surah_id,
                    server: download.server,
                    local_path: download.local_path,
                    status,
                    bytes_downloaded: download.bytes_downloaded,
                    total_bytes: download.total_bytes,
                    error: download.error,
                    updated_at: download.updated_at,
                }
            })
            .collect();

        Self {
            jobs,
            concurrency: DEFAULT_CONCURRENCY,
            tx,
            rx,
            active_jobs: HashMap::new(),
            dirty: false,
        }
    }

    pub(crate) fn set_concurrency(&mut self, concurrency: usize) {
        self.concurrency = concurrency.clamp(1, 8);
    }

    pub(crate) fn enqueue(&mut self, request: DownloadRequest) -> bool {
        let key = JobKey::from_request(&request);
        if let Some(existing) = self.job_mut(key) {
            match existing.status {
                DownloadStatus::Completed if existing.local_path.exists() => return false,
                DownloadStatus::Pending | DownloadStatus::Running => return false,
                _ => {
                    existing.status = DownloadStatus::Pending;
                    existing.error = None;
                    existing.server = request.server;
                    existing.reciter_name = request.reciter_name;
                    existing.local_path = request.local_path;
                    existing.updated_at = unix_timestamp();
                    self.dirty = true;
                    return true;
                }
            }
        }

        self.jobs.push(DownloadJob {
            reciter_id: request.reciter_id,
            reciter_name: request.reciter_name.clone(),
            surah_id: request.surah_id,
            server: request.server,
            local_path: request.local_path,
            status: DownloadStatus::Pending,
            bytes_downloaded: 0,
            total_bytes: None,
            error: None,
            updated_at: unix_timestamp(),
        });
        self.jobs.sort_by_key(|job| (job.reciter_id, job.surah_id));
        self.dirty = true;
        true
    }

    pub(crate) fn poll(&mut self) {
        while let Ok(event) = self.rx.try_recv() {
            self.apply_event(event);
        }
        self.start_pending_jobs();
    }

    #[cfg(test)]
    pub(crate) fn jobs(&self) -> &[DownloadJob] {
        &self.jobs
    }

    pub(crate) fn local_path_for(&self, reciter_id: u32, surah_id: u32) -> Option<PathBuf> {
        self.job(JobKey {
            reciter_id,
            surah_id,
        })
        .filter(|job| job.status == DownloadStatus::Completed && job.local_path.exists())
        .map(|job| job.local_path.clone())
    }

    pub(crate) fn reconcile_local_file(&mut self, file: LocalFileRecord) {
        let key = JobKey {
            reciter_id: file.reciter_id,
            surah_id: file.surah_id,
        };
        let size = file
            .local_path
            .metadata()
            .map(|meta| meta.len())
            .unwrap_or_default();

        if let Some(existing) = self.job_mut(key) {
            existing.reciter_name = file.reciter_name;
            existing.server = file.server;
            existing.local_path = file.local_path;
            existing.status = DownloadStatus::Completed;
            existing.bytes_downloaded = size;
            existing.total_bytes = Some(size);
            existing.error = None;
            existing.updated_at = unix_timestamp();
        } else {
            self.jobs.push(DownloadJob {
                reciter_id: file.reciter_id,
                reciter_name: file.reciter_name,
                surah_id: file.surah_id,
                server: file.server,
                local_path: file.local_path,
                status: DownloadStatus::Completed,
                bytes_downloaded: size,
                total_bytes: Some(size),
                error: None,
                updated_at: unix_timestamp(),
            });
            self.jobs.sort_by_key(|job| (job.reciter_id, job.surah_id));
        }
        self.dirty = true;
    }

    pub(crate) fn sync_local_files(&mut self, files: &[LocalFileRecord]) {
        let discovered_keys: std::collections::HashSet<_> = files
            .iter()
            .map(|file| JobKey {
                reciter_id: file.reciter_id,
                surah_id: file.surah_id,
            })
            .collect();

        for file in files {
            self.reconcile_local_file(file.clone());
        }

        for job in &mut self.jobs {
            let key = JobKey {
                reciter_id: job.reciter_id,
                surah_id: job.surah_id,
            };
            if matches!(job.status, DownloadStatus::Completed)
                && !discovered_keys.contains(&key)
                && !job.local_path.exists()
            {
                job.status = DownloadStatus::Failed;
                job.error = Some("local file missing".to_string());
                job.updated_at = unix_timestamp();
                self.dirty = true;
            }
        }
    }

    pub(crate) fn cancel(&mut self, reciter_id: u32, surah_id: u32) -> bool {
        let key = JobKey {
            reciter_id,
            surah_id,
        };
        if let Some(flag) = self.active_jobs.get(&key) {
            flag.store(true, Ordering::Relaxed);
            return true;
        }

        if let Some(job) = self.job_mut(key)
            && job.status == DownloadStatus::Pending
        {
            job.status = DownloadStatus::Cancelled;
            job.updated_at = unix_timestamp();
            self.dirty = true;
            return true;
        }

        false
    }

    pub(crate) fn retry(&mut self, reciter_id: u32, surah_id: u32) -> bool {
        let key = JobKey {
            reciter_id,
            surah_id,
        };
        let Some(job) = self.job_mut(key) else {
            return false;
        };
        if !matches!(
            job.status,
            DownloadStatus::Failed | DownloadStatus::Cancelled
        ) {
            return false;
        }
        job.status = DownloadStatus::Pending;
        job.error = None;
        job.bytes_downloaded = 0;
        job.total_bytes = None;
        job.updated_at = unix_timestamp();
        self.dirty = true;
        true
    }

    pub(crate) fn status_label(&self, reciter_id: u32, surah_id: u32) -> Option<String> {
        let job = self.job(JobKey {
            reciter_id,
            surah_id,
        })?;
        Some(match job.status {
            DownloadStatus::Pending => "queued".to_string(),
            DownloadStatus::Running => format!(
                "downloading {}{}",
                job.bytes_downloaded / 1024,
                job.total_bytes
                    .map(|total| format!("/{} KB", total / 1024))
                    .unwrap_or_default()
            ),
            DownloadStatus::Completed => "offline".to_string(),
            DownloadStatus::Failed => job
                .error
                .clone()
                .unwrap_or_else(|| "download failed".to_string()),
            DownloadStatus::Cancelled => "cancelled".to_string(),
        })
    }

    pub(crate) fn queue_preview(&self, limit: usize) -> Vec<String> {
        self.jobs
            .iter()
            .filter(|job| {
                matches!(
                    job.status,
                    DownloadStatus::Pending | DownloadStatus::Running
                )
            })
            .take(limit)
            .map(|job| format!("{:03} {}", job.surah_id, job.reciter_name))
            .collect()
    }

    pub(crate) fn take_persisted_downloads(&mut self) -> Option<Vec<StoredDownload>> {
        if !self.dirty {
            return None;
        }
        self.dirty = false;
        Some(
            self.jobs
                .iter()
                .map(|job| StoredDownload {
                    reciter_id: job.reciter_id,
                    reciter_name: job.reciter_name.clone(),
                    surah_id: job.surah_id,
                    server: job.server.clone(),
                    local_path: job.local_path.clone(),
                    status: job.status.as_str().to_string(),
                    bytes_downloaded: job.bytes_downloaded,
                    total_bytes: job.total_bytes,
                    error: job.error.clone(),
                    updated_at: job.updated_at,
                })
                .collect(),
        )
    }

    fn apply_event(&mut self, event: DownloadEvent) {
        match event {
            DownloadEvent::Started(key) => {
                if let Some(job) = self.job_mut(key) {
                    job.status = DownloadStatus::Running;
                    job.updated_at = unix_timestamp();
                    job.error = None;
                    self.dirty = true;
                }
            }
            DownloadEvent::Progress(key, downloaded, total) => {
                if let Some(job) = self.job_mut(key) {
                    job.status = DownloadStatus::Running;
                    job.bytes_downloaded = downloaded;
                    job.total_bytes = total;
                    job.updated_at = unix_timestamp();
                    self.dirty = true;
                }
            }
            DownloadEvent::Finished(key, path, total)
            | DownloadEvent::Skipped(key, path, total) => {
                self.active_jobs.remove(&key);
                if let Some(job) = self.job_mut(key) {
                    job.status = DownloadStatus::Completed;
                    job.local_path = path;
                    job.bytes_downloaded = total.max(job.bytes_downloaded);
                    job.total_bytes = Some(total.max(job.total_bytes.unwrap_or_default()));
                    job.error = None;
                    job.updated_at = unix_timestamp();
                    self.dirty = true;
                }
            }
            DownloadEvent::Failed(key, error) => {
                self.active_jobs.remove(&key);
                if let Some(job) = self.job_mut(key) {
                    job.status = DownloadStatus::Failed;
                    job.error = Some(error);
                    job.updated_at = unix_timestamp();
                    self.dirty = true;
                }
            }
            DownloadEvent::Cancelled(key) => {
                self.active_jobs.remove(&key);
                if let Some(job) = self.job_mut(key) {
                    job.status = DownloadStatus::Cancelled;
                    job.updated_at = unix_timestamp();
                    self.dirty = true;
                }
            }
        }
    }

    fn start_pending_jobs(&mut self) {
        let capacity = self.concurrency.saturating_sub(self.active_jobs.len());
        if capacity == 0 {
            return;
        }

        let pending_keys: Vec<JobKey> = self
            .jobs
            .iter()
            .filter(|job| job.status == DownloadStatus::Pending)
            .take(capacity)
            .map(|job| JobKey {
                reciter_id: job.reciter_id,
                surah_id: job.surah_id,
            })
            .collect();

        for key in pending_keys {
            let Some(job) = self.job(key).cloned() else {
                continue;
            };
            let cancel_flag = Arc::new(AtomicBool::new(false));
            self.active_jobs.insert(key, Arc::clone(&cancel_flag));
            let sender = self.tx.clone();
            thread::spawn(move || run_download(job, cancel_flag, sender));
        }
    }

    fn job(&self, key: JobKey) -> Option<&DownloadJob> {
        self.jobs
            .iter()
            .find(|job| job.reciter_id == key.reciter_id && job.surah_id == key.surah_id)
    }

    fn job_mut(&mut self, key: JobKey) -> Option<&mut DownloadJob> {
        self.jobs
            .iter_mut()
            .find(|job| job.reciter_id == key.reciter_id && job.surah_id == key.surah_id)
    }
}

fn run_download(
    job: DownloadJob,
    cancel_flag: Arc<AtomicBool>,
    sender: mpsc::Sender<DownloadEvent>,
) {
    let key = JobKey {
        reciter_id: job.reciter_id,
        surah_id: job.surah_id,
    };
    let final_path = job.local_path.clone();
    let _ = sender.send(DownloadEvent::Started(key));

    if final_path.exists() {
        let size = final_path
            .metadata()
            .map(|meta| meta.len())
            .unwrap_or_default();
        let _ = sender.send(DownloadEvent::Skipped(key, final_path, size));
        return;
    }

    let url = format!("{}{:03}.mp3", job.server, job.surah_id);
    if !is_allowed_remote_url(&url) {
        fail_download(&sender, key, "download URL rejected by allowlist");
        return;
    }

    let Some(parent) = final_path.parent() else {
        fail_download(&sender, key, "download path missing parent");
        return;
    };
    if let Err(error) = fs::create_dir_all(parent) {
        fail_download(&sender, key, error);
        return;
    }

    let temp_path = temp_download_path(&final_path);
    let response = match blocking_http_client().get(&url).send() {
        Ok(response) => response,
        Err(error) => {
            fail_download(&sender, key, error);
            return;
        }
    };
    let response = match response.error_for_status() {
        Ok(response) => response,
        Err(error) => {
            fail_download(&sender, key, error);
            return;
        }
    };

    let total = response.content_length();
    let mut response = response;
    let mut file = match File::create(&temp_path) {
        Ok(file) => file,
        Err(error) => {
            fail_download(&sender, key, error);
            return;
        }
    };

    let mut buffer = [0_u8; DOWNLOAD_BUFFER_SIZE];
    let mut downloaded = 0_u64;
    loop {
        if cancel_flag.load(Ordering::Relaxed) {
            let _ = fs::remove_file(&temp_path);
            let _ = sender.send(DownloadEvent::Cancelled(key));
            return;
        }

        let read = match response.read(&mut buffer) {
            Ok(0) => break,
            Ok(read) => read,
            Err(error) => {
                fail_download_with_cleanup(&sender, key, &temp_path, error);
                return;
            }
        };

        if let Err(error) = file.write_all(&buffer[..read]) {
            fail_download_with_cleanup(&sender, key, &temp_path, error);
            return;
        }

        downloaded += read as u64;
        let _ = sender.send(DownloadEvent::Progress(key, downloaded, total));
    }

    if let Err(error) = file.flush() {
        fail_download_with_cleanup(&sender, key, &temp_path, error);
        return;
    }

    if let Err(error) = fs::rename(&temp_path, &final_path) {
        fail_download_with_cleanup(&sender, key, &temp_path, error);
        return;
    }

    info!(reciter_id = job.reciter_id, surah_id = job.surah_id, path = %final_path.display(), "download complete");
    let _ = sender.send(DownloadEvent::Finished(key, final_path, downloaded));
}

fn fail_download(sender: &mpsc::Sender<DownloadEvent>, key: JobKey, error: impl ToString) {
    let _ = sender.send(DownloadEvent::Failed(key, error.to_string()));
}

fn fail_download_with_cleanup(
    sender: &mpsc::Sender<DownloadEvent>,
    key: JobKey,
    temp_path: &Path,
    error: impl ToString,
) {
    let _ = fs::remove_file(temp_path);
    fail_download(sender, key, error);
}

fn temp_download_path(path: &Path) -> PathBuf {
    let mut temp_path = path.to_path_buf();
    temp_path.set_extension("part");
    temp_path
}

pub(crate) fn destination_path(
    root: &Path,
    reciter_id: u32,
    reciter_name: &str,
    surah_id: u32,
) -> PathBuf {
    let safe_name = sanitize_component(reciter_name);
    root.to_path_buf()
        .join(format!("{:04}-{}", reciter_id, safe_name))
        .join(format!("{:03}.mp3", surah_id))
}

#[cfg(test)]
pub(crate) fn default_destination_path(
    reciter_id: u32,
    reciter_name: &str,
    surah_id: u32,
) -> PathBuf {
    destination_path(
        &crate::config::downloads_root(),
        reciter_id,
        reciter_name,
        surah_id,
    )
}

fn sanitize_component(value: &str) -> String {
    let mut safe = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else if matches!(ch, ' ' | '-' | '_') {
                '-'
            } else {
                '_'
            }
        })
        .collect::<String>();
    while safe.contains("--") {
        safe = safe.replace("--", "-");
    }
    let trimmed = safe.trim_matches(['-', '_']);
    if trimmed.is_empty() {
        "reciter".to_string()
    } else {
        trimmed.to_string()
    }
}

impl JobKey {
    fn from_request(request: &DownloadRequest) -> Self {
        Self {
            reciter_id: request.reciter_id,
            surah_id: request.surah_id,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        DownloadManager, DownloadRequest, DownloadStatus, default_destination_path,
        sanitize_component,
    };
    use crate::persistence::StoredDownload;
    use std::path::PathBuf;

    #[test]
    fn enqueue_deduplicates_existing_completed_job() {
        let path = default_destination_path(1, "Reader", 1);
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(&path, b"ok");

        let mut manager = DownloadManager::with_downloads(vec![StoredDownload {
            reciter_id: 1,
            reciter_name: "Reader".to_string(),
            surah_id: 1,
            server: "https://server6.mp3quran.net/reader/".to_string(),
            local_path: path.clone(),
            status: "completed".to_string(),
            bytes_downloaded: 2,
            total_bytes: Some(2),
            error: None,
            updated_at: 1,
        }]);

        let changed = manager.enqueue(DownloadRequest {
            reciter_id: 1,
            reciter_name: "Reader".to_string(),
            surah_id: 1,
            server: "https://server6.mp3quran.net/reader/".to_string(),
            local_path: path.clone(),
        });

        assert!(!changed);
        assert_eq!(manager.jobs().len(), 1);
        assert_eq!(manager.jobs()[0].status, DownloadStatus::Completed);
    }

    #[test]
    fn retry_requeues_failed_job() {
        let mut manager = DownloadManager::with_downloads(vec![StoredDownload {
            reciter_id: 1,
            reciter_name: "Reader".to_string(),
            surah_id: 2,
            server: "https://server6.mp3quran.net/reader/".to_string(),
            local_path: PathBuf::from("/tmp/002.mp3"),
            status: "failed".to_string(),
            bytes_downloaded: 0,
            total_bytes: None,
            error: Some("no network".to_string()),
            updated_at: 1,
        }]);

        assert!(manager.retry(1, 2));
        assert_eq!(manager.jobs()[0].status, DownloadStatus::Pending);
        assert_eq!(manager.jobs()[0].error, None);
    }

    #[test]
    fn sanitize_component_produces_safe_folder_names() {
        assert_eq!(sanitize_component("Reader Name"), "reader-name");
        assert_eq!(sanitize_component("مشاري"), "reciter");
    }
}
