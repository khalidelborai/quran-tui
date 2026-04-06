use serde::{Deserialize, de::DeserializeOwned};
use tracing::{error, info, warn};

use crate::config::{API_BASE, async_http_client, blocking_http_client};

const QURAN_TEXT_API_BASE: &str = "https://api.alquran.cloud/v1";
const QURAN_TEXT_EDITION: &str = "quran-simple-clean";

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct AyahTiming {
    pub(crate) ayah: u32,
    pub(crate) start_time: u32,
    pub(crate) end_time: u32,
}

#[derive(Debug, Deserialize, Clone)]
pub(crate) struct TimingRead {
    pub(crate) id: u32,
    #[serde(rename = "name")]
    pub(crate) _name: String,
    #[serde(default)]
    pub(crate) folder_url: String,
}

#[derive(Debug, Deserialize, Clone)]
pub(crate) struct Reciter {
    #[serde(rename = "id")]
    pub(crate) _id: u32,
    pub(crate) name: String,
    pub(crate) moshaf: Vec<Moshaf>,
}

#[derive(Debug, Deserialize, Clone)]
pub(crate) struct Moshaf {
    #[serde(rename = "id")]
    pub(crate) _id: u32,
    #[serde(rename = "name")]
    pub(crate) _name: String,
    pub(crate) server: String,
    pub(crate) surah_total: u32,
    pub(crate) surah_list: String,
}

#[derive(Debug, Deserialize, Clone)]
pub(crate) struct Surah {
    pub(crate) id: u32,
    pub(crate) name: String,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct AyahText {
    #[serde(rename = "numberInSurah")]
    pub(crate) ayah: u32,
    pub(crate) text: String,
}

#[derive(Deserialize)]
struct RecitersResponse {
    reciters: Vec<Reciter>,
}

#[derive(Deserialize)]
struct SuwarResponse {
    suwar: Vec<Surah>,
}

#[derive(Deserialize)]
struct SurahTextResponse {
    data: SurahTextData,
}

#[derive(Deserialize)]
struct SurahTextData {
    ayahs: Vec<AyahText>,
}

pub(crate) fn parse_surah_list(value: &str) -> Vec<u32> {
    value
        .split(',')
        .filter_map(|surah| surah.trim().parse().ok())
        .collect()
}

fn fetch_json_blocking<T: DeserializeOwned>(url: &str, label: &str) -> Option<T> {
    let response = match blocking_http_client().get(url).send() {
        Ok(response) => response,
        Err(error) => {
            warn!(%label, %url, %error, "Blocking request failed");
            return None;
        }
    };

    if !response.status().is_success() {
        warn!(%label, %url, status = %response.status(), "Blocking request returned non-success status");
        return None;
    }

    match response.json() {
        Ok(payload) => Some(payload),
        Err(error) => {
            warn!(%label, %url, %error, "Failed to parse blocking JSON response");
            None
        }
    }
}

async fn fetch_json_async<T: DeserializeOwned>(url: &str, label: &str) -> Result<T, String> {
    let response = async_http_client().get(url).send().await.map_err(|error| {
        error!(%label, %url, %error, "Async request failed");
        error.to_string()
    })?;

    let response = response.error_for_status().map_err(|error| {
        error!(%label, %url, %error, "Async request returned non-success status");
        error.to_string()
    })?;

    response.json().await.map_err(|error| {
        error!(%label, %url, %error, "Failed to parse async JSON response");
        error.to_string()
    })
}

pub(crate) fn fetch_ayah_timing(surah: u32, read_id: u32) -> Vec<AyahTiming> {
    info!(surah, read_id, "Fetching ayah timing");
    let url = format!("{}/ayat_timing?surah={}&read={}", API_BASE, surah, read_id);
    let timings = fetch_json_blocking::<Vec<AyahTiming>>(&url, "ayah timing").unwrap_or_default();
    info!(surah, read_id, count = timings.len(), "Ayah timing loaded");
    timings
}

pub(crate) fn fetch_timing_reads() -> Vec<TimingRead> {
    info!("Fetching timing reads");
    let url = format!("{}/ayat_timing/reads", API_BASE);
    let reads = fetch_json_blocking::<Vec<TimingRead>>(&url, "timing reads").unwrap_or_default();
    info!(count = reads.len(), "Timing reads loaded");
    reads
}

pub(crate) fn fetch_surah_text(surah: u32) -> Vec<AyahText> {
    info!(surah, "Fetching surah text");
    let url = format!(
        "{}/surah/{}/{}",
        QURAN_TEXT_API_BASE, surah, QURAN_TEXT_EDITION
    );
    let data = fetch_json_blocking::<SurahTextResponse>(&url, "surah text")
        .map(|response| response.data.ayahs)
        .unwrap_or_default();
    info!(surah, count = data.len(), "Surah text loaded");
    data
}

pub(crate) async fn fetch_reciters() -> Result<Vec<Reciter>, String> {
    info!("Fetching reciters from API...");
    let url = format!("{}/reciters?language=ar", API_BASE);
    let data: RecitersResponse = fetch_json_async(&url, "reciters").await?;
    info!(count = data.reciters.len(), "Reciters loaded");
    Ok(data.reciters)
}

pub(crate) async fn fetch_surahs() -> Result<Vec<Surah>, String> {
    info!("Fetching surahs from API...");
    let url = format!("{}/suwar?language=ar", API_BASE);
    let data: SuwarResponse = fetch_json_async(&url, "surahs").await?;
    info!(count = data.suwar.len(), "Surahs loaded");
    Ok(data.suwar)
}
