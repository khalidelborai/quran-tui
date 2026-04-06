use super::*;

impl App {
    pub(super) fn recent_position(&self, reciter_id: u32, surah_id: u32) -> Option<f64> {
        self.recent
            .iter()
            .find(|entry| entry.reciter_id == reciter_id && entry.surah_id == surah_id)
            .map(|entry| entry.position_secs)
    }

    pub(super) fn record_recent(&mut self, reciter_id: u32, surah_id: u32, position_secs: f64) {
        let now = unix_timestamp();
        if let Some(entry) = self
            .recent
            .iter_mut()
            .find(|entry| entry.reciter_id == reciter_id && entry.surah_id == surah_id)
        {
            entry.position_secs = position_secs;
            entry.updated_at = now;
        } else {
            self.recent.push(RecentEntry {
                reciter_id,
                surah_id,
                position_secs,
                updated_at: now,
            });
        }
        self.recent
            .sort_by(|left, right| right.updated_at.cmp(&left.updated_at));
        self.recent.truncate(10);
        self.persistence.save_recent(&self.recent);
    }

    pub(crate) fn sync_downloads(&mut self) {
        self.downloads.poll();
        self.persist_downloads();
    }

    pub(super) fn persist_downloads(&mut self) {
        if let Some(downloads) = self.downloads.take_persisted_downloads() {
            self.persistence.save_downloads(&downloads);
        }
    }

    pub(crate) fn persist_settings(&self) {
        let settings = SettingsSnapshot {
            mode: Some(self.mode.as_str().to_string()),
            focus: Some(self.focus.as_str().to_string()),
            settings_field: Some(self.settings_field.as_str().to_string()),
            browse_filter: Some(self.browse_filter.as_str().to_string()),
            repeat_mode: Some(self.repeat_mode.as_str().to_string()),
            selected_reciter_id: self.selected_reciter_id(),
            selected_surah: self.selected_surah_number(),
            last_reciter_id: self
                .playing_reciter
                .and_then(|index| self.reciters.get(index).map(|reciter| reciter._id)),
            last_surah: self.playing_surah,
            last_position: self.position,
            speed: self.speed,
            search_query: self.search_query.clone(),
            prefer_offline: self.settings.prefer_offline_playback,
            cache_streams: self.settings.cache_streams_while_playing,
            download_directory: self.settings.download_directory.clone(),
            download_concurrency: self.settings.download_concurrency,
        };
        self.persistence.save_settings(&settings);
    }

    pub(crate) fn shutdown(&mut self) {
        self.finish_active_stream_recording();
        self.persist_settings();
        self.persistence.save_recent(&self.recent);
        self.persistence.save_favorites(&self.favorites);
        self.persist_downloads();
        self.player.stop();
    }

    pub(crate) fn queue_len(&self) -> usize {
        self.queue.len()
    }

    pub(crate) fn track_notice(&self) -> Option<&str> {
        self.notice.as_deref()
    }

    pub(crate) fn should_handle_second_g(&self) -> bool {
        self.pending_g
    }

    pub(crate) fn mark_pending_g(&mut self, pending: bool) {
        self.pending_g = pending;
    }
}
