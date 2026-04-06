use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Gauge, List, ListItem, Paragraph, Wrap},
};

use crate::app::{App, Focus, Mode, adjust_scroll};

const C_GREEN: Color = Color::Rgb(122, 170, 80);
const C_AMBER: Color = Color::Rgb(201, 162, 39);
const C_CREAM: Color = Color::Rgb(240, 232, 216);
const C_DIM: Color = Color::Rgb(106, 90, 69);
const C_MED: Color = Color::Rgb(154, 138, 112);
const C_BORDER: Color = Color::Rgb(58, 53, 48);
const C_BG_SEL: Color = Color::Rgb(45, 74, 24);
const C_CARD_BG: Color = Color::Rgb(35, 47, 30);
const C_CARD_BORDER: Color = Color::Rgb(108, 140, 74);
const C_CARD_TEXT: Color = Color::Rgb(236, 230, 214);

fn list_item_style(is_selected: bool, is_active: bool) -> Style {
    if is_selected {
        Style::default().fg(Color::White).bg(C_BG_SEL)
    } else if is_active {
        Style::default().fg(C_GREEN)
    } else {
        Style::default().fg(C_CREAM)
    }
}

fn list_item_icon(is_active: bool, is_favorite: bool) -> &'static str {
    if is_active {
        "▶"
    } else if is_favorite {
        "★"
    } else {
        " "
    }
}

fn focus_border_style(is_focused: bool) -> Style {
    if is_focused {
        Style::default().fg(C_GREEN)
    } else {
        Style::default().fg(C_BORDER)
    }
}

pub(crate) fn inner_panel_area(area: Rect) -> Rect {
    if area.width <= 2 || area.height <= 2 {
        area
    } else {
        Rect {
            x: area.x + 1,
            y: area.y + 1,
            width: area.width - 2,
            height: area.height - 2,
        }
    }
}

fn fmt_time(secs: f64) -> String {
    let total = secs as u32;
    let s = total % 60;
    let m = (total / 60) % 60;
    let h = total / 3600;
    if h > 0 {
        format!("{}:{:02}:{:02}", h, m, s)
    } else {
        format!("{}:{:02}", m, s)
    }
}

pub(crate) fn ui(frame: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(8),
            Constraint::Length(3),
        ])
        .split(frame.area());

    render_header(frame, app, chunks[0]);

    if app.loading {
        let loading = Paragraph::new("Loading reciters...")
            .alignment(Alignment::Center)
            .style(Style::default().fg(C_MED));
        frame.render_widget(loading, chunks[1]);
    } else if let Some(ref err) = app.error {
        let msg = Paragraph::new(format!("Error: {}", err))
            .alignment(Alignment::Center)
            .style(Style::default().fg(Color::Red));
        frame.render_widget(msg, chunks[1]);
    } else {
        match app.mode {
            Mode::Browse => render_browse(frame, app, chunks[1]),
            Mode::Listen => render_listen(frame, app, chunks[1]),
            Mode::Study => render_study(frame, app, chunks[1]),
            Mode::Settings => render_settings(frame, app, chunks[1]),
        }
    }

    render_player(frame, app, chunks[2]);
}

fn render_header(frame: &mut Frame, app: &App, area: Rect) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(area);

    let mode_style = |mode: Mode| -> Style {
        if app.mode == mode {
            Style::default().fg(C_GREEN).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(C_DIM)
        }
    };

    let title = Paragraph::new(Line::from(vec![
        Span::styled(
            " Quran MP3 ",
            Style::default().fg(C_GREEN).add_modifier(Modifier::BOLD),
        ),
        Span::styled("│ ", Style::default().fg(C_BORDER)),
        Span::styled("[B]rowse", mode_style(Mode::Browse)),
        Span::raw("  "),
        Span::styled("[L]isten", mode_style(Mode::Listen)),
        Span::raw("  "),
        Span::styled("[S]tudy", mode_style(Mode::Study)),
        Span::raw("  "),
        Span::styled("[O]ptions", mode_style(Mode::Settings)),
        Span::styled(
            format!("  │  {} reciters", app.reciters.len()),
            Style::default().fg(C_DIM),
        ),
    ]));
    frame.render_widget(title, rows[0]);

    let search_label = if app.search_mode { "SEARCH" } else { "Search" };
    let filter_label = format!("Filter: {}", app.browse_filter.label());
    let search_value = if app.search_query.is_empty() {
        "(press /)".to_string()
    } else {
        app.search_query.clone()
    };
    let meta = Paragraph::new(Line::from(vec![
        Span::styled(format!(" {} ", search_label), Style::default().fg(C_AMBER)),
        Span::styled(search_value, Style::default().fg(C_CREAM)),
        Span::styled("  │  ", Style::default().fg(C_BORDER)),
        Span::styled(filter_label, Style::default().fg(C_MED)),
        Span::styled(
            "  │  [F] cycle filter  [/ ] search  [f] favorite",
            Style::default().fg(C_DIM),
        ),
    ]));
    frame.render_widget(meta, rows[1]);

    let notice = app
        .track_notice()
        .or(app.player_error.as_deref())
        .unwrap_or("[d] download  [D] download all  [n/p] next/prev  [r] repeat  [o] settings");
    let notice_style = if app.player_error.is_some() {
        Style::default().fg(Color::Red)
    } else {
        Style::default().fg(C_MED)
    };
    frame.render_widget(Paragraph::new(notice).style(notice_style), rows[2]);
}

fn render_browse(frame: &mut Frame, app: &mut App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(10), Constraint::Length(4)])
        .split(area);
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(chunks[0]);

    let visible_reciters = app.visible_reciter_indices();
    let current_visible_reciter = visible_reciters
        .iter()
        .position(|index| *index == app.selected_reciter)
        .unwrap_or_default();
    let visible_height = cols[0].height.saturating_sub(2) as usize;
    app.reciter_viewport_height = visible_height.max(1);
    let items: Vec<ListItem> = visible_reciters
        .iter()
        .enumerate()
        .skip(app.reciter_scroll)
        .take(visible_height)
        .map(|(visible_index, index)| {
            let reciter = &app.reciters[*index];
            let is_selected = *index == app.selected_reciter && app.focus == Focus::Reciters;
            let is_playing = app.playing_reciter == Some(*index);
            let is_favorite = app.reciter_is_favorite(reciter._id);
            let display_name = app
                .reciter_display_name(*index)
                .unwrap_or(reciter.name.as_str());
            let download_count = app.reciter_downloaded_count(*index);
            let style = list_item_style(is_selected, is_playing);
            let prefix = list_item_icon(is_playing, is_favorite);
            ListItem::new(Line::from(vec![
                Span::styled(format!("{} ", prefix), Style::default().fg(C_GREEN)),
                Span::styled(display_name.to_string(), style),
                Span::styled(
                    format!(
                        "  {} surahs",
                        reciter
                            .moshaf
                            .first()
                            .map(|m| m.surah_total)
                            .unwrap_or_default()
                    ),
                    Style::default().fg(C_DIM),
                ),
                Span::styled(
                    format!("  {} offline", download_count),
                    Style::default().fg(C_AMBER),
                ),
                Span::styled(
                    format!("  #{}", visible_index + 1),
                    Style::default().fg(C_BORDER),
                ),
            ]))
        })
        .collect();

    let reciters = List::new(items).block(
        Block::default()
            .title(Span::styled(
                format!(
                    " RECITERS ({}/{}) ",
                    visible_reciters.len(),
                    app.reciters.len()
                ),
                Style::default().fg(C_MED).add_modifier(Modifier::BOLD),
            ))
            .borders(Borders::ALL)
            .border_style(focus_border_style(app.focus == Focus::Reciters)),
    );
    frame.render_widget(reciters, cols[0]);
    app.reciter_scroll = adjust_scroll(
        current_visible_reciter,
        app.reciter_scroll,
        app.reciter_viewport_height,
    );

    let surah_list = app.selected_surah_list();
    let surah_visible_height = cols[1].height.saturating_sub(2) as usize;
    app.surah_viewport_height = surah_visible_height.max(1);
    let surah_items: Vec<ListItem> = surah_list
        .iter()
        .enumerate()
        .skip(app.surah_scroll)
        .take(surah_visible_height)
        .map(|(index, &surah_num)| {
            let is_selected = index == app.selected_surah && app.focus == Focus::Surahs;
            let is_playing = app.playing_surah_number() == Some(surah_num)
                && app.playing_reciter == Some(app.selected_reciter);
            let reciter_id = app.selected_reciter_id().unwrap_or_default();
            let is_favorite = app.surah_is_favorite(reciter_id, surah_num);
            let name = app
                .surah_display_name(surah_num)
                .map(str::to_string)
                .unwrap_or_else(|| format!("Surah {}", surah_num));

            let style = list_item_style(is_selected, is_playing);
            let icon = list_item_icon(is_playing, is_favorite);
            let badge = app
                .download_status_label(reciter_id, surah_num)
                .unwrap_or_default();
            ListItem::new(Line::from(vec![
                Span::styled(
                    format!("{} {:>3}. ", icon, surah_num),
                    Style::default().fg(C_DIM),
                ),
                Span::styled(name, style),
                Span::styled(
                    if badge.is_empty() {
                        String::new()
                    } else {
                        format!("  {}", badge)
                    },
                    Style::default().fg(C_AMBER),
                ),
            ]))
        })
        .collect();

    let surahs_widget = List::new(surah_items).block(
        Block::default()
            .title(Span::styled(
                format!(" SURAHS ({}) ", surah_list.len()),
                Style::default().fg(C_MED).add_modifier(Modifier::BOLD),
            ))
            .borders(Borders::ALL)
            .border_style(focus_border_style(app.focus == Focus::Surahs)),
    );
    frame.render_widget(surahs_widget, cols[1]);

    let bottom = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(chunks[1]);
    render_simple_list(
        frame,
        bottom[0],
        " RECENT ",
        app.active_recent()
            .iter()
            .take(4)
            .map(|entry| format!("{:03} @ {}s", entry.surah_id, entry.position_secs as u32))
            .collect(),
    );
    render_simple_list(
        frame,
        bottom[1],
        " DOWNLOAD QUEUE ",
        app.download_preview(4),
    );
}

fn render_listen(frame: &mut Frame, app: &mut App, area: Rect) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(65), Constraint::Percentage(35)])
        .split(area);
    let left = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(12), Constraint::Length(4)])
        .split(cols[0]);

    render_current_ayah_text(frame, app, left[0], false);
    render_now_playing(frame, app, left[1]);

    let sidebar = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(7),
            Constraint::Length(7),
            Constraint::Min(6),
        ])
        .split(cols[1]);
    render_up_next(frame, app, sidebar[0]);
    render_recent(frame, app, sidebar[1]);
    render_simple_list(frame, sidebar[2], " DOWNLOADS ", app.download_preview(6));
}

fn render_study(frame: &mut Frame, app: &mut App, area: Rect) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(38), Constraint::Percentage(62)])
        .split(area);
    let right = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(10), Constraint::Length(3)])
        .split(cols[1]);
    let ayah_visible_height = cols[0].height.saturating_sub(2) as usize;
    app.study_viewport_height = ayah_visible_height.max(1);
    app.study_scroll = adjust_scroll(
        app.selected_ayah_index,
        app.study_scroll,
        app.study_viewport_height,
    );

    let ayah_items: Vec<ListItem> = app
        .study_ayahs()
        .iter()
        .enumerate()
        .skip(app.study_scroll)
        .take(ayah_visible_height.max(1))
        .map(|(index, ayah)| {
            let is_selected = app.selected_ayah_index == index;
            let is_current = app.mushaf.current_ayah() == Some(ayah.ayah);
            let style = list_item_style(is_selected, is_current);
            ListItem::new(Line::from(vec![
                Span::styled(format!("{:>3} ", ayah.ayah), Style::default().fg(C_DIM)),
                Span::styled(
                    app.study_ayah_display_text(index)
                        .unwrap_or(ayah.text.as_str())
                        .to_string(),
                    style,
                ),
            ]))
        })
        .collect();

    let sidebar = List::new(ayah_items).block(
        Block::default()
            .title(Span::styled(
                " AYAHS ",
                Style::default().fg(C_MED).add_modifier(Modifier::BOLD),
            ))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(C_BORDER)),
    );
    frame.render_widget(sidebar, cols[0]);

    render_current_ayah_text(frame, app, right[0], true);
    let loop_label = if app.repeat_current_ayah {
        "Loop: current ayah".to_string()
    } else if let Some((start, end)) = app.loop_range {
        format!("Loop: ayah {} → {}", start, end)
    } else {
        "Loop: off".to_string()
    };
    frame.render_widget(
        Paragraph::new(format!(
            "{}  │  [Enter] jump  [y] repeat ayah  [[]/[ ]] set loop  [Esc] clear",
            loop_label
        ))
        .style(Style::default().fg(C_MED)),
        right[1],
    );
}

fn render_settings(frame: &mut Frame, app: &mut App, area: Rect) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(10), Constraint::Length(4)])
        .split(area);

    let fields = [
        crate::app::SettingsField::PreferOffline,
        crate::app::SettingsField::CacheStreams,
        crate::app::SettingsField::DownloadDirectory,
        crate::app::SettingsField::DownloadConcurrency,
    ];
    let items: Vec<ListItem> = fields
        .into_iter()
        .map(|field| {
            let is_selected = app.settings_field == field;
            let value = if app.settings_edit_mode && app.settings_field == field {
                format!("{}_", app.settings_buffer)
            } else {
                app.settings_value(field)
            };
            let style = if is_selected {
                Style::default().fg(Color::White).bg(C_BG_SEL)
            } else {
                Style::default().fg(C_CREAM)
            };
            ListItem::new(Line::from(vec![
                Span::styled(format!("{}: ", field.label()), Style::default().fg(C_MED)),
                Span::styled(value, style),
            ]))
        })
        .collect();
    let list = List::new(items).block(
        Block::default()
            .title(Span::styled(
                " SETTINGS ",
                Style::default().fg(C_MED).add_modifier(Modifier::BOLD),
            ))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(C_BORDER)),
    );
    frame.render_widget(list, rows[0]);

    let help = if app.settings_edit_mode {
        "Type a path, then press Enter to save or Esc to cancel."
    } else {
        "Use ↑/↓ to choose a setting. Enter toggles/edit. ←/→ changes values. Esc closes."
    };
    frame.render_widget(
        Paragraph::new(format!(
            "{}  │  Files are stored under the selected reciter folder.",
            help
        ))
        .style(Style::default().fg(C_MED)),
        rows[1],
    );
}

fn render_now_playing(frame: &mut Frame, app: &App, area: Rect) {
    let reciter_name = app
        .playing_reciter
        .and_then(|index| app.reciter_display_name(index))
        .unwrap_or("No reciter selected")
        .to_string();

    let surah_display = app
        .playing_surah_number()
        .map(|surah_num| {
            let display_name = app.surah_display_name(surah_num).unwrap_or("Unknown surah");
            format!("{:03} - {}", surah_num, display_name)
        })
        .unwrap_or_else(|| "Press Enter to play".to_string());

    let source = app.current_source_label().unwrap_or("Idle");
    let summary = Paragraph::new(format!(
        "{surah_display}
{reciter_name}
{source}"
    ))
    .alignment(Alignment::Center)
    .style(Style::default().fg(C_CREAM).add_modifier(Modifier::BOLD))
    .block(
        Block::default()
            .title(Span::styled(" NOW PLAYING ", Style::default().fg(C_DIM)))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(C_BORDER)),
    );
    frame.render_widget(summary, area);
}

fn render_up_next(frame: &mut Frame, app: &App, area: Rect) {
    render_simple_list(frame, area, " UP NEXT ", app.up_next(5));
}

fn render_recent(frame: &mut Frame, app: &App, area: Rect) {
    render_simple_list(
        frame,
        area,
        " RECENT ",
        app.active_recent()
            .iter()
            .take(5)
            .map(|entry| format!("{:03} @ {}s", entry.surah_id, entry.position_secs as u32))
            .collect(),
    );
}

fn render_current_ayah_text(frame: &mut Frame, app: &mut App, area: Rect, compact: bool) {
    let title = if compact {
        " CURRENT AYAH "
    } else {
        " CURRENT AYAH TEXT "
    };
    let body = if let Some(status) = app.ayah_text_status() {
        status.to_string()
    } else if app.current_ayah_text().is_some() {
        String::new()
    } else {
        "Play a surah to show the active ayah text.".to_string()
    };
    let title = if let Some(ayah) = app.mushaf.current_ayah() {
        format!("{title} — Ayah {ayah}")
    } else {
        title.to_string()
    };
    let inner = inner_panel_area(area);
    let current_ayah_text = app.current_ayah_text().map(ToOwned::to_owned);
    app.ayah_text_panel
        .update(current_ayah_text.as_deref(), inner);

    let has_text = !app.ayah_text_panel.rendered_lines().is_empty();
    let outer = Block::default()
        .title(Span::styled(title, Style::default().fg(C_GREEN)))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(C_BORDER));
    frame.render_widget(outer, area);

    if has_text {
        let lines = app
            .ayah_text_panel
            .rendered_lines()
            .iter()
            .cloned()
            .map(Line::from)
            .collect::<Vec<_>>();
        let longest = app
            .ayah_text_panel
            .rendered_lines()
            .iter()
            .map(|line| line.chars().count())
            .max()
            .unwrap_or(12) as u16;
        let max_card_width = inner.width.saturating_sub(2).max(28);
        let card_width = longest.saturating_add(8).clamp(28, max_card_width);
        let card_height = (lines.len() as u16)
            .saturating_add(2)
            .clamp(3, inner.height.max(3));
        let card = centered_rect(card_width, card_height, inner);
        let paragraph = Paragraph::new(Text::from(lines))
            .alignment(Alignment::Center)
            .style(
                Style::default()
                    .fg(C_CARD_TEXT)
                    .add_modifier(Modifier::BOLD),
            )
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(C_CARD_BORDER))
                    .style(Style::default().bg(C_CARD_BG)),
            );
        frame.render_widget(paragraph, card);
    } else {
        let paragraph = Paragraph::new(Text::from(body))
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: true })
            .style(Style::default().fg(C_MED));
        frame.render_widget(paragraph, inner);
    }
}

fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let width = width.min(area.width).max(1);
    let height = height.min(area.height).max(1);
    Rect {
        x: area.x + (area.width.saturating_sub(width) / 2),
        y: area.y + (area.height.saturating_sub(height) / 2),
        width,
        height,
    }
}

fn render_player(frame: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(area);

    let play_icon = if app.is_playing() { "▶" } else { "⏸" };
    let track_name = app
        .playing_surah_number()
        .and_then(|surah_num| app.surah_display_name(surah_num))
        .unwrap_or("—")
        .to_string();
    let repeat_label = app.repeat_mode.label();

    let now = Line::from(vec![
        Span::styled(format!(" {} ", play_icon), Style::default().fg(C_GREEN)),
        Span::styled(
            track_name,
            Style::default().fg(C_CREAM).add_modifier(Modifier::BOLD),
        ),
        Span::styled(format!("  {}x", app.speed), Style::default().fg(C_AMBER)),
        Span::styled(format!("  │  {}", repeat_label), Style::default().fg(C_DIM)),
    ]);
    frame.render_widget(Paragraph::new(now), chunks[0]);

    let ratio = if app.duration > 0.0 {
        (app.position / app.duration).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let gauge = Gauge::default()
        .gauge_style(Style::default().fg(C_GREEN).bg(Color::Rgb(44, 40, 36)))
        .ratio(ratio)
        .label(format!(
            " {} / {} ",
            fmt_time(app.position),
            fmt_time(app.duration)
        ));
    frame.render_widget(gauge, chunks[1]);

    let controls = Paragraph::new(Line::from(vec![
        Span::styled("[Space]", Style::default().fg(C_MED)),
        Span::styled(" pause  ", Style::default().fg(C_DIM)),
        Span::styled("[←→]", Style::default().fg(C_MED)),
        Span::styled(" seek  ", Style::default().fg(C_DIM)),
        Span::styled("[n/p]", Style::default().fg(C_MED)),
        Span::styled(" prev/next  ", Style::default().fg(C_DIM)),
        Span::styled("[j/k]", Style::default().fg(C_MED)),
        Span::styled(" move  ", Style::default().fg(C_DIM)),
        Span::styled("[q]", Style::default().fg(C_MED)),
        Span::styled(" quit", Style::default().fg(C_DIM)),
    ]));
    frame.render_widget(controls, chunks[2]);
}

fn render_simple_list(frame: &mut Frame, area: Rect, title: &str, items: Vec<String>) {
    let items = if items.is_empty() {
        vec![ListItem::new(Line::from(vec![Span::styled(
            "Nothing here yet",
            Style::default().fg(C_DIM),
        )]))]
    } else {
        items
            .into_iter()
            .map(|item| ListItem::new(Line::from(item)))
            .collect::<Vec<_>>()
    };
    let widget = List::new(items).block(
        Block::default()
            .title(Span::styled(
                title,
                Style::default().fg(C_MED).add_modifier(Modifier::BOLD),
            ))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(C_BORDER)),
    );
    frame.render_widget(widget, area);
}

#[cfg(test)]
mod tests {
    use super::ui;
    use crate::api::{AyahText, Moshaf, Reciter, Surah};
    use crate::app::{App, Mode};
    use ratatui::{Terminal, backend::TestBackend};

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
                    surah_list: "1".to_string(),
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

    fn render_lines(app: &mut App, width: u16, height: u16) -> Vec<String> {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).expect("test terminal");
        let frame = terminal.draw(|frame| ui(frame, app)).expect("draw frame");

        (0..frame.area.height)
            .map(|y| {
                (0..frame.area.width)
                    .map(|x| frame.buffer[(x, y)].symbol().to_string())
                    .collect::<String>()
            })
            .collect()
    }

    #[test]
    fn browse_screen_shows_reciters_surahs_and_filter_line() {
        let mut app = App::new_for_test();
        app.loading = false;
        app.set_library_data(sample_reciters(), sample_surahs());

        let lines = render_lines(&mut app, 80, 22);

        assert!(lines.iter().any(|line| line.contains("Quran MP3")));
        assert!(lines.iter().any(|line| line.contains("Filter:")));
        assert!(lines.iter().any(|line| line.contains("Reader One")));
        assert!(lines.iter().any(|line| line.contains("Fatiha")));
    }

    #[test]
    fn controls_show_player_startup_error() {
        let mut app = App::new_for_test();
        app.loading = false;
        app.set_library_data(sample_reciters(), sample_surahs());

        let lines = render_lines(&mut app, 100, 22);

        assert!(
            lines
                .iter()
                .any(|line| line.contains("test stub player unavailable"))
        );
    }

    #[test]
    fn listen_screen_renders_current_ayah_and_now_playing() {
        let mut app = App::new_for_test();
        app.loading = false;
        app.mode = Mode::Listen;
        app.set_library_data(sample_reciters(), sample_surahs());
        app.playing_reciter = Some(0);
        app.playing_surah = Some(1);
        app.set_test_current_ayah(1);
        app.set_test_ayah_texts(
            1,
            vec![AyahText {
                ayah: 1,
                text: "Hello  world".to_string(),
            }],
        );

        let lines = render_lines(&mut app, 96, 24);

        assert!(lines.iter().any(|line| line.contains("NOW PLAYING")));
        assert!(lines.iter().any(|line| line.contains("Fatiha")));
        assert!(lines.iter().any(|line| line.contains("Hello  world")));
        assert!(lines.iter().any(|line| line.contains("UP NEXT")));
    }

    #[test]
    fn settings_screen_renders_configurable_options() {
        let mut app = App::new_for_test();
        app.loading = false;
        app.open_settings();
        app.set_library_data(sample_reciters(), sample_surahs());

        let lines = render_lines(&mut app, 96, 24);

        assert!(lines.iter().any(|line| line.contains("SETTINGS")));
        assert!(
            lines
                .iter()
                .any(|line| line.contains("Prefer offline playback"))
        );
        assert!(
            lines
                .iter()
                .any(|line| line.contains("Save streams while playing"))
        );
    }

    #[test]
    fn study_screen_scrolls_selected_ayah_into_view() {
        let mut app = App::new_for_test();
        app.loading = false;
        app.mode = Mode::Study;
        app.set_library_data(sample_reciters(), sample_surahs());
        app.set_test_ayah_texts(
            1,
            (1..=20)
                .map(|ayah| AyahText {
                    ayah,
                    text: format!("Ayah {ayah:02}"),
                })
                .collect(),
        );
        app.selected_ayah_index = 11;

        let lines = render_lines(&mut app, 72, 16);

        assert!(lines.iter().any(|line| line.contains("Ayah 12")));
        assert!(!lines.iter().any(|line| line.contains("Ayah 01")));
    }
}
