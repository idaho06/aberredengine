//! The engine's own loading screen: a dark-grey overlay with the engine name
//! and a progress bar drawn with default-font glyphs.
//!
//! It is shown while the game is in `Setup` and is not a scene: it fires no
//! scene events and has no `SceneManager` entry. [`show_loading_screen`] and
//! [`hide_loading_screen`] run as the `SHOW_LOADING_SCREEN` and
//! `HIDE_LOADING_SCREEN` hooks on entering and leaving `Setup`.
//!
//! The layout functions are pure so the screen's text can be pinned without
//! a window or font metrics.

use bevy_ecs::prelude::*;

use crate::components::dynamic_text::DynamicText;
use crate::components::screen_position::ScreenPosition;
use crate::components::shadow::Shadow;
use crate::components::z_index::ZIndex;
use crate::math::Color;
use crate::resources::font_key::DEFAULT_FONT_KEY;
use crate::resources::game_config::GameConfig;
use crate::resources::loading_hold::LoadingHold;
use crate::resources::pending_assets::PendingAssets;
use crate::resources::render_size::RenderSize;

/// Cells between the bar's two side borders.
pub const BAR_INNER_WIDTH: usize = 30;

/// Background shown while the loading scene is active.
pub const BACKGROUND: Color = Color::new(32, 32, 32, 255);

/// Seconds `Setup` lasts at least while the screen is up, so a fast load
/// still shows it.
pub const MIN_DISPLAY_SECONDS: f32 = 1.0;

/// Height of one text row: the default font's cell height in pixels.
const ROW_HEIGHT: f32 = 14.0;

/// Rows the engine name is tall: it is drawn three cells high.
const TITLE_ROWS: f32 = 3.0;

/// Rows from the title's top to the subtitle's: the title and a gap.
const SUBTITLE_OFFSET_ROWS: f32 = TITLE_ROWS + 1.0;

/// Rows from the title's top to the bar's top: title, gap, subtitle, gap.
const BAR_OFFSET_ROWS: f32 = SUBTITLE_OFFSET_ROWS + 2.0;

/// Rows the whole screen occupies: the title block plus the three bar rows.
const TOTAL_ROWS: f32 = BAR_OFFSET_ROWS + 3.0;

/// Marks every entity of the loading screen, so hiding it despawns exactly
/// these.
#[derive(Component)]
pub struct EngineLoadingScreen;

/// Marks the bar's middle row, the only text that changes while loading.
#[derive(Component)]
pub struct LoadingBar;

/// The game's background color, kept while the loading screen paints its own.
#[derive(Resource)]
pub struct SavedBackground(pub Color);

/// Shows the loading screen: saves the game's background color and paints
/// [`BACKGROUND`], then spawns the texts stacked on the render size's
/// vertical center, with the bar at the current load progress.
///
/// The texts start at x 0: they are measured once the default font's metrics
/// arrive, which is after this runs, and [`center_loading_screen`] centers
/// them then.
pub fn show_loading_screen(
    mut commands: Commands,
    mut config: ResMut<GameConfig>,
    render_size: Res<RenderSize>,
    pending: Res<PendingAssets>,
) {
    commands.insert_resource(SavedBackground(config.background_color));
    commands.insert_resource(LoadingHold::for_seconds(MIN_DISPLAY_SECONDS, config.sim_hz));
    config.background_color = BACKGROUND;

    let top = ((render_size.h as f32 - TOTAL_ROWS * ROW_HEIGHT) / 2.0).floor();
    let text =
        |content: &str, size: f32| DynamicText::new(content, DEFAULT_FONT_KEY, size, Color::WHITE);

    // The shadow is one source pixel at the text's scale.
    for (row, content, rows_tall) in [
        (0.0, "AberredEngine", TITLE_ROWS),
        (SUBTITLE_OFFSET_ROWS, "Loading...", 1.0),
    ] {
        commands.spawn((
            EngineLoadingScreen,
            text(content, rows_tall * ROW_HEIGHT),
            ScreenPosition::new(0.0, top + row * ROW_HEIGHT),
            ZIndex(0.0),
            Shadow::default_color(rows_tall, rows_tall),
        ));
    }

    let rows = [
        bar_top(BAR_INNER_WIDTH),
        bar_middle(pending.progress(), BAR_INNER_WIDTH),
        bar_bottom(BAR_INNER_WIDTH),
    ];
    for (index, content) in rows.iter().enumerate() {
        let position =
            ScreenPosition::new(0.0, top + (BAR_OFFSET_ROWS + index as f32) * ROW_HEIGHT);
        let mut row = commands.spawn((
            EngineLoadingScreen,
            text(content, ROW_HEIGHT),
            position,
            ZIndex(0.0),
        ));
        if index == 1 {
            row.insert(LoadingBar);
        }
    }
}

/// Centers each loading-screen text horizontally on its measured width.
///
/// Texts have no width until their font's metrics arrive and
/// `dynamic_text_size_system` measures them; a text re-centers whenever its
/// content changes size.
pub fn center_loading_screen(
    render_size: Res<RenderSize>,
    mut texts: Query<(&DynamicText, &mut ScreenPosition), With<EngineLoadingScreen>>,
) {
    for (text, mut position) in &mut texts {
        let width = text.size().x;
        if width <= 0.0 {
            continue;
        }
        let x = centered_x(render_size.w as f32, width);
        if position.pos.x != x {
            position.pos.x = x;
        }
    }
}

/// Counts the minimum display time down, one sim tick at a time.
pub fn tick_loading_hold(hold: Option<ResMut<LoadingHold>>) {
    if let Some(mut hold) = hold {
        hold.tick();
    }
}

/// Hides the loading screen: despawns its entities, drops the minimum display
/// time and gives the game's background color back.
pub fn hide_loading_screen(
    mut commands: Commands,
    screens: Query<Entity, With<EngineLoadingScreen>>,
    mut config: ResMut<GameConfig>,
    saved: Option<Res<SavedBackground>>,
) {
    for entity in &screens {
        commands.entity(entity).despawn();
    }
    commands.remove_resource::<LoadingHold>();
    if let Some(saved) = saved {
        config.background_color = saved.0;
        commands.remove_resource::<SavedBackground>();
    }
}

/// Redraws the bar's middle row when the pending loads change.
pub fn update_loading_bar(
    pending: Res<PendingAssets>,
    mut bars: Query<&mut DynamicText, With<LoadingBar>>,
) {
    if !pending.is_changed() {
        return;
    }
    let row = bar_middle(pending.progress(), BAR_INNER_WIDTH);
    for mut bar in &mut bars {
        bar.set_text(&row);
    }
}

/// The bar's top border for `inner` cells between the corners.
pub fn bar_top(inner: usize) -> String {
    format!("┌{}┐", "─".repeat(inner))
}

/// The bar's bottom border for `inner` cells between the corners.
pub fn bar_bottom(inner: usize) -> String {
    format!("└{}┘", "─".repeat(inner))
}

/// The bar's middle row: `inner` cells filled left to right in proportion to
/// `progress` (clamped to `0..=1`, NaN counts as 0). The fill rounds down, so
/// the bar is full only when `progress` is complete.
pub fn bar_middle(progress: f32, inner: usize) -> String {
    // A float-to-integer `as` cast saturates: NaN and negatives become 0.
    let filled = ((progress * inner as f32) as usize).min(inner);
    format!("│{}{}│", "█".repeat(filled), " ".repeat(inner - filled))
}

/// The x that centers content of `content_width` in `screen_width`, floored
/// so glyphs stay on the pixel grid.
pub fn centered_x(screen_width: f32, content_width: f32) -> f32 {
    ((screen_width - content_width) / 2.0).floor()
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::math::Vec2;
    use crate::resources::font_metrics::test_support::default_font_metrics;
    use crate::resources::font_metrics::{FontMetricsStore, FontMetricsWarnCache};
    use crate::resources::loading_hold::LoadingHold;
    use crate::systems::dynamic_text_size::dynamic_text_size_system;
    use bevy_ecs::system::RunSystemOnce;

    /// A world as it is when `Setup` is entered: render size and config, but
    /// no font metrics yet (they arrive with the first message drain).
    fn world_before_font_metrics() -> World {
        let mut world = World::new();
        world.insert_resource(RenderSize { w: 640, h: 360 });
        world.insert_resource(GameConfig::new());
        world.insert_resource(PendingAssets::default());
        world.insert_resource(FontMetricsStore::default());
        world.insert_resource(FontMetricsWarnCache::default());
        world
    }

    /// Every loading-screen text as `(content, font size, position)`, top to
    /// bottom.
    fn rows(world: &mut World) -> Vec<(String, f32, Vec2)> {
        let mut rows: Vec<_> = world
            .query_filtered::<(&DynamicText, &ScreenPosition), With<EngineLoadingScreen>>()
            .iter(world)
            .map(|(text, position)| (text.text.to_string(), text.font_size, position.pos))
            .collect();
        rows.sort_by(|a, b| a.2.y.total_cmp(&b.2.y));
        rows
    }

    /// The screen is shown before the default font's metrics are in the
    /// store, so it centers once they arrive and the texts are measured.
    #[test]
    fn texts_are_centered_once_their_font_metrics_arrive() {
        let mut world = world_before_font_metrics();
        world.run_system_once(show_loading_screen).unwrap();

        world
            .resource_mut::<FontMetricsStore>()
            .0
            .insert(DEFAULT_FONT_KEY.to_owned(), default_font_metrics());
        world.run_system_once(dynamic_text_size_system).unwrap();
        world.run_system_once(center_loading_screen).unwrap();

        let mut checked = 0;
        for (text, position) in world
            .query_filtered::<(&DynamicText, &ScreenPosition), With<EngineLoadingScreen>>()
            .iter(&world)
        {
            assert!(text.size().x > 0.0, "{:?} is measured", text.text);
            assert_eq!(position.pos.x, centered_x(640.0, text.size().x));
            checked += 1;
        }
        assert_eq!(checked, 5);
    }

    /// The name is three cells tall; the subtitle and the bar stack below it
    /// on whole cells without overlapping.
    #[test]
    fn the_title_is_three_cells_tall_and_nothing_overlaps_it() {
        let mut world = world_before_font_metrics();
        world.run_system_once(show_loading_screen).unwrap();

        let rows = rows(&mut world);
        assert_eq!(rows[0].0, "AberredEngine");
        assert_eq!(rows[0].1, 42.0);
        for row in &rows[1..] {
            assert_eq!(row.1, 14.0);
        }
        assert!(rows[0].2.y + 42.0 <= rows[1].2.y, "subtitle below the name");
        assert!(rows[1].2.y + 14.0 <= rows[2].2.y, "bar below the subtitle");
        for row in &rows {
            assert_eq!((row.2.y - rows[0].2.y) % 14.0, 0.0, "on the cell grid");
        }
    }

    /// The hold counts from showing the screen to hiding it.
    #[test]
    fn showing_starts_the_hold_and_hiding_ends_it() {
        let mut world = world_before_font_metrics();
        world.run_system_once(show_loading_screen).unwrap();
        assert!(world.contains_resource::<LoadingHold>());

        world.run_system_once(hide_loading_screen).unwrap();
        assert!(!world.contains_resource::<LoadingHold>());
        assert!(rows(&mut world).is_empty());
    }

    #[test]
    fn the_top_and_bottom_rows_close_the_box() {
        assert_eq!(bar_top(3), "┌───┐");
        assert_eq!(bar_bottom(3), "└───┘");
        assert_eq!(bar_top(0), "┌┐");
    }

    #[test]
    fn the_middle_row_fills_from_the_left() {
        assert_eq!(bar_middle(0.0, 4), "│    │");
        assert_eq!(bar_middle(0.5, 4), "│██  │");
        assert_eq!(bar_middle(1.0, 4), "│████│");
    }

    #[test]
    fn every_row_is_inner_plus_two_cells_wide() {
        for progress in [0.0, 0.1, 0.5, 0.99, 1.0] {
            assert_eq!(bar_middle(progress, 7).chars().count(), 9);
        }
        assert_eq!(bar_top(7).chars().count(), 9);
        assert_eq!(bar_bottom(7).chars().count(), 9);
    }

    #[test]
    fn a_bar_is_full_only_when_progress_is_complete() {
        assert_eq!(bar_middle(0.99, 10), "│█████████ │");
        assert_eq!(bar_middle(0.01, 10), "│          │");
    }

    #[test]
    fn progress_outside_the_unit_interval_is_clamped() {
        assert_eq!(bar_middle(-1.0, 2), "│  │");
        assert_eq!(bar_middle(2.0, 2), "│██│");
        assert_eq!(bar_middle(f32::NAN, 2), "│  │");
    }

    #[test]
    fn centered_x_floors_to_a_whole_pixel() {
        assert_eq!(centered_x(100.0, 40.0), 30.0);
        assert_eq!(centered_x(101.0, 40.0), 30.0);
        assert_eq!(centered_x(100.0, 41.0), 29.0);
    }
}
