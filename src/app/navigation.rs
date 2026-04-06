use super::*;

impl App {
    pub(crate) fn move_reciter_selection(&mut self, delta: isize) {
        let visible = self.visible_reciter_indices();
        if visible.is_empty() {
            return;
        }
        let current = visible
            .iter()
            .position(|index| *index == self.selected_reciter)
            .unwrap_or_default();
        let next = current
            .saturating_add_signed(delta)
            .min(visible.len().saturating_sub(1));
        if next == current {
            return;
        }
        self.selected_reciter = visible[next];
        self.selected_surah = 0;
        self.surah_scroll = 0;
        self.reciter_scroll =
            adjust_scroll(next, self.reciter_scroll, self.reciter_viewport_height);
    }

    pub(crate) fn move_surah_selection(&mut self, delta: isize) {
        let surahs = self.selected_surah_list();
        if surahs.is_empty() {
            return;
        }
        let next = self
            .selected_surah
            .saturating_add_signed(delta)
            .min(surahs.len().saturating_sub(1));
        if next == self.selected_surah {
            return;
        }
        self.selected_surah = next;
        self.surah_scroll = adjust_scroll(next, self.surah_scroll, self.surah_viewport_height);
    }

    pub(crate) fn jump_to_start(&mut self) {
        match self.mode {
            Mode::Browse if self.focus == Focus::Reciters => {
                let visible = self.visible_reciter_indices();
                if let Some(first) = visible.first() {
                    self.selected_reciter = *first;
                    self.selected_surah = 0;
                    self.reciter_scroll = 0;
                    self.surah_scroll = 0;
                }
            }
            Mode::Browse => {
                self.selected_surah = 0;
                self.surah_scroll = 0;
            }
            Mode::Study => {
                self.selected_ayah_index = 0;
                self.study_scroll = 0;
            }
            Mode::Settings => {
                self.settings_field = SettingsField::PreferOffline;
            }
            Mode::Listen => {}
        }
    }

    pub(crate) fn jump_to_end(&mut self) {
        match self.mode {
            Mode::Browse if self.focus == Focus::Reciters => {
                let visible = self.visible_reciter_indices();
                if let Some(last) = visible.last() {
                    self.selected_reciter = *last;
                    self.selected_surah = 0;
                    self.reciter_scroll = visible.len().saturating_sub(1);
                    self.surah_scroll = 0;
                }
            }
            Mode::Browse => {
                let surahs = self.selected_surah_list();
                if !surahs.is_empty() {
                    self.selected_surah = surahs.len().saturating_sub(1);
                    self.surah_scroll = adjust_scroll(
                        self.selected_surah,
                        self.surah_scroll,
                        self.surah_viewport_height,
                    );
                }
            }
            Mode::Study => {
                self.selected_ayah_index = self.ayah_texts.len().saturating_sub(1);
                self.study_scroll = adjust_scroll(
                    self.selected_ayah_index,
                    self.study_scroll,
                    self.study_viewport_height,
                );
            }
            Mode::Settings => {
                self.settings_field = SettingsField::DownloadConcurrency;
            }
            Mode::Listen => {}
        }
    }

    pub(crate) fn toggle_focus(&mut self) {
        if self.mode == Mode::Browse {
            self.focus = match self.focus {
                Focus::Reciters => Focus::Surahs,
                Focus::Surahs => Focus::Reciters,
            };
            self.persist_settings();
        }
    }

    pub(crate) fn cycle_speed(&mut self) {
        self.speed_idx = (self.speed_idx + 1) % self.speeds.len();
        self.speed = self.speeds[self.speed_idx];
        info!(speed = self.speed, "Speed changed");
        self.player.set_speed(self.speed);
        self.persist_settings();
    }

    pub(crate) fn cycle_filter(&mut self) {
        self.browse_filter = self.browse_filter.next();
        self.ensure_valid_selection();
        self.persist_settings();
    }

    pub(crate) fn enter_search(&mut self) {
        self.search_mode = true;
    }

    pub(crate) fn exit_search(&mut self) {
        self.search_mode = false;
        self.persist_settings();
    }

    pub(crate) fn push_search_char(&mut self, ch: char) {
        self.search_query.push(ch);
        self.ensure_valid_selection();
    }

    pub(crate) fn pop_search_char(&mut self) {
        self.search_query.pop();
        self.ensure_valid_selection();
    }

    pub(crate) fn clear_search(&mut self) {
        self.search_query.clear();
        self.ensure_valid_selection();
        self.persist_settings();
    }

    pub(crate) fn toggle_favorite(&mut self) {
        match self.mode {
            Mode::Browse if self.focus == Focus::Reciters => {
                let Some(reciter_id) = self.selected_reciter_id() else {
                    return;
                };
                if !self.favorites.reciters.insert(reciter_id) {
                    self.favorites.reciters.remove(&reciter_id);
                }
            }
            _ => {
                let Some(reciter_id) = self
                    .playing_reciter
                    .and_then(|index| self.reciters.get(index).map(|reciter| reciter._id))
                    .or_else(|| self.selected_reciter_id())
                else {
                    return;
                };
                let Some(surah_id) = self.playing_surah.or_else(|| self.selected_surah_number())
                else {
                    return;
                };
                if !self.favorites.surahs.insert((reciter_id, surah_id)) {
                    self.favorites.surahs.remove(&(reciter_id, surah_id));
                }
            }
        }
        self.persistence.save_favorites(&self.favorites);
        self.ensure_valid_selection();
    }
}
