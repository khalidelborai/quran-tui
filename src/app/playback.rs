use super::*;

impl App {
    pub(crate) fn play_selected(&mut self) {
        let Some(moshaf) = self.selected_moshaf().cloned() else {
            return;
        };
        let visible_surahs = self.selected_surah_list();
        let Some(surah_num) = visible_surahs.get(self.selected_surah).copied() else {
            return;
        };

        self.queue = visible_surahs
            .iter()
            .copied()
            .map(|surah_number| QueueItem {
                reciter_index: self.selected_reciter,
                reciter_id: self.reciters[self.selected_reciter]._id,
                surah_number,
                server: moshaf.server.clone(),
            })
            .collect();
        self.queue_index = Some(self.selected_surah);
        let resume_position =
            self.recent_position(self.reciters[self.selected_reciter]._id, surah_num);
        self.play_current_queue_item(resume_position);
    }

    pub(super) fn play_current_queue_item(&mut self, resume_position: Option<f64>) {
        self.finish_active_stream_recording();

        let Some(index) = self.queue_index else {
            return;
        };
        let Some(item) = self.queue.get(index).cloned() else {
            return;
        };
        self.error = None;
        self.notice = None;
        if let Some(message) = self.player.startup_error() {
            self.player_error = Some(message.to_string());
            self.mushaf.clear_timing("Audio player unavailable");
            self.ayah_text_status = Some("Audio player unavailable".to_string());
            warn!(error = %message, "Playback requested while mpv is unavailable");
            return;
        }

        let reciter_name = self
            .reciters
            .get(item.reciter_index)
            .map(|reciter| reciter.name.clone())
            .unwrap_or_else(|| "reciter".to_string());
        let source_label = self.start_playback_for_item(&item, &reciter_name);

        if let Some(position) = resume_position.filter(|position| *position > 3.0) {
            self.player.seek_absolute(position);
            self.position = position;
        } else {
            self.position = 0.0;
        }
        self.duration = 0.0;
        self.playing_reciter = Some(item.reciter_index);
        self.playing_surah = Some(item.surah_number);
        self.mode = Mode::Listen;
        self.last_saved_position_bucket = -1;

        self.mushaf.timing_status = None;
        if let Some(read_id) = self.mushaf.find_read_id(&item.server) {
            self.mushaf.load_timing_async(item.surah_number, read_id);
        } else {
            self.mushaf
                .clear_timing("Timing not available for this reciter");
        }
        self.load_surah_text_async(item.surah_number);
        self.record_recent(
            item.reciter_id,
            item.surah_number,
            resume_position.unwrap_or_default(),
        );
        trace!(
            source = source_label,
            surah = item.surah_number,
            "Playback started"
        );
        self.persist_settings();
    }

    fn maybe_cache_stream(
        &mut self,
        reciter_id: u32,
        reciter_name: &str,
        surah_id: u32,
        server: &str,
    ) {
        if !self.settings.cache_streams_while_playing {
            return;
        }
        let local_path = self.expected_local_path(reciter_id, reciter_name, surah_id);
        if local_path.exists() {
            self.register_completed_local_file(
                reciter_id,
                reciter_name,
                surah_id,
                server,
                local_path,
            );
            return;
        }
        if let Some(parent) = local_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        self.player.set_stream_record(&local_path);
        self.active_stream_recording = Some(ActiveStreamRecording {
            reciter_id,
            reciter_name: reciter_name.to_string(),
            surah_id,
            server: server.to_string(),
            path: local_path,
        });
        self.notice = Some(format!("Saving {:03} while streaming", surah_id));
    }

    fn start_playback_for_item(&mut self, item: &QueueItem, reciter_name: &str) -> &'static str {
        if self.settings.prefer_offline_playback
            && let Some(path) =
                self.offline_path_for(item.reciter_id, reciter_name, item.surah_number)
        {
            self.player.play_path(&path);
            self.current_source_label = Some("Offline".to_string());
            self.notice = Some(format!("Playing local file {}", path.display()));
            return "offline";
        }

        let url = format!("{}{:03}.mp3", item.server, item.surah_number);
        self.player.play_url(&url);
        self.current_source_label = Some("Streaming".to_string());
        self.notice = Some("Streaming from mp3quran".to_string());
        self.maybe_cache_stream(
            item.reciter_id,
            reciter_name,
            item.surah_number,
            &item.server,
        );
        "streaming"
    }

    pub(crate) fn play_next(&mut self) {
        if self.queue.is_empty() {
            return;
        }
        let next_index = match self.queue_index {
            Some(index) if index + 1 < self.queue.len() => Some(index + 1),
            Some(_) if self.repeat_mode == RepeatMode::All => Some(0),
            _ => None,
        };
        if let Some(index) = next_index {
            self.queue_index = Some(index);
            self.play_current_queue_item(None);
        } else {
            self.finish_active_stream_recording();
        }
    }

    pub(crate) fn play_previous(&mut self) {
        if self.position > 5.0 {
            self.player.seek_absolute(0.0);
            self.position = 0.0;
            return;
        }
        let previous_index = match self.queue_index {
            Some(index) if index > 0 => Some(index - 1),
            Some(_) if self.repeat_mode == RepeatMode::All && !self.queue.is_empty() => {
                Some(self.queue.len().saturating_sub(1))
            }
            _ => None,
        };
        if let Some(index) = previous_index {
            self.queue_index = Some(index);
            self.play_current_queue_item(None);
        }
    }

    pub(crate) fn cycle_repeat_mode(&mut self) {
        self.repeat_mode = self.repeat_mode.next();
        self.notice = Some(self.repeat_mode.label().to_string());
        self.persist_settings();
    }

    pub(crate) fn queue_selected_download(&mut self) {
        let Some(reciter) = self.reciters.get(self.selected_reciter) else {
            return;
        };
        let Some(moshaf) = reciter.moshaf.first() else {
            return;
        };
        let reciter_id = reciter._id;
        let reciter_name = reciter.name.clone();
        let server = moshaf.server.clone();
        let Some(surah_id) = self.selected_surah_number() else {
            return;
        };
        if self.enqueue_download(reciter_id, &reciter_name, surah_id, &server) {
            self.notice = Some(format!("Queued download for {:03}", surah_id));
            self.persist_downloads();
        }
    }

    pub(crate) fn queue_selected_reciter_downloads(&mut self) {
        let Some(reciter) = self.reciters.get(self.selected_reciter) else {
            return;
        };
        let Some(moshaf) = reciter.moshaf.first() else {
            return;
        };
        let reciter_id = reciter._id;
        let reciter_name = reciter.name.clone();
        let server = moshaf.server.clone();
        let surahs = self.selected_surah_list();
        let mut queued = 0;
        for surah_id in surahs {
            if self.enqueue_download(reciter_id, &reciter_name, surah_id, &server) {
                queued += 1;
            }
        }
        if queued > 0 {
            self.notice = Some(format!("Queued {queued} download(s)"));
            self.persist_downloads();
        }
    }

    fn enqueue_download(
        &mut self,
        reciter_id: u32,
        reciter_name: &str,
        surah_id: u32,
        server: &str,
    ) -> bool {
        self.downloads.enqueue(DownloadRequest {
            reciter_id,
            reciter_name: reciter_name.to_string(),
            surah_id,
            server: server.to_string(),
            local_path: self.expected_local_path(reciter_id, reciter_name, surah_id),
        })
    }

    pub(crate) fn cancel_selected_download(&mut self) {
        let Some(reciter_id) = self.selected_reciter_id() else {
            return;
        };
        let Some(surah_id) = self.selected_surah_number() else {
            return;
        };
        if self.downloads.cancel(reciter_id, surah_id) {
            self.notice = Some(format!("Cancelled download {:03}", surah_id));
            self.persist_downloads();
        }
    }

    pub(crate) fn retry_selected_download(&mut self) {
        let Some(reciter_id) = self.selected_reciter_id() else {
            return;
        };
        let Some(surah_id) = self.selected_surah_number() else {
            return;
        };
        if self.downloads.retry(reciter_id, surah_id) {
            self.notice = Some(format!("Retrying download {:03}", surah_id));
            self.persist_downloads();
        }
    }
}
