pub mod disc_view;
pub mod header;
pub mod help_view;
pub mod library_view;
pub mod lyrics_view;
pub mod queue_view;
pub mod telemetry;
pub mod theme;
pub mod visualizer;

use crate::app::{ActiveTab, App};
use disc_view::DiscView;
use header::HeaderWidget;
use help_view::HelpModal;
use library_view::LibraryView;
use lyrics_view::LyricsView;
use queue_view::QueueView;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

pub fn draw(f: &mut Frame, app: &App) {
    let size = f.area();
    let geom = App::get_cover_geometry(size);

    // Responsive centered container (no solid background fill, 100% transparent to terminal!)
    let card_rect = geom.card_rect;

    let vertical_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Top Header (Tabs + Like)
            Constraint::Min(10),   // Main View (Disc / Lyrics / Queue / Lib)
            Constraint::Length(1), // Toast / Status line
        ])
        .split(card_rect);

    let header_area = vertical_chunks[0];
    let body_area = vertical_chunks[1];
    let toast_area = vertical_chunks[2];

    // 1. Top Header Bar
    f.render_widget(
        HeaderWidget {
            _song: app.current_song.as_ref(),
            active_tab: app.active_tab,
            theme: &app.theme,
            is_favorite: app.is_favorite,
        },
        header_area,
    );

    // 2. Body View according to active tab
    match app.active_tab {
        ActiveTab::Player => {
            f.render_widget(
                DiscView {
                    song: app.current_song.as_ref(),
                    status: &app.mpd_status,
                    cover_art: &app.cover_art,
                    rotation_angle: app.rotation_angle,
                    theme: &app.theme,
                    tick_counter: app.tick_counter,
                    cover_mode: app.cover_mode,
                    bottom_mode: app.bottom_mode,
                    lyrics_mgr: &app.lyrics_mgr,
                    clicked_button: app.clicked_button,
                    geom,
                },
                body_area,
            );
        }
        ActiveTab::Lyrics => {
            f.render_widget(
                LyricsView {
                    lyrics_mgr: &app.lyrics_mgr,
                    song: app.current_song.as_ref(),
                    elapsed: app.mpd_status.elapsed,
                    scroll_offset: app.lyrics_scroll_offset,
                    theme: &app.theme,
                },
                body_area,
            );
        }
        ActiveTab::Queue => {
            f.render_widget(
                QueueView {
                    playlist: &app.filtered_queue(),
                    current_song_id: app.current_song.as_ref().map(|s| s.id),
                    status: &app.mpd_status,
                    selected_idx: app.queue_selected_idx,
                    search_query: &app.queue_search_query,
                    is_searching: app.is_searching,
                    theme: &app.theme,
                },
                body_area,
            );
        }
        ActiveTab::Library => {
            f.render_widget(
                LibraryView {
                    files: &app.filtered_library(),
                    selected_idx: app.library_selected_idx,
                    search_query: &app.library_search_query,
                    is_searching: app.is_searching,
                    theme: &app.theme,
                },
                body_area,
            );
        }
    }

    // 3. Status Toast if present
    if let Some(msg) = &app.status_message {
        let toast = Paragraph::new(format!("  {}  ", msg))
            .style(app.theme.accent())
            .alignment(ratatui::layout::Alignment::Center);
        f.render_widget(toast, toast_area);
    }

    // 4. Help modal if toggled
    if app.show_help {
        f.render_widget(HelpModal { theme: &app.theme }, size);
    }
}
