use super::*;

impl App {
    pub(crate) fn new() -> Self {
        let mut player = MpvPlayer::new();
        let player_error = player.start().err();
        Self::from_player(player, player_error)
    }

    #[cfg(test)]
    pub(crate) fn new_for_test() -> Self {
        let mut app = Self::from_player(
            MpvPlayer::stub_unavailable(),
            Some("test stub player unavailable".to_string()),
        );
        let test_db_path = std::env::temp_dir().join(format!(
            "quran-tui-test-{}-{}.sqlite",
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
        app.settings_edit_mode = false;
        app.settings_buffer.clear();
        app.mode = Mode::Browse;
        app.focus = Focus::Reciters;
        app.speed = 1.0;
        app.speed_idx = 1;
        app.downloads.set_concurrency(4);
        app
    }

    pub(super) fn from_player(player: MpvPlayer, player_error: Option<String>) -> Self {
        let persistence = AppPersistence::new();
        let restored_snapshot = persistence.load_snapshot();
        let speed = if restored_snapshot.settings.speed > 0.0 {
            restored_snapshot.settings.speed
        } else {
            1.0
        };
        let speeds = vec![0.75, 1.0, 1.25, 1.5, 2.0];
        let speed_idx = nearest_speed_index(&speeds, speed);
        let browse_filter =
            BrowseFilter::from_str(restored_snapshot.settings.browse_filter.as_deref());
        let repeat_mode = RepeatMode::from_str(restored_snapshot.settings.repeat_mode.as_deref());
        let mode = Mode::from_str(restored_snapshot.settings.mode.as_deref());
        let focus = Focus::from_str(restored_snapshot.settings.focus.as_deref());
        let settings_field =
            SettingsField::from_str(restored_snapshot.settings.settings_field.as_deref());
        let favorites = restored_snapshot.favorites.clone();
        let recent = restored_snapshot.recent.clone();
        let download_directory = if restored_snapshot.settings.download_directory.is_empty() {
            downloads_root().display().to_string()
        } else {
            restored_snapshot.settings.download_directory.clone()
        };
        let settings = AppSettings {
            prefer_offline_playback: restored_snapshot.settings.prefer_offline,
            cache_streams_while_playing: restored_snapshot.settings.cache_streams,
            download_directory,
            download_concurrency: restored_snapshot.settings.download_concurrency.clamp(1, 8),
        };
        let mut downloads = DownloadManager::with_downloads(restored_snapshot.downloads.clone());
        downloads.set_concurrency(settings.download_concurrency);

        Self {
            reciters: vec![],
            surahs: vec![],
            reciter_display_names: vec![],
            reciter_surah_lists: vec![],
            surah_display_names: HashMap::new(),
            selected_reciter: 0,
            selected_surah: 0,
            reciter_scroll: 0,
            surah_scroll: 0,
            reciter_viewport_height: 0,
            surah_viewport_height: 0,
            mode,
            focus,
            settings_field,
            player,
            playing_reciter: None,
            playing_surah: None,
            position: 0.0,
            duration: 0.0,
            speed,
            speeds,
            speed_idx,
            loading: true,
            error: None,
            player_error,
            notice: None,
            mushaf: MushafWidget::new(),
            ayah_text_panel: AyahTextPanel::new(),
            ayah_texts: vec![],
            ayah_display_texts: vec![],
            ayah_text_status: None,
            ayah_text_surah: None,
            ayah_text_request_id: 0,
            latest_ayah_text_request_id: None,
            ayah_text_rx: None,
            search_query: restored_snapshot.settings.search_query.clone(),
            search_mode: false,
            browse_filter,
            repeat_mode,
            settings,
            settings_edit_mode: false,
            settings_buffer: String::new(),
            favorites,
            recent,
            queue: vec![],
            queue_index: None,
            current_source_label: None,
            persistence,
            restored_snapshot,
            downloads,
            local_media_index: HashMap::new(),
            active_stream_recording: None,
            selected_ayah_index: 0,
            study_scroll: 0,
            study_viewport_height: 0,
            repeat_current_ayah: false,
            loop_range: None,
            loop_latch_end_ms: None,
            last_saved_position_bucket: -1,
            pending_g: false,
        }
    }

    pub(crate) fn set_library_data(&mut self, reciters: Vec<Reciter>, surahs: Vec<Surah>) {
        self.reciter_display_names = reciters
            .iter()
            .map(|reciter| shape(&reciter.name))
            .collect();
        self.reciter_surah_lists = reciters
            .iter()
            .map(|reciter| {
                reciter
                    .moshaf
                    .first()
                    .map(|moshaf| parse_surah_list(&moshaf.surah_list))
                    .unwrap_or_default()
            })
            .collect();
        self.surah_display_names = surahs
            .iter()
            .map(|surah| (surah.id, shape(&surah.name)))
            .collect();
        self.reciters = reciters;
        self.surahs = surahs;
        self.restore_from_snapshot();
        self.refresh_local_media_index();
        self.player.set_speed(self.speed);
        self.persist_settings();
    }

    fn restore_from_snapshot(&mut self) {
        if self.reciters.is_empty() {
            return;
        }

        if let Some(reciter_id) = self.restored_snapshot.settings.selected_reciter_id
            && let Some(index) = self.reciter_index_by_id(reciter_id)
        {
            self.selected_reciter = index;
        }

        self.ensure_valid_selection();

        if let Some(surah_number) = self.restored_snapshot.settings.selected_surah
            && let Some(index) = self
                .selected_surah_list()
                .iter()
                .position(|candidate| *candidate == surah_number)
        {
            self.selected_surah = index;
        }

        self.ensure_valid_selection();
    }

    pub(super) fn selected_moshaf(&self) -> Option<&Moshaf> {
        self.current_moshaf(self.selected_reciter)
    }

    fn current_moshaf(&self, reciter_index: usize) -> Option<&Moshaf> {
        self.reciters.get(reciter_index)?.moshaf.first()
    }

    fn reciter_index_by_id(&self, reciter_id: u32) -> Option<usize> {
        self.reciters
            .iter()
            .position(|reciter| reciter._id == reciter_id)
    }

    pub(crate) fn selected_reciter_id(&self) -> Option<u32> {
        self.reciters
            .get(self.selected_reciter)
            .map(|reciter| reciter._id)
    }

    pub(crate) fn set_mode(&mut self, mode: Mode) {
        self.mode = mode;
        self.persist_settings();
    }

    pub(crate) fn selected_surah_list(&self) -> Vec<u32> {
        let Some(reciter_id) = self.selected_reciter_id() else {
            return Vec::new();
        };
        let base = self
            .reciter_surah_lists
            .get(self.selected_reciter)
            .cloned()
            .unwrap_or_default();
        let query = normalize_query(&self.search_query);
        base.into_iter()
            .filter(|surah_id| {
                self.matches_surah_filter(reciter_id, self.selected_reciter, *surah_id, &query)
            })
            .collect()
    }

    pub(crate) fn visible_reciter_indices(&self) -> Vec<usize> {
        let query = normalize_query(&self.search_query);
        self.reciters
            .iter()
            .enumerate()
            .filter(|(index, reciter)| self.matches_reciter_filter(*index, reciter, &query))
            .map(|(index, _)| index)
            .collect()
    }

    fn reciter_has_timing(&self, reciter_index: usize) -> bool {
        self.current_moshaf(reciter_index)
            .map(|moshaf| self.mushaf.find_read_id(&moshaf.server).is_some())
            .unwrap_or(false)
    }

    fn matches_surah_query(&self, surah_id: u32, query: &str) -> bool {
        if query.is_empty() {
            return true;
        }
        let name_matches = self
            .surahs
            .iter()
            .find(|surah| surah.id == surah_id)
            .map(|surah| normalize_query(&surah.name).contains(query))
            .unwrap_or(false);
        name_matches || surah_id.to_string().contains(query)
    }

    fn matches_surah_filter(
        &self,
        reciter_id: u32,
        reciter_index: usize,
        surah_id: u32,
        query: &str,
    ) -> bool {
        let query_matches = query.is_empty() || self.matches_surah_query(surah_id, query);
        let filter_matches = match self.browse_filter {
            BrowseFilter::All | BrowseFilter::HasTiming => true,
            BrowseFilter::Favorites => {
                self.favorites.reciters.contains(&reciter_id)
                    || self.favorites.surahs.contains(&(reciter_id, surah_id))
            }
            BrowseFilter::Downloaded => self
                .reciters
                .get(reciter_index)
                .map(|reciter| self.has_downloaded_surah(reciter_id, &reciter.name, surah_id))
                .unwrap_or(false),
            BrowseFilter::Recent => self
                .recent
                .iter()
                .any(|entry| entry.reciter_id == reciter_id && entry.surah_id == surah_id),
        };
        query_matches && filter_matches
    }

    fn matches_reciter_filter(&self, reciter_index: usize, reciter: &Reciter, query: &str) -> bool {
        let filter_matches = match self.browse_filter {
            BrowseFilter::All => true,
            BrowseFilter::Favorites => {
                self.favorites.reciters.contains(&reciter._id)
                    || self
                        .favorites
                        .surahs
                        .iter()
                        .any(|(reciter_id, _)| *reciter_id == reciter._id)
            }
            BrowseFilter::Downloaded => self.reciter_downloaded_count(reciter_index) > 0,
            BrowseFilter::HasTiming => self.reciter_has_timing(reciter_index),
            BrowseFilter::Recent => self
                .recent
                .iter()
                .any(|entry| entry.reciter_id == reciter._id),
        };
        let query_matches = query.is_empty()
            || normalize_query(&reciter.name).contains(query)
            || self
                .reciter_surah_lists
                .get(reciter_index)
                .is_some_and(|surah_ids| {
                    surah_ids
                        .iter()
                        .any(|surah_id| self.matches_surah_query(*surah_id, query))
                });
        filter_matches && query_matches
    }

    pub(super) fn ensure_valid_selection(&mut self) {
        let visible_reciters = self.visible_reciter_indices();
        if visible_reciters.is_empty() {
            self.selected_reciter = 0;
            self.selected_surah = 0;
            self.reciter_scroll = 0;
            self.surah_scroll = 0;
            return;
        }
        if !visible_reciters.contains(&self.selected_reciter) {
            self.selected_reciter = visible_reciters[0];
            self.reciter_scroll = 0;
        }
        let surahs = self.selected_surah_list();
        if surahs.is_empty() {
            self.selected_surah = 0;
            self.surah_scroll = 0;
        } else {
            self.selected_surah = self.selected_surah.min(surahs.len().saturating_sub(1));
        }
    }

    pub(crate) fn reciter_display_name(&self, index: usize) -> Option<&str> {
        self.reciter_display_names.get(index).map(String::as_str)
    }

    pub(crate) fn surah_display_name(&self, surah_id: u32) -> Option<&str> {
        self.surah_display_names.get(&surah_id).map(String::as_str)
    }

    pub(crate) fn selected_surah_number(&self) -> Option<u32> {
        self.selected_surah_list().get(self.selected_surah).copied()
    }

    pub(crate) fn playing_surah_number(&self) -> Option<u32> {
        self.playing_surah
    }

    pub(crate) fn reciter_is_favorite(&self, reciter_id: u32) -> bool {
        self.favorites.reciters.contains(&reciter_id)
    }

    pub(crate) fn surah_is_favorite(&self, reciter_id: u32, surah_id: u32) -> bool {
        self.favorites.surahs.contains(&(reciter_id, surah_id))
    }

    pub(crate) fn active_recent(&self) -> &[RecentEntry] {
        &self.recent
    }

    pub(crate) fn current_source_label(&self) -> Option<&str> {
        self.current_source_label.as_deref()
    }

    pub(crate) fn download_root_path(&self) -> PathBuf {
        PathBuf::from(&self.settings.download_directory)
    }

    pub(crate) fn expected_local_path(
        &self,
        reciter_id: u32,
        reciter_name: &str,
        surah_id: u32,
    ) -> PathBuf {
        self.expected_local_path_in_root(
            &self.download_root_path(),
            reciter_id,
            reciter_name,
            surah_id,
        )
    }

    fn expected_local_path_in_root(
        &self,
        root: &Path,
        reciter_id: u32,
        reciter_name: &str,
        surah_id: u32,
    ) -> PathBuf {
        crate::downloads::destination_path(root, reciter_id, reciter_name, surah_id)
    }

    pub(crate) fn offline_path_for(
        &self,
        reciter_id: u32,
        reciter_name: &str,
        surah_id: u32,
    ) -> Option<PathBuf> {
        let root = self.download_root_path();
        if let Some(path) = self.local_media_index.get(&(reciter_id, surah_id))
            && path.exists()
        {
            return Some(path.clone());
        }
        self.downloads
            .local_path_for(reciter_id, surah_id)
            .filter(|path| path.exists() && path.starts_with(&root))
            .or_else(|| {
                let path = self.expected_local_path(reciter_id, reciter_name, surah_id);
                path.exists().then_some(path)
            })
    }

    pub(super) fn refresh_local_media_index(&mut self) {
        let root = self.download_root_path();
        let discovered = self.discover_local_media(&root);
        self.local_media_index = discovered
            .iter()
            .map(|record| {
                (
                    (record.reciter_id, record.surah_id),
                    record.local_path.clone(),
                )
            })
            .collect();
        self.downloads.sync_local_files(&discovered);
        self.persist_downloads();
    }

    fn discover_local_media(&self, root: &Path) -> Vec<LocalFileRecord> {
        if self.reciters.is_empty() || !root.exists() {
            return Vec::new();
        }

        let mut files = Vec::new();
        let mut pending = vec![root.to_path_buf()];
        while let Some(dir) = pending.pop() {
            let Ok(entries) = std::fs::read_dir(&dir) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    pending.push(path);
                } else if path
                    .extension()
                    .is_some_and(|extension| extension.eq_ignore_ascii_case("mp3"))
                {
                    files.push(path);
                }
            }
        }

        let mut discovered = HashMap::<(u32, u32), LocalFileRecord>::new();
        for path in files {
            let Some(surah_id) = parse_surah_id_from_path(&path) else {
                continue;
            };
            let Some(reciter_index) = self.reciter_index_for_media_path(&path) else {
                continue;
            };
            let Some(reciter) = self.reciters.get(reciter_index) else {
                continue;
            };
            let Some(moshaf) = reciter.moshaf.first() else {
                continue;
            };

            discovered.insert(
                (reciter._id, surah_id),
                LocalFileRecord {
                    reciter_id: reciter._id,
                    reciter_name: reciter.name.clone(),
                    surah_id,
                    server: moshaf.server.clone(),
                    local_path: path,
                },
            );
        }

        discovered.into_values().collect()
    }

    fn reciter_index_for_media_path(&self, path: &Path) -> Option<usize> {
        let folder_name = path.parent()?.file_name()?.to_string_lossy();
        let normalized_folder = normalize_media_component(&folder_name);

        self.reciters
            .iter()
            .enumerate()
            .find_map(|(index, reciter)| {
                let expected_folder = self
                    .expected_local_path(reciter._id, &reciter.name, 1)
                    .parent()
                    .and_then(|path| path.file_name())
                    .map(|name| normalize_media_component(&name.to_string_lossy()))
                    .unwrap_or_default();
                let normalized_name = normalize_media_component(&reciter.name);
                let id_token = format!("{:04}", reciter._id);

                if normalized_folder == expected_folder
                    || normalized_folder.starts_with(&format!("{id_token}-"))
                    || normalized_folder == id_token
                    || (!normalized_name.is_empty()
                        && (normalized_folder == normalized_name
                            || normalized_folder.contains(&normalized_name)))
                {
                    Some(index)
                } else {
                    None
                }
            })
    }

    pub(super) fn migrate_download_directory(&mut self, new_root: &Path) -> usize {
        let old_records = self.discover_local_media(&self.download_root_path());
        let mut migrated = 0;

        for record in old_records {
            let target = self.expected_local_path_in_root(
                new_root,
                record.reciter_id,
                &record.reciter_name,
                record.surah_id,
            );
            if record.local_path == target {
                continue;
            }

            if let Some(parent) = target.parent() {
                let _ = std::fs::create_dir_all(parent);
            }

            match std::fs::rename(&record.local_path, &target) {
                Ok(()) => migrated += 1,
                Err(_) => {
                    if std::fs::copy(&record.local_path, &target).is_ok()
                        && std::fs::remove_file(&record.local_path).is_ok()
                    {
                        migrated += 1;
                    }
                }
            }
        }

        migrated
    }

    pub(super) fn register_completed_local_file(
        &mut self,
        reciter_id: u32,
        reciter_name: &str,
        surah_id: u32,
        server: &str,
        path: PathBuf,
    ) {
        self.local_media_index
            .insert((reciter_id, surah_id), path.clone());
        self.downloads.reconcile_local_file(LocalFileRecord {
            reciter_id,
            reciter_name: reciter_name.to_string(),
            surah_id,
            server: server.to_string(),
            local_path: path,
        });
        self.persist_downloads();
    }

    pub(super) fn finish_active_stream_recording(&mut self) {
        let Some(active) = self.active_stream_recording.take() else {
            return;
        };
        self.player.clear_stream_record();
        if active.path.exists() {
            self.register_completed_local_file(
                active.reciter_id,
                &active.reciter_name,
                active.surah_id,
                &active.server,
                active.path,
            );
        }
    }
}
