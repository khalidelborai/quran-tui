//! Quran MP3 TUI — A terminal Quran player with Arabic text support.
//!
//! Uses arabic_reshaper + unicode-bidi for proper Arabic rendering on any terminal.
//! Uses mpv via subprocess + JSON IPC for audio playback.

mod api;
mod app;
mod ayah_panel;
mod config;
mod downloads;
mod mushaf;
mod persistence;
mod player;
mod shaping;
mod terminal;
mod ui;

use std::fs::OpenOptions;
use std::io;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::Terminal;
use tracing::{debug, error, info};
use tracing_subscriber::{EnvFilter, fmt, layer::SubscriberExt, util::SubscriberInitExt};

use crate::api::{Reciter, Surah, TimingRead, fetch_reciters, fetch_surahs, fetch_timing_reads};
use crate::app::{App, Focus, Mode};
use crate::config::runtime_path;
use crate::terminal::TerminalGuard;
use crate::ui::ui;

const UI_TICK_MS: u64 = 50;

type LibraryLoadResult = (Result<Vec<Reciter>, String>, Result<Vec<Surah>, String>);

#[tokio::main]
async fn main() -> io::Result<()> {
    let log_path = runtime_path("log");
    let log_file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&log_path)
        .expect("create log file");
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("debug")))
        .with(
            fmt::layer()
                .with_writer(std::sync::Mutex::new(log_file))
                .with_ansi(false),
        )
        .init();

    info!(db = %crate::persistence::AppPersistence::new().path().display(), "Quran TUI starting");

    let mut app = App::new();
    let (library_rx, timing_rx) = spawn_background_loads();

    let _terminal_guard = TerminalGuard::enter()?;
    let backend = ratatui::backend::CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::new(backend)?;

    loop {
        handle_library_results(&mut app, &library_rx);
        handle_timing_reads(&mut app, &timing_rx);
        poll_app(&mut app);

        terminal.draw(|frame| ui(frame, &mut app))?;

        if let Some(key_event) = next_key_event()?
            && handle_key_event(&mut app, key_event)
        {
            break;
        }
    }

    app.shutdown();
    info!(downloads = app.queue_len(), "Quran TUI shutting down");
    Ok(())
}

fn spawn_background_loads() -> (
    mpsc::Receiver<LibraryLoadResult>,
    mpsc::Receiver<Vec<TimingRead>>,
) {
    let (library_tx, library_rx) = mpsc::channel();
    tokio::spawn(async move {
        let reciters = fetch_reciters().await;
        let surahs = fetch_surahs().await;
        let _ = library_tx.send((reciters, surahs));
    });

    let (timing_tx, timing_rx) = mpsc::channel();
    thread::spawn(move || {
        let reads = fetch_timing_reads();
        let _ = timing_tx.send(reads);
    });

    (library_rx, timing_rx)
}

fn handle_library_results(app: &mut App, library_rx: &mpsc::Receiver<LibraryLoadResult>) {
    if let Ok((reciters_result, surahs_result)) = library_rx.try_recv() {
        match (reciters_result, surahs_result) {
            (Ok(reciters), Ok(surahs)) => {
                info!(
                    reciters = reciters.len(),
                    surahs = surahs.len(),
                    "Data loaded successfully"
                );
                app.set_library_data(reciters, surahs);
                app.loading = false;
            }
            (Err(error), _) | (_, Err(error)) => {
                error!(error = %error, "Failed to load data");
                app.error = Some(error);
                app.loading = false;
            }
        }
    }
}

fn handle_timing_reads(app: &mut App, timing_rx: &mpsc::Receiver<Vec<TimingRead>>) {
    if let Ok(reads) = timing_rx.try_recv() {
        info!(count = reads.len(), "Timing reads received");
        app.mushaf.set_timing_reads(reads);
    }
}

fn poll_app(app: &mut App) {
    app.mushaf.poll_background_results();
    app.poll_surah_text();
    app.poll_player();
    app.sync_downloads();

    if app.playing_reciter.is_some() {
        app.mushaf.update_position(app.position);
    }
}

fn next_key_event() -> io::Result<Option<KeyEvent>> {
    if !event::poll(Duration::from_millis(UI_TICK_MS))? {
        return Ok(None);
    }

    let Event::Key(key) = event::read()? else {
        return Ok(None);
    };
    if key.kind != KeyEventKind::Press {
        return Ok(None);
    }

    Ok(Some(key))
}

fn handle_key_event(app: &mut App, key_event: KeyEvent) -> bool {
    if app.search_mode {
        return handle_search_key(app, key_event);
    }
    if app.mode == Mode::Settings && app.settings_edit_mode {
        return handle_settings_edit_key(app, key_event);
    }
    if app.mode == Mode::Settings {
        return handle_settings_key(app, key_event);
    }

    match key_event.code {
        KeyCode::Char('q') => {
            info!("Quit requested");
            true
        }
        KeyCode::Char('/') => {
            app.enter_search();
            false
        }
        KeyCode::Char('?') => false,
        KeyCode::Char('b') => {
            app.set_mode(Mode::Browse);
            false
        }
        KeyCode::Char('l') => {
            app.set_mode(Mode::Listen);
            false
        }
        KeyCode::Char('s') | KeyCode::Char('S') => {
            app.set_mode(Mode::Study);
            false
        }
        KeyCode::Char('o') | KeyCode::Char('O') => {
            app.open_settings();
            false
        }
        KeyCode::Char('f') | KeyCode::Char('*') => {
            app.toggle_favorite();
            false
        }
        KeyCode::Char('r') => {
            app.cycle_repeat_mode();
            false
        }
        KeyCode::Char('F') => {
            app.cycle_filter();
            false
        }
        KeyCode::Char('d') => {
            app.queue_selected_download();
            false
        }
        KeyCode::Char('D') => {
            app.queue_selected_reciter_downloads();
            false
        }
        KeyCode::Char('c') => {
            app.cancel_selected_download();
            false
        }
        KeyCode::Char('R') => {
            app.retry_selected_download();
            false
        }
        KeyCode::Char(' ') => {
            app.player.toggle_pause();
            false
        }
        KeyCode::Left => {
            if app.mode == Mode::Settings {
                app.adjust_settings_value(-1);
            } else {
                debug!("Seeking -5s");
                app.player.seek(-5.0);
            }
            false
        }
        KeyCode::Right => {
            if app.mode == Mode::Settings {
                app.adjust_settings_value(1);
            } else {
                debug!("Seeking +5s");
                app.player.seek(5.0);
            }
            false
        }
        KeyCode::Char('[') => {
            if app.mode == Mode::Study {
                app.set_loop_start();
            } else {
                app.player.seek(-15.0);
            }
            false
        }
        KeyCode::Char(']') => {
            if app.mode == Mode::Study {
                app.set_loop_end();
            } else {
                app.player.seek(15.0);
            }
            false
        }
        KeyCode::Esc => {
            if app.mode == Mode::Settings {
                if app.settings_edit_mode {
                    app.cancel_settings_edit();
                } else {
                    app.close_settings();
                }
            } else {
                app.clear_loop_range();
            }
            false
        }
        KeyCode::Char('y') => {
            if app.mode == Mode::Study {
                app.toggle_repeat_current_ayah();
            }
            false
        }
        KeyCode::Char('v') | KeyCode::Char('V') => {
            app.cycle_speed();
            false
        }
        KeyCode::Char('n') => {
            app.play_next();
            false
        }
        KeyCode::Char('p') => {
            app.play_previous();
            false
        }
        KeyCode::Tab => {
            app.toggle_focus();
            false
        }
        KeyCode::Up | KeyCode::Char('k') => {
            handle_vertical_motion(app, -1);
            false
        }
        KeyCode::Down | KeyCode::Char('j') => {
            handle_vertical_motion(app, 1);
            false
        }
        KeyCode::Char('g') => {
            if app.should_handle_second_g() {
                app.jump_to_start();
                app.mark_pending_g(false);
            } else {
                app.mark_pending_g(true);
            }
            false
        }
        KeyCode::Char('G') => {
            app.jump_to_end();
            app.mark_pending_g(false);
            false
        }
        KeyCode::Enter => {
            app.mark_pending_g(false);
            if app.mode == Mode::Browse {
                info!("Enter pressed in Browse mode, starting playback");
                app.play_selected();
            } else if app.mode == Mode::Study {
                app.jump_to_selected_ayah();
            } else if app.mode == Mode::Settings {
                if app.settings_edit_mode {
                    app.commit_settings_edit();
                } else {
                    app.activate_settings_field();
                }
            }
            false
        }
        KeyCode::Home => {
            app.jump_to_start();
            false
        }
        KeyCode::End => {
            app.jump_to_end();
            false
        }
        _ => {
            app.mark_pending_g(false);
            false
        }
    }
}

fn handle_vertical_motion(app: &mut App, delta: isize) {
    app.mark_pending_g(false);
    match app.mode {
        Mode::Browse => match app.focus {
            Focus::Reciters => app.move_reciter_selection(delta),
            Focus::Surahs => app.move_surah_selection(delta),
        },
        Mode::Study => app.move_study_selection(delta),
        Mode::Settings => app.move_settings_selection(delta),
        Mode::Listen => {}
    }
}

fn handle_search_key(app: &mut App, key_event: KeyEvent) -> bool {
    match key_event.code {
        KeyCode::Esc => {
            if app.search_query.is_empty() {
                app.exit_search();
            } else {
                app.clear_search();
            }
            false
        }
        KeyCode::Enter => {
            app.exit_search();
            false
        }
        KeyCode::Backspace => {
            app.pop_search_char();
            false
        }
        KeyCode::Char('u') if key_event.modifiers.contains(KeyModifiers::CONTROL) => {
            app.clear_search();
            false
        }
        KeyCode::Char(ch) if !ch.is_control() => {
            app.push_search_char(ch);
            false
        }
        _ => false,
    }
}

fn handle_settings_key(app: &mut App, key_event: KeyEvent) -> bool {
    match key_event.code {
        KeyCode::Char('q') => true,
        KeyCode::Esc | KeyCode::Char('o') | KeyCode::Char('O') => {
            app.close_settings();
            false
        }
        KeyCode::Up | KeyCode::Char('k') => {
            app.move_settings_selection(-1);
            false
        }
        KeyCode::Down | KeyCode::Char('j') => {
            app.move_settings_selection(1);
            false
        }
        KeyCode::Left => {
            app.adjust_settings_value(-1);
            false
        }
        KeyCode::Right => {
            app.adjust_settings_value(1);
            false
        }
        KeyCode::Enter | KeyCode::Char(' ') => {
            app.activate_settings_field();
            false
        }
        KeyCode::Home | KeyCode::Char('g') => {
            app.jump_to_start();
            false
        }
        KeyCode::End | KeyCode::Char('G') => {
            app.jump_to_end();
            false
        }
        _ => false,
    }
}

fn handle_settings_edit_key(app: &mut App, key_event: KeyEvent) -> bool {
    match key_event.code {
        KeyCode::Esc => {
            app.cancel_settings_edit();
            false
        }
        KeyCode::Enter => {
            app.commit_settings_edit();
            false
        }
        KeyCode::Backspace => {
            app.pop_settings_char();
            false
        }
        KeyCode::Char(ch) if !ch.is_control() => {
            app.push_settings_char(ch);
            false
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use crate::api::{AyahText, AyahTiming, TimingRead};
    use crate::app::adjust_scroll;
    use crate::config::is_allowed_remote_url;
    use crate::mushaf::{MushafWidget, find_current_ayah};

    #[test]
    fn adjust_scroll_keeps_selection_visible() {
        assert_eq!(adjust_scroll(0, 0, 5), 0);
        assert_eq!(adjust_scroll(4, 0, 5), 0);
        assert_eq!(adjust_scroll(5, 0, 5), 1);
        assert_eq!(adjust_scroll(9, 4, 5), 5);
        assert_eq!(adjust_scroll(2, 4, 5), 2);
    }

    #[test]
    fn find_current_ayah_uses_half_open_ranges() {
        let timings = vec![
            AyahTiming {
                ayah: 1,
                start_time: 0,
                end_time: 1000,
            },
            AyahTiming {
                ayah: 2,
                start_time: 1000,
                end_time: 2000,
            },
        ];

        assert_eq!(
            find_current_ayah(&timings, 0).map(|ayah| ayah.ayah),
            Some(1)
        );
        assert_eq!(
            find_current_ayah(&timings, 999).map(|ayah| ayah.ayah),
            Some(1)
        );
        assert_eq!(
            find_current_ayah(&timings, 1000).map(|ayah| ayah.ayah),
            Some(2)
        );
        assert_eq!(
            find_current_ayah(&timings, 2000).map(|ayah| ayah.ayah),
            None
        );
    }

    #[test]
    fn allowed_remote_url_is_restricted_to_https_mp3quran_hosts() {
        assert!(is_allowed_remote_url(
            "https://server6.mp3quran.net/akdr/001.mp3"
        ));
        assert!(is_allowed_remote_url(
            "https://www.mp3quran.net/download/abdulbasit/001.mp3"
        ));
        assert!(!is_allowed_remote_url(
            "http://server6.mp3quran.net/akdr/001.mp3"
        ));
        assert!(!is_allowed_remote_url("https://example.com/file.mp3"));
        assert!(!is_allowed_remote_url("file:///tmp/test.mp3"));
    }

    #[test]
    fn find_read_id_matches_folder_url_without_fallback() {
        let mut mushaf = MushafWidget::new();
        mushaf.set_timing_reads(vec![
            TimingRead {
                id: 10,
                _name: "A".to_string(),
                folder_url: "server6.mp3quran.net/akdr".to_string(),
            },
            TimingRead {
                id: 20,
                _name: "B".to_string(),
                folder_url: "server9.mp3quran.net/other".to_string(),
            },
        ]);

        assert_eq!(
            mushaf.find_read_id("https://server6.mp3quran.net/akdr/"),
            Some(10)
        );
        assert_eq!(
            mushaf.find_read_id("https://unmatched.mp3quran.net/reader/"),
            None
        );
    }

    #[test]
    fn timing_reads_deserialize_from_top_level_array() {
        let json =
            r#"[{"id":1,"name":"Reader","folder_url":"https://server6.mp3quran.net/akdr/"}]"#;
        let reads: Vec<TimingRead> = serde_json::from_str(json).expect("timing reads JSON");
        assert_eq!(reads.len(), 1);
        assert_eq!(reads[0].id, 1);
        assert_eq!(reads[0].folder_url, "https://server6.mp3quran.net/akdr/");
    }

    #[test]
    fn ayah_timings_deserialize_without_needing_page_metadata() {
        let json =
            r#"[{"ayah":1,"start_time":0,"end_time":10,"polygon":"ignored","page":"ignored"}]"#;
        let timings: Vec<AyahTiming> = serde_json::from_str(json).expect("ayah timing JSON");
        assert_eq!(timings.len(), 1);
        assert_eq!(timings[0].ayah, 1);
        assert_eq!(timings[0].start_time, 0);
        assert_eq!(timings[0].end_time, 10);
    }

    #[test]
    fn surah_text_deserializes_uthmani_payload() {
        let json = r#"{"data":{"ayahs":[{"numberInSurah":3,"text":"ٱلرَّحۡمَٰنِ ٱلرَّحِيمِ"}]}}"#;
        #[derive(serde::Deserialize)]
        struct Wrapper {
            data: Inner,
        }
        #[derive(serde::Deserialize)]
        struct Inner {
            ayahs: Vec<AyahText>,
        }

        let payload: Wrapper = serde_json::from_str(json).expect("surah text JSON");
        assert_eq!(payload.data.ayahs.len(), 1);
        assert_eq!(payload.data.ayahs[0].ayah, 3);
        assert_eq!(payload.data.ayahs[0].text, "ٱلرَّحۡمَٰنِ ٱلرَّحِيمِ");
    }
}
