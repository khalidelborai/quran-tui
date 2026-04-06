use super::*;

impl App {
    pub(crate) fn reciter_downloaded_count(&self, reciter_index: usize) -> usize {
        let Some(reciter) = self.reciters.get(reciter_index) else {
            return 0;
        };
        self.reciter_surah_lists
            .get(reciter_index)
            .map(|surahs| {
                surahs
                    .iter()
                    .filter(|surah_id| {
                        self.offline_path_for(reciter._id, &reciter.name, **surah_id)
                            .is_some()
                    })
                    .count()
            })
            .unwrap_or_default()
    }

    pub(crate) fn has_downloaded_surah(
        &self,
        reciter_id: u32,
        reciter_name: &str,
        surah_id: u32,
    ) -> bool {
        self.offline_path_for(reciter_id, reciter_name, surah_id)
            .is_some()
    }

    pub(crate) fn up_next(&self, limit: usize) -> Vec<String> {
        let Some(current_index) = self.queue_index else {
            return Vec::new();
        };
        self.queue
            .iter()
            .skip(current_index.saturating_add(1))
            .take(limit)
            .map(|item| {
                let surah_name = self
                    .surah_display_name(item.surah_number)
                    .unwrap_or("Unknown surah");
                format!("{:03} {}", item.surah_number, surah_name)
            })
            .collect()
    }

    pub(crate) fn download_preview(&self, limit: usize) -> Vec<String> {
        self.downloads.queue_preview(limit)
    }

    pub(crate) fn download_status_label(&self, reciter_id: u32, surah_id: u32) -> Option<String> {
        self.downloads.status_label(reciter_id, surah_id)
    }
}
