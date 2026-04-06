use super::*;

impl App {
    pub(crate) fn move_study_selection(&mut self, delta: isize) {
        if self.ayah_texts.is_empty() {
            return;
        }
        self.selected_ayah_index = self
            .selected_ayah_index
            .saturating_add_signed(delta)
            .min(self.ayah_texts.len().saturating_sub(1));
        self.study_scroll = adjust_scroll(
            self.selected_ayah_index,
            self.study_scroll,
            self.study_viewport_height,
        );
    }

    pub(crate) fn study_selected_ayah(&self) -> Option<u32> {
        self.ayah_texts
            .get(self.selected_ayah_index)
            .map(|ayah| ayah.ayah)
    }

    pub(crate) fn jump_to_selected_ayah(&mut self) {
        let Some(ayah) = self.study_selected_ayah() else {
            return;
        };
        if let Some((start_ms, _)) = self.mushaf.ayah_bounds(ayah) {
            self.player.seek_absolute(start_ms as f64 / 1000.0);
            self.position = start_ms as f64 / 1000.0;
            self.mode = Mode::Study;
        }
    }

    pub(crate) fn toggle_repeat_current_ayah(&mut self) {
        self.repeat_current_ayah = !self.repeat_current_ayah;
        if self.repeat_current_ayah {
            self.loop_range = None;
        }
        self.loop_latch_end_ms = None;
    }

    pub(crate) fn set_loop_start(&mut self) {
        let Some(ayah) = self
            .study_selected_ayah()
            .or_else(|| self.mushaf.current_ayah())
        else {
            return;
        };
        let end = self.loop_range.map(|(_, end)| end).unwrap_or(ayah);
        self.loop_range = Some((ayah.min(end), ayah.max(end)));
        self.repeat_current_ayah = false;
        self.loop_latch_end_ms = None;
    }

    pub(crate) fn set_loop_end(&mut self) {
        let Some(ayah) = self
            .study_selected_ayah()
            .or_else(|| self.mushaf.current_ayah())
        else {
            return;
        };
        let start = self.loop_range.map(|(start, _)| start).unwrap_or(ayah);
        self.loop_range = Some((start.min(ayah), start.max(ayah)));
        self.repeat_current_ayah = false;
        self.loop_latch_end_ms = None;
    }

    pub(crate) fn clear_loop_range(&mut self) {
        self.loop_range = None;
        self.repeat_current_ayah = false;
        self.loop_latch_end_ms = None;
    }

    pub(crate) fn is_playing(&self) -> bool {
        self.playing_reciter.is_some() && !self.player.is_paused()
    }

    pub(crate) fn poll_player(&mut self) {
        if self.playing_reciter.is_some() {
            self.position = self.player.get_position();
            self.duration = self.player.get_duration();
            trace!(
                position = self.position,
                duration = self.duration,
                "Player poll"
            );
        }
        if self.playing_reciter.is_some() && self.player.eof_reached() {
            self.handle_track_end();
        }
        self.apply_study_loops();
        self.maybe_persist_playback();
    }

    fn handle_track_end(&mut self) {
        let Some(current_index) = self.queue_index else {
            return;
        };
        let next_index = match self.repeat_mode {
            RepeatMode::One => Some(current_index),
            RepeatMode::All if current_index + 1 >= self.queue.len() => Some(0),
            _ if current_index + 1 < self.queue.len() => Some(current_index + 1),
            _ => None,
        };

        if let Some(index) = next_index {
            self.queue_index = Some(index);
            self.play_current_queue_item(None);
        }
    }

    fn apply_study_loops(&mut self) {
        let target = if self.repeat_current_ayah {
            self.mushaf
                .current_ayah()
                .and_then(|ayah| self.mushaf.ayah_bounds(ayah))
        } else if let Some((start_ayah, end_ayah)) = self.loop_range {
            match (
                self.mushaf.ayah_bounds(start_ayah),
                self.mushaf.ayah_bounds(end_ayah),
            ) {
                (Some((start_ms, _)), Some((_, end_ms))) => Some((start_ms, end_ms)),
                _ => None,
            }
        } else {
            None
        };

        let Some((start_ms, end_ms)) = target else {
            self.loop_latch_end_ms = None;
            return;
        };
        let end_secs = end_ms as f64 / 1000.0;
        if self.position + 0.05 >= end_secs {
            if self.loop_latch_end_ms != Some(end_ms) {
                self.player.seek_absolute(start_ms as f64 / 1000.0);
                self.position = start_ms as f64 / 1000.0;
                self.loop_latch_end_ms = Some(end_ms);
            }
        } else if self.position < end_secs - 0.25 {
            self.loop_latch_end_ms = None;
        }
    }

    fn maybe_persist_playback(&mut self) {
        if let (Some(reciter_index), Some(surah_id)) = (self.playing_reciter, self.playing_surah) {
            let bucket = (self.position / 10.0).floor() as i64;
            if bucket != self.last_saved_position_bucket {
                self.last_saved_position_bucket = bucket;
                let reciter_id = self
                    .reciters
                    .get(reciter_index)
                    .map(|reciter| reciter._id)
                    .unwrap_or_default();
                self.record_recent(reciter_id, surah_id, self.position);
                self.persist_settings();
            }
        }
    }

    pub(crate) fn poll_surah_text(&mut self) {
        if let Some(rx) = self.ayah_text_rx.take() {
            match rx.try_recv() {
                Ok(result) => {
                    if self.latest_ayah_text_request_id != Some(result.request_id) {
                        return;
                    }

                    self.latest_ayah_text_request_id = None;
                    self.ayah_text_surah = Some(result.surah);
                    self.ayah_texts = result.ayahs;
                    self.ayah_display_texts = self
                        .ayah_texts
                        .iter()
                        .map(|item| shape(&item.text))
                        .collect();
                    self.selected_ayah_index = self
                        .mushaf
                        .current_ayah()
                        .and_then(|ayah| self.ayah_texts.iter().position(|item| item.ayah == ayah))
                        .unwrap_or_default();
                    self.study_scroll = adjust_scroll(
                        self.selected_ayah_index,
                        self.study_scroll,
                        self.study_viewport_height,
                    );
                    self.ayah_text_status = if self.ayah_texts.is_empty() {
                        Some("Ayah text unavailable".to_string())
                    } else {
                        None
                    };
                }
                Err(mpsc::TryRecvError::Empty) => {
                    self.ayah_text_rx = Some(rx);
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.latest_ayah_text_request_id = None;
                    self.ayah_text_status = Some("Ayah text worker stopped".to_string());
                }
            }
        }
    }

    pub(super) fn load_surah_text_async(&mut self, surah: u32) {
        if self.ayah_text_surah == Some(surah) && !self.ayah_texts.is_empty() {
            return;
        }

        self.ayah_text_request_id = self.ayah_text_request_id.wrapping_add(1);
        let request_id = self.ayah_text_request_id;
        self.latest_ayah_text_request_id = Some(request_id);
        self.ayah_texts.clear();
        self.ayah_display_texts.clear();
        self.ayah_text_status = Some("Loading ayah text…".to_string());
        self.ayah_text_surah = None;
        self.ayah_text_panel.clear();

        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            let ayahs = fetch_surah_text(surah);
            let _ = tx.send(AyahTextLoadResult {
                request_id,
                surah,
                ayahs,
            });
        });
        self.ayah_text_rx = Some(rx);
    }

    pub(crate) fn current_ayah_text(&self) -> Option<&str> {
        let ayah = self.mushaf.current_ayah()?;
        self.ayah_texts
            .iter()
            .find(|item| item.ayah == ayah)
            .map(|item| item.text.as_str())
    }

    pub(crate) fn ayah_text_status(&self) -> Option<&str> {
        self.ayah_text_status.as_deref().or_else(|| {
            if self.mushaf.current_ayah().is_some()
                && !self.ayah_texts.is_empty()
                && self.current_ayah_text().is_none()
            {
                Some("Ayah text unavailable for current timing")
            } else {
                None
            }
        })
    }

    pub(crate) fn study_ayahs(&self) -> &[AyahText] {
        &self.ayah_texts
    }

    pub(crate) fn study_ayah_display_text(&self, index: usize) -> Option<&str> {
        self.ayah_display_texts.get(index).map(String::as_str)
    }
}
