use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{Connection, OptionalExtension, params};
use tracing::warn;

use crate::config::data_path;

const SCHEMA_VERSION: i64 = 1;

#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct SettingsSnapshot {
    pub(crate) mode: Option<String>,
    pub(crate) focus: Option<String>,
    pub(crate) settings_field: Option<String>,
    pub(crate) browse_filter: Option<String>,
    pub(crate) repeat_mode: Option<String>,
    pub(crate) selected_reciter_id: Option<u32>,
    pub(crate) selected_surah: Option<u32>,
    pub(crate) last_reciter_id: Option<u32>,
    pub(crate) last_surah: Option<u32>,
    pub(crate) last_position: f64,
    pub(crate) speed: f64,
    pub(crate) search_query: String,
    pub(crate) prefer_offline: bool,
    pub(crate) cache_streams: bool,
    pub(crate) download_directory: String,
    pub(crate) download_concurrency: usize,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct FavoriteData {
    pub(crate) reciters: HashSet<u32>,
    pub(crate) surahs: HashSet<(u32, u32)>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct RecentEntry {
    pub(crate) reciter_id: u32,
    pub(crate) surah_id: u32,
    pub(crate) position_secs: f64,
    pub(crate) updated_at: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct StoredDownload {
    pub(crate) reciter_id: u32,
    pub(crate) reciter_name: String,
    pub(crate) surah_id: u32,
    pub(crate) server: String,
    pub(crate) local_path: PathBuf,
    pub(crate) status: String,
    pub(crate) bytes_downloaded: u64,
    pub(crate) total_bytes: Option<u64>,
    pub(crate) error: Option<String>,
    pub(crate) updated_at: i64,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct PersistedSnapshot {
    pub(crate) settings: SettingsSnapshot,
    pub(crate) favorites: FavoriteData,
    pub(crate) recent: Vec<RecentEntry>,
    pub(crate) downloads: Vec<StoredDownload>,
}

#[derive(Debug, Clone)]
pub(crate) struct AppPersistence {
    path: PathBuf,
}

impl AppPersistence {
    pub(crate) fn new() -> Self {
        Self::with_path(data_path("state.sqlite"))
    }

    pub(crate) fn with_path(path: PathBuf) -> Self {
        let persistence = Self { path };
        if let Err(error) = persistence.init() {
            warn!(error = %error, path = %persistence.path.display(), "Failed to initialize persistence");
        }
        persistence
    }

    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    pub(crate) fn load_snapshot(&self) -> PersistedSnapshot {
        let Ok(connection) = self.connection() else {
            return PersistedSnapshot::default();
        };

        PersistedSnapshot {
            settings: load_settings(&connection).unwrap_or_default(),
            favorites: load_favorites(&connection).unwrap_or_default(),
            recent: load_recent(&connection).unwrap_or_default(),
            downloads: load_downloads(&connection).unwrap_or_default(),
        }
    }

    pub(crate) fn save_settings(&self, settings: &SettingsSnapshot) {
        let Ok(connection) = self.connection() else {
            return;
        };

        let entries = [
            ("mode", settings.mode.clone().unwrap_or_default()),
            ("focus", settings.focus.clone().unwrap_or_default()),
            (
                "settings_field",
                settings.settings_field.clone().unwrap_or_default(),
            ),
            (
                "browse_filter",
                settings.browse_filter.clone().unwrap_or_default(),
            ),
            (
                "repeat_mode",
                settings.repeat_mode.clone().unwrap_or_default(),
            ),
            (
                "selected_reciter_id",
                settings
                    .selected_reciter_id
                    .map(|value| value.to_string())
                    .unwrap_or_default(),
            ),
            (
                "selected_surah",
                settings
                    .selected_surah
                    .map(|value| value.to_string())
                    .unwrap_or_default(),
            ),
            (
                "last_reciter_id",
                settings
                    .last_reciter_id
                    .map(|value| value.to_string())
                    .unwrap_or_default(),
            ),
            (
                "last_surah",
                settings
                    .last_surah
                    .map(|value| value.to_string())
                    .unwrap_or_default(),
            ),
            ("last_position", settings.last_position.to_string()),
            ("speed", settings.speed.to_string()),
            ("search_query", settings.search_query.clone()),
            ("prefer_offline", settings.prefer_offline.to_string()),
            ("cache_streams", settings.cache_streams.to_string()),
            ("download_directory", settings.download_directory.clone()),
            (
                "download_concurrency",
                settings.download_concurrency.to_string(),
            ),
        ];

        for (key, value) in entries {
            let _ = connection.execute(
                "INSERT INTO settings (key, value) VALUES (?1, ?2)
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                params![key, value],
            );
        }
    }

    pub(crate) fn save_favorites(&self, favorites: &FavoriteData) {
        let Ok(mut connection) = self.connection() else {
            return;
        };
        let Ok(transaction) = connection.transaction() else {
            return;
        };

        let _ = transaction.execute("DELETE FROM favorites", []);
        for reciter_id in &favorites.reciters {
            let _ = transaction.execute(
                "INSERT INTO favorites (kind, reciter_id, surah_id) VALUES ('reciter', ?1, NULL)",
                params![reciter_id],
            );
        }
        for (reciter_id, surah_id) in &favorites.surahs {
            let _ = transaction.execute(
                "INSERT INTO favorites (kind, reciter_id, surah_id) VALUES ('surah', ?1, ?2)",
                params![reciter_id, surah_id],
            );
        }
        let _ = transaction.commit();
    }

    pub(crate) fn save_recent(&self, recent: &[RecentEntry]) {
        let Ok(mut connection) = self.connection() else {
            return;
        };
        let Ok(transaction) = connection.transaction() else {
            return;
        };

        let _ = transaction.execute("DELETE FROM recent_history", []);
        for entry in recent {
            let _ = transaction.execute(
                "INSERT INTO recent_history (reciter_id, surah_id, position_secs, updated_at)
                 VALUES (?1, ?2, ?3, ?4)",
                params![
                    entry.reciter_id,
                    entry.surah_id,
                    entry.position_secs,
                    entry.updated_at
                ],
            );
        }
        let _ = transaction.commit();
    }

    pub(crate) fn save_downloads(&self, downloads: &[StoredDownload]) {
        let Ok(mut connection) = self.connection() else {
            return;
        };
        let Ok(transaction) = connection.transaction() else {
            return;
        };

        let _ = transaction.execute("DELETE FROM downloads", []);
        for download in downloads {
            let _ = transaction.execute(
                "INSERT INTO downloads (
                    reciter_id,
                    reciter_name,
                    surah_id,
                    server,
                    local_path,
                    status,
                    bytes_downloaded,
                    total_bytes,
                    error,
                    updated_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                params![
                    download.reciter_id,
                    download.reciter_name,
                    download.surah_id,
                    download.server,
                    download.local_path.display().to_string(),
                    download.status,
                    download.bytes_downloaded,
                    download.total_bytes,
                    download.error,
                    download.updated_at,
                ],
            );
        }
        let _ = transaction.commit();
    }

    fn init(&self) -> rusqlite::Result<()> {
        let connection = self.connection()?;
        connection.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS schema_version (version INTEGER NOT NULL);
            CREATE TABLE IF NOT EXISTS settings (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS favorites (
                kind TEXT NOT NULL,
                reciter_id INTEGER NOT NULL,
                surah_id INTEGER,
                UNIQUE(kind, reciter_id, surah_id)
            );
            CREATE TABLE IF NOT EXISTS recent_history (
                reciter_id INTEGER NOT NULL,
                surah_id INTEGER NOT NULL,
                position_secs REAL NOT NULL,
                updated_at INTEGER NOT NULL,
                PRIMARY KEY (reciter_id, surah_id)
            );
            CREATE TABLE IF NOT EXISTS downloads (
                reciter_id INTEGER NOT NULL,
                reciter_name TEXT NOT NULL,
                surah_id INTEGER NOT NULL,
                server TEXT NOT NULL,
                local_path TEXT NOT NULL,
                status TEXT NOT NULL,
                bytes_downloaded INTEGER NOT NULL,
                total_bytes INTEGER,
                error TEXT,
                updated_at INTEGER NOT NULL,
                PRIMARY KEY (reciter_id, surah_id)
            );
            ",
        )?;

        let version = connection
            .query_row(
                "SELECT version FROM schema_version ORDER BY version DESC LIMIT 1",
                [],
                |row| row.get::<_, i64>(0),
            )
            .optional()?
            .unwrap_or_default();
        if version == 0 {
            connection.execute(
                "INSERT INTO schema_version (version) VALUES (?1)",
                params![SCHEMA_VERSION],
            )?;
        }
        Ok(())
    }

    fn connection(&self) -> rusqlite::Result<Connection> {
        if let Some(parent) = self.path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let connection = Connection::open(&self.path)?;
        connection.busy_timeout(std::time::Duration::from_secs(1))?;
        Ok(connection)
    }
}

fn load_settings(connection: &Connection) -> rusqlite::Result<SettingsSnapshot> {
    let mut statement = connection.prepare("SELECT key, value FROM settings")?;
    let rows = statement.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;

    let mut settings = SettingsSnapshot {
        speed: 1.0,
        prefer_offline: true,
        cache_streams: true,
        download_concurrency: 4,
        ..SettingsSnapshot::default()
    };

    for row in rows {
        let (key, value) = row?;
        match key.as_str() {
            "mode" if !value.is_empty() => settings.mode = Some(value),
            "focus" if !value.is_empty() => settings.focus = Some(value),
            "settings_field" if !value.is_empty() => settings.settings_field = Some(value),
            "browse_filter" if !value.is_empty() => settings.browse_filter = Some(value),
            "repeat_mode" if !value.is_empty() => settings.repeat_mode = Some(value),
            "selected_reciter_id" => settings.selected_reciter_id = value.parse().ok(),
            "selected_surah" => settings.selected_surah = value.parse().ok(),
            "last_reciter_id" => settings.last_reciter_id = value.parse().ok(),
            "last_surah" => settings.last_surah = value.parse().ok(),
            "last_position" => settings.last_position = value.parse().unwrap_or_default(),
            "speed" => settings.speed = value.parse().unwrap_or(1.0),
            "search_query" => settings.search_query = value,
            "prefer_offline" => settings.prefer_offline = value.parse().unwrap_or(true),
            "cache_streams" => settings.cache_streams = value.parse().unwrap_or(true),
            "download_directory" => settings.download_directory = value,
            "download_concurrency" => settings.download_concurrency = value.parse().unwrap_or(4),
            _ => {}
        }
    }

    Ok(settings)
}

fn load_favorites(connection: &Connection) -> rusqlite::Result<FavoriteData> {
    let mut statement = connection.prepare("SELECT kind, reciter_id, surah_id FROM favorites")?;
    let rows = statement.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, u32>(1)?,
            row.get::<_, Option<u32>>(2)?,
        ))
    })?;

    let mut favorites = FavoriteData::default();
    for row in rows {
        let (kind, reciter_id, surah_id) = row?;
        match (kind.as_str(), surah_id) {
            ("reciter", _) => {
                favorites.reciters.insert(reciter_id);
            }
            ("surah", Some(surah_id)) => {
                favorites.surahs.insert((reciter_id, surah_id));
            }
            _ => {}
        }
    }

    Ok(favorites)
}

fn load_recent(connection: &Connection) -> rusqlite::Result<Vec<RecentEntry>> {
    let mut statement = connection.prepare(
        "SELECT reciter_id, surah_id, position_secs, updated_at
         FROM recent_history
         ORDER BY updated_at DESC
         LIMIT 20",
    )?;
    let rows = statement.query_map([], |row| {
        Ok(RecentEntry {
            reciter_id: row.get(0)?,
            surah_id: row.get(1)?,
            position_secs: row.get(2)?,
            updated_at: row.get(3)?,
        })
    })?;

    rows.collect()
}

fn load_downloads(connection: &Connection) -> rusqlite::Result<Vec<StoredDownload>> {
    let mut statement = connection.prepare(
        "SELECT reciter_id, reciter_name, surah_id, server, local_path, status,
                bytes_downloaded, total_bytes, error, updated_at
         FROM downloads
         ORDER BY updated_at DESC",
    )?;
    let rows = statement.query_map([], |row| {
        Ok(StoredDownload {
            reciter_id: row.get(0)?,
            reciter_name: row.get(1)?,
            surah_id: row.get(2)?,
            server: row.get(3)?,
            local_path: PathBuf::from(row.get::<_, String>(4)?),
            status: row.get(5)?,
            bytes_downloaded: row.get(6)?,
            total_bytes: row.get(7)?,
            error: row.get(8)?,
            updated_at: row.get(9)?,
        })
    })?;

    rows.collect()
}

pub(crate) fn unix_timestamp() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

#[cfg(test)]
mod tests {
    use super::{
        AppPersistence, FavoriteData, RecentEntry, SettingsSnapshot, StoredDownload, unix_timestamp,
    };
    use std::collections::HashSet;
    use std::path::PathBuf;

    fn test_path(name: &str) -> PathBuf {
        let unique = format!("{}-{}-{}", std::process::id(), name, unix_timestamp());
        std::env::temp_dir().join("quran-tui-tests").join(unique)
    }

    #[test]
    fn snapshot_round_trip_persists_settings_favorites_recent_and_downloads() {
        let persistence = AppPersistence::with_path(test_path("snapshot.sqlite"));
        let settings = SettingsSnapshot {
            mode: Some("listen".to_string()),
            focus: Some("surahs".to_string()),
            settings_field: Some("download_directory".to_string()),
            browse_filter: Some("downloaded".to_string()),
            repeat_mode: Some("all".to_string()),
            selected_reciter_id: Some(7),
            selected_surah: Some(2),
            last_reciter_id: Some(7),
            last_surah: Some(2),
            last_position: 42.5,
            speed: 1.25,
            search_query: "husary".to_string(),
            prefer_offline: false,
            cache_streams: true,
            download_directory: "/tmp/quran-cache".to_string(),
            download_concurrency: 6,
        };
        persistence.save_settings(&settings);

        let favorites = FavoriteData {
            reciters: HashSet::from([7]),
            surahs: HashSet::from([(7, 2)]),
        };
        persistence.save_favorites(&favorites);
        persistence.save_recent(&[RecentEntry {
            reciter_id: 7,
            surah_id: 2,
            position_secs: 42.5,
            updated_at: unix_timestamp(),
        }]);
        persistence.save_downloads(&[StoredDownload {
            reciter_id: 7,
            reciter_name: "Reader".to_string(),
            surah_id: 2,
            server: "https://server6.mp3quran.net/reader/".to_string(),
            local_path: PathBuf::from("/tmp/002.mp3"),
            status: "completed".to_string(),
            bytes_downloaded: 100,
            total_bytes: Some(100),
            error: None,
            updated_at: unix_timestamp(),
        }]);

        let snapshot = persistence.load_snapshot();

        assert_eq!(snapshot.settings, settings);
        assert!(snapshot.favorites.reciters.contains(&7));
        assert!(snapshot.favorites.surahs.contains(&(7, 2)));
        assert_eq!(snapshot.recent.len(), 1);
        assert_eq!(snapshot.downloads.len(), 1);
        assert_eq!(snapshot.downloads[0].status, "completed");
    }

    #[test]
    fn overwriting_recent_keeps_latest_entries() {
        let persistence = AppPersistence::with_path(test_path("recent.sqlite"));
        persistence.save_recent(&[RecentEntry {
            reciter_id: 1,
            surah_id: 1,
            position_secs: 3.0,
            updated_at: 1,
        }]);
        persistence.save_recent(&[RecentEntry {
            reciter_id: 2,
            surah_id: 2,
            position_secs: 9.0,
            updated_at: 2,
        }]);

        let snapshot = persistence.load_snapshot();
        assert_eq!(snapshot.recent.len(), 1);
        assert_eq!(snapshot.recent[0].reciter_id, 2);
    }
}
