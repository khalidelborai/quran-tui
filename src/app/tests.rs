use super::*;
use crate::shaping::shape;

#[cfg(test)]
impl App {
    pub(crate) fn new_for_playback_test() -> Self {
        let mut app = Self::from_player(MpvPlayer::stub_available(), None);
        let test_db_path = std::env::temp_dir().join(format!(
            "quran-tui-playback-test-{}-{}.sqlite",
            std::process::id(),
            unix_timestamp()
        ));
        app.persistence = AppPersistence::with_path(test_db_path);
        app.restored_snapshot = PersistedSnapshot::default();
        app.search_query.clear();
        app.browse_filter = BrowseFilter::All;
        app.repeat_mode = RepeatMode::Off;
        app.settings_field = SettingsField::PreferOffline;
        app.favorites = FavoriteData::default();
        app.recent.clear();
        app.downloads = DownloadManager::with_downloads(Vec::new());
        app.settings = AppSettings {
            prefer_offline_playback: true,
            cache_streams_while_playing: true,
            download_directory: downloads_root().display().to_string(),
            download_concurrency: 4,
        };
        app.player_error = None;
        app.loading = false;
        app.settings_edit_mode = false;
        app.settings_buffer.clear();
        app.mode = Mode::Browse;
        app.focus = Focus::Reciters;
        app.speed = 1.0;
        app.speed_idx = 1;
        app.downloads.set_concurrency(4);
        app
    }

    pub(crate) fn set_test_ayah_texts(&mut self, surah: u32, ayahs: Vec<AyahText>) {
        self.ayah_text_surah = Some(surah);
        self.ayah_display_texts = ayahs.iter().map(|ayah| shape(&ayah.text)).collect();
        self.ayah_texts = ayahs;
        self.ayah_text_status = None;
    }

    pub(crate) fn set_test_current_ayah(&mut self, ayah: u32) {
        self.mushaf.set_current_ayah_for_test(ayah);
    }

    pub(crate) fn refresh_local_media_index_for_test(&mut self) {
        self.refresh_local_media_index();
    }
}

#[cfg(test)]
mod regression_tests {
    use super::{App, AyahTextLoadResult, BrowseFilter, RepeatMode};
    use crate::api::{AyahText, Moshaf, Reciter, Surah};
    use crate::persistence::unix_timestamp;
    use crate::shaping::shape;
    use std::path::PathBuf;

    fn sample_reciters() -> Vec<Reciter> {
        vec![
            Reciter {
                _id: 1,
                name: "Reader One".to_string(),
                moshaf: vec![Moshaf {
                    _id: 11,
                    _name: "Hafs".to_string(),
                    server: "https://server6.mp3quran.net/akdr/".to_string(),
                    surah_total: 2,
                    surah_list: "1,2".to_string(),
                }],
            },
            Reciter {
                _id: 2,
                name: "Reader Two".to_string(),
                moshaf: vec![Moshaf {
                    _id: 22,
                    _name: "Hafs".to_string(),
                    server: "https://server9.mp3quran.net/other/".to_string(),
                    surah_total: 1,
                    surah_list: "2".to_string(),
                }],
            },
        ]
    }

    fn sample_surahs() -> Vec<Surah> {
        vec![
            Surah {
                id: 1,
                name: "Fatiha".to_string(),
            },
            Surah {
                id: 2,
                name: "Baqara".to_string(),
            },
        ]
    }

    fn temp_download_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "quran-tui-{}-{}-{}",
            std::process::id(),
            unix_timestamp(),
            name
        ));
        let _ = std::fs::create_dir_all(&dir);
        dir
    }

    #[test]
    fn stale_ayah_text_result_is_ignored() {
        let mut app = App::new_for_test();
        app.latest_ayah_text_request_id = Some(2);
        let (tx, rx) = std::sync::mpsc::channel();
        tx.send(AyahTextLoadResult {
            request_id: 1,
            surah: 1,
            ayahs: vec![],
        })
        .expect("send test result");
        app.ayah_text_rx = Some(rx);

        app.poll_surah_text();

        assert_eq!(app.ayah_text_surah, None);
        assert!(app.ayah_texts.is_empty());
    }

    #[test]
    fn search_filters_reciters_and_surahs() {
        let mut app = App::new_for_test();
        app.loading = false;
        app.set_library_data(sample_reciters(), sample_surahs());
        app.search_query = "baq".to_string();

        let visible = app.visible_reciter_indices();
        assert_eq!(visible, vec![0, 1]);
        assert_eq!(app.selected_surah_list(), vec![2]);
    }

    #[test]
    fn favorites_filter_limits_visible_entries() {
        let mut app = App::new_for_test();
        app.loading = false;
        app.set_library_data(sample_reciters(), sample_surahs());
        app.browse_filter = BrowseFilter::Favorites;
        app.toggle_favorite();

        let visible = app.visible_reciter_indices();
        assert_eq!(visible, vec![0]);
    }

    #[test]
    fn repeat_mode_cycles() {
        let mut app = App::new_for_test();
        assert_eq!(app.repeat_mode, RepeatMode::Off);
        app.cycle_repeat_mode();
        assert_eq!(app.repeat_mode, RepeatMode::One);
        app.cycle_repeat_mode();
        assert_eq!(app.repeat_mode, RepeatMode::All);
    }

    #[test]
    fn study_selection_tracks_current_ayah_text() {
        let mut app = App::new_for_test();
        app.loading = false;
        app.set_library_data(sample_reciters(), sample_surahs());
        app.set_test_current_ayah(2);
        app.set_test_ayah_texts(
            1,
            vec![
                AyahText {
                    ayah: 1,
                    text: "One".to_string(),
                },
                AyahText {
                    ayah: 2,
                    text: "Two".to_string(),
                },
            ],
        );
        app.selected_ayah_index = 1;

        assert_eq!(app.study_selected_ayah(), Some(2));
    }

    #[test]
    fn study_ayah_display_text_uses_same_shape_pipeline() {
        let mut app = App::new_for_test();
        let raw_text = "\u{feff}  ٱلْحَمْدُ لِلَّهِ  ";
        let expected = shape(raw_text);
        app.set_test_ayah_texts(
            1,
            vec![AyahText {
                ayah: 1,
                text: raw_text.to_string(),
            }],
        );

        assert_eq!(app.study_ayah_display_text(0), Some(expected.as_str()));
    }

    #[test]
    fn reciter_and_surah_display_names_use_same_shape_pipeline() {
        let mut app = App::new_for_test();
        app.loading = false;
        let reciter_name = "مِشَارِي";
        let surah_name = "ٱلْفَاتِحَة";
        app.set_library_data(
            vec![Reciter {
                _id: 1,
                name: reciter_name.to_string(),
                moshaf: vec![Moshaf {
                    _id: 11,
                    _name: "Hafs".to_string(),
                    server: "https://server6.mp3quran.net/akdr/".to_string(),
                    surah_total: 1,
                    surah_list: "1".to_string(),
                }],
            }],
            vec![Surah {
                id: 1,
                name: surah_name.to_string(),
            }],
        );

        let expected_reciter = shape(reciter_name);
        let expected_surah = shape(surah_name);
        assert_eq!(app.reciter_display_name(0), Some(expected_reciter.as_str()));
        assert_eq!(app.surah_display_name(1), Some(expected_surah.as_str()));
    }

    #[test]
    fn detects_offline_file_from_reciter_folder() {
        let mut app = App::new_for_test();
        app.loading = false;
        app.set_library_data(sample_reciters(), sample_surahs());
        let root = temp_download_dir("offline");
        app.settings.download_directory = root.display().to_string();
        app.refresh_local_media_index_for_test();
        let path = app.expected_local_path(1, "Reader One", 1);
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        std::fs::write(&path, b"offline").expect("write offline file");

        assert!(app.has_downloaded_surah(1, "Reader One", 1));
        assert_eq!(app.offline_path_for(1, "Reader One", 1), Some(path));
    }

    #[test]
    fn settings_edit_updates_download_directory() {
        let mut app = App::new_for_test();
        let target = temp_download_dir("settings");
        app.open_settings();
        app.settings_field = super::SettingsField::DownloadDirectory;
        app.activate_settings_field();
        app.settings_buffer = target.display().to_string();
        app.commit_settings_edit();

        assert_eq!(
            app.settings.download_directory,
            target.display().to_string()
        );
    }

    #[test]
    fn scan_detects_legacy_named_mp3_files() {
        let mut app = App::new_for_test();
        app.loading = false;
        app.set_library_data(sample_reciters(), sample_surahs());
        let root = temp_download_dir("legacy-scan");
        app.settings.download_directory = root.display().to_string();
        let legacy_dir = root.join("Reader One");
        std::fs::create_dir_all(&legacy_dir).expect("create legacy folder");
        let legacy_path = legacy_dir.join("001 - Al-Fatiha.mp3");
        std::fs::write(&legacy_path, b"legacy").expect("write legacy mp3");

        app.refresh_local_media_index_for_test();

        assert!(app.has_downloaded_surah(1, "Reader One", 1));
        assert_eq!(app.offline_path_for(1, "Reader One", 1), Some(legacy_path));
    }

    #[test]
    fn changing_download_directory_migrates_known_files() {
        let mut app = App::new_for_test();
        app.loading = false;
        app.set_library_data(sample_reciters(), sample_surahs());
        let old_root = temp_download_dir("old-downloads");
        let new_root = temp_download_dir("new-downloads");
        app.settings.download_directory = old_root.display().to_string();

        let old_path = app.expected_local_path(1, "Reader One", 1);
        std::fs::create_dir_all(old_path.parent().expect("parent")).expect("create old parent");
        std::fs::write(&old_path, b"offline").expect("write old download");
        app.refresh_local_media_index_for_test();

        app.open_settings();
        app.settings_field = super::SettingsField::DownloadDirectory;
        app.activate_settings_field();
        app.settings_buffer = new_root.display().to_string();
        app.commit_settings_edit();

        let new_path = crate::downloads::destination_path(&new_root, 1, "Reader One", 1);
        assert!(!old_path.exists());
        assert!(new_path.exists());
        assert_eq!(app.offline_path_for(1, "Reader One", 1), Some(new_path));
    }

    #[test]
    fn streaming_playback_records_to_local_file_without_queueing_download_job() {
        let mut app = App::new_for_playback_test();
        let root = temp_download_dir("stream-record");
        app.settings.download_directory = root.display().to_string();
        app.settings.prefer_offline_playback = false;
        app.settings.cache_streams_while_playing = true;
        app.set_library_data(sample_reciters(), sample_surahs());

        app.play_selected();

        let expected = app.expected_local_path(1, "Reader One", 1);
        assert_eq!(app.current_source_label(), Some("Streaming"));
        assert!(app.download_preview(5).is_empty());
        assert_eq!(app.player.test_record_path(), Some(expected));
    }

    #[test]
    fn shutdown_finalizes_recorded_stream_into_offline_index() {
        let mut app = App::new_for_playback_test();
        let root = temp_download_dir("record-finish");
        app.settings.download_directory = root.display().to_string();
        app.settings.prefer_offline_playback = false;
        app.settings.cache_streams_while_playing = true;
        app.set_library_data(sample_reciters(), sample_surahs());

        app.play_selected();
        let expected = app.expected_local_path(1, "Reader One", 1);
        std::fs::create_dir_all(expected.parent().expect("parent")).expect("create record dir");
        std::fs::write(&expected, b"captured stream").expect("write captured stream");

        app.shutdown();

        assert!(app.has_downloaded_surah(1, "Reader One", 1));
        assert_eq!(app.download_status_label(1, 1), Some("offline".to_string()));
        assert_eq!(app.player.test_record_path(), None);
    }
}
