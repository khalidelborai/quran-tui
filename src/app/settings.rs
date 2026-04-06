use super::*;

impl App {
    pub(crate) fn open_settings(&mut self) {
        self.set_mode(Mode::Settings);
        self.settings_edit_mode = false;
        self.settings_buffer.clear();
    }

    pub(crate) fn close_settings(&mut self) {
        self.set_mode(Mode::Browse);
        self.settings_edit_mode = false;
        self.settings_buffer.clear();
    }

    pub(crate) fn move_settings_selection(&mut self, delta: isize) {
        self.settings_field = if delta > 0 {
            self.settings_field.next()
        } else {
            self.settings_field.previous()
        };
        self.persist_settings();
    }

    pub(crate) fn settings_value(&self, field: SettingsField) -> String {
        match field {
            SettingsField::PreferOffline => {
                on_off_label(self.settings.prefer_offline_playback).to_string()
            }
            SettingsField::CacheStreams => {
                on_off_label(self.settings.cache_streams_while_playing).to_string()
            }
            SettingsField::DownloadDirectory => self.settings.download_directory.clone(),
            SettingsField::DownloadConcurrency => self.settings.download_concurrency.to_string(),
        }
    }

    pub(crate) fn activate_settings_field(&mut self) {
        match self.settings_field {
            SettingsField::PreferOffline => {
                self.set_prefer_offline_playback(!self.settings.prefer_offline_playback, true)
            }
            SettingsField::CacheStreams => self
                .set_cache_streams_while_playing(!self.settings.cache_streams_while_playing, true),
            SettingsField::DownloadDirectory => {
                self.settings_edit_mode = true;
                self.settings_buffer = self.settings.download_directory.clone();
            }
            SettingsField::DownloadConcurrency => {
                self.adjust_download_concurrency(1);
            }
        }
    }

    pub(crate) fn adjust_settings_value(&mut self, delta: isize) {
        match self.settings_field {
            SettingsField::PreferOffline => self.set_prefer_offline_playback(delta >= 0, false),
            SettingsField::CacheStreams => {
                self.set_cache_streams_while_playing(delta >= 0, false);
            }
            SettingsField::DownloadDirectory => {}
            SettingsField::DownloadConcurrency => self.adjust_download_concurrency(delta),
        }
    }

    fn set_prefer_offline_playback(&mut self, enabled: bool, announce: bool) {
        self.settings.prefer_offline_playback = enabled;
        if announce {
            self.notice = Some(format!(
                "Prefer offline playback {}",
                on_off_label(self.settings.prefer_offline_playback)
            ));
        }
        self.persist_settings();
    }

    fn set_cache_streams_while_playing(&mut self, enabled: bool, announce: bool) {
        self.settings.cache_streams_while_playing = enabled;
        if !enabled {
            self.finish_active_stream_recording();
        }
        if announce {
            self.notice = Some(format!(
                "Save streams while playing {}",
                on_off_label(self.settings.cache_streams_while_playing)
            ));
        }
        self.persist_settings();
    }

    fn adjust_download_concurrency(&mut self, delta: isize) {
        let next = self
            .settings
            .download_concurrency
            .saturating_add_signed(delta)
            .clamp(1, 8);
        self.settings.download_concurrency = next;
        self.downloads.set_concurrency(next);
        self.notice = Some(format!("Concurrent downloads set to {next}"));
        self.persist_settings();
    }

    pub(crate) fn push_settings_char(&mut self, ch: char) {
        if self.settings_edit_mode {
            self.settings_buffer.push(ch);
        }
    }

    pub(crate) fn pop_settings_char(&mut self) {
        if self.settings_edit_mode {
            self.settings_buffer.pop();
        }
    }

    pub(crate) fn commit_settings_edit(&mut self) {
        if !self.settings_edit_mode {
            return;
        }
        if self.settings_field == SettingsField::DownloadDirectory {
            let value = self.settings_buffer.trim().to_string();
            if !value.is_empty() {
                self.finish_active_stream_recording();
                let old_root = self.download_root_path();
                let new_root = PathBuf::from(&value);
                let _ = std::fs::create_dir_all(&new_root);
                let migrated = if old_root != new_root {
                    self.migrate_download_directory(&new_root)
                } else {
                    0
                };
                self.settings.download_directory = new_root.display().to_string();
                self.refresh_local_media_index();
                self.notice = Some(if migrated > 0 {
                    format!("Updated download directory and moved {migrated} file(s)")
                } else {
                    "Updated download directory".to_string()
                });
            }
        }
        self.settings_edit_mode = false;
        self.settings_buffer.clear();
        self.persist_settings();
    }

    pub(crate) fn cancel_settings_edit(&mut self) {
        self.settings_edit_mode = false;
        self.settings_buffer.clear();
    }
}
