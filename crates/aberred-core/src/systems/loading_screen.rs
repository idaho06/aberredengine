//! The engine's own loading screen: a dark-grey overlay with the engine name
//! and a progress bar made of two shapes, a frame and a fill.
//!
//! It is shown while the game is in `Setup` and is not a scene: it fires no
//! scene events and has no `SceneManager` entry. [`show_loading_screen`] and
//! [`hide_loading_screen`] run as the `SHOW_LOADING_SCREEN` and
//! `HIDE_LOADING_SCREEN` hooks on entering and leaving `Setup`.
//!
//! The bar needs no text measurement, so it is centered the moment it is
//! shown.

use bevy_ecs::prelude::*;

use crate::components::dynamic_text::DynamicText;
use crate::components::screen_position::ScreenPosition;
use crate::components::shadow::Shadow;
use crate::components::shape::Shape;
use crate::components::z_index::ZIndex;
use crate::math::{Color, Vec2};
use crate::resources::font_key::DEFAULT_FONT_KEY;
use crate::resources::game_config::GameConfig;
use crate::resources::loading_hold::LoadingHold;
use crate::resources::pending_assets::PendingAssets;
use crate::resources::render_size::RenderSize;

/// Outer width of the progress bar's frame, in pixels.
pub const BAR_WIDTH: f32 = 256.0;

/// Outer height of the progress bar's frame, in pixels.
pub const BAR_HEIGHT: f32 = 16.0;

/// Gap between the frame's edge and the fill: the 1 px outline plus a 1 px
/// gap, so the two never overlap.
pub const BAR_PADDING: f32 = 2.0;

/// Width of the fill when loading is complete.
pub const BAR_FILL_WIDTH: f32 = BAR_WIDTH - 2.0 * BAR_PADDING;

/// Height of the fill.
pub const BAR_FILL_HEIGHT: f32 = BAR_HEIGHT - 2.0 * BAR_PADDING;

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

/// Pixels from the title's top to the bar's bottom.
const TOTAL_HEIGHT: f32 = BAR_OFFSET_ROWS * ROW_HEIGHT + BAR_HEIGHT;

/// Marks every entity of the loading screen, so hiding it despawns exactly
/// these.
#[derive(Component)]
pub struct EngineLoadingScreen;

/// Marks the bar's fill, the only entity that changes while loading.
#[derive(Component)]
pub struct LoadingBar;

/// The game's background color, kept while the loading screen paints its own.
#[derive(Resource)]
pub struct SavedBackground(pub Color);

/// Shows the loading screen: saves the game's background color and paints
/// [`BACKGROUND`], then spawns the texts and the bar stacked on the render
/// size's vertical center, with the bar at the current load progress.
///
/// The bar is centered at once. The texts start at x 0: they are measured
/// once the default font's metrics arrive, which is after this runs, and
/// [`center_loading_screen`] centers them then.
pub fn show_loading_screen(
    mut commands: Commands,
    mut config: ResMut<GameConfig>,
    render_size: Res<RenderSize>,
    pending: Res<PendingAssets>,
) {
    commands.insert_resource(SavedBackground(config.background_color));
    commands.insert_resource(LoadingHold::for_seconds(MIN_DISPLAY_SECONDS, config.sim_hz));
    config.background_color = BACKGROUND;

    let top = ((render_size.h as f32 - TOTAL_HEIGHT) / 2.0).floor();
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

    let x = centered_x(render_size.w as f32, BAR_WIDTH);
    let bar_top = top + BAR_OFFSET_ROWS * ROW_HEIGHT;
    commands.spawn((
        EngineLoadingScreen,
        Shape::rect(Vec2::new(BAR_WIDTH, BAR_HEIGHT), Color::WHITE),
        ScreenPosition::new(x, bar_top),
        ZIndex(0.0),
    ));
    commands.spawn((
        EngineLoadingScreen,
        LoadingBar,
        bar_fill(bar_fill_width(pending.progress(), BAR_FILL_WIDTH)),
        ScreenPosition::new(x + BAR_PADDING, bar_top + BAR_PADDING),
        ZIndex(0.0),
    ));
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

/// Resizes the bar's fill when the pending loads change.
pub fn update_loading_bar(
    pending: Res<PendingAssets>,
    mut bars: Query<&mut Shape, With<LoadingBar>>,
) {
    if !pending.is_changed() {
        return;
    }
    let fill = bar_fill(bar_fill_width(pending.progress(), BAR_FILL_WIDTH));
    for mut bar in &mut bars {
        *bar = fill.clone();
    }
}

/// The bar's fill: a filled rect `width` wide.
fn bar_fill(width: f32) -> Shape {
    Shape::rect(Vec2::new(width, BAR_FILL_HEIGHT), Color::WHITE).filled()
}

/// The fill's width for `progress` of a bar `full` pixels wide: `progress`
/// (clamped to `0..=1`, NaN counts as 0) of `full`, rounded down to a whole
/// pixel, so the bar is full only when `progress` is complete.
pub fn bar_fill_width(progress: f32, full: f32) -> f32 {
    // A float-to-integer `as` cast saturates: NaN and negatives become 0.
    ((progress * full) as u32 as f32).min(full)
}

/// The x that centers content of `content_width` in `screen_width`, floored
/// so content stays on the pixel grid.
pub fn centered_x(screen_width: f32, content_width: f32) -> f32 {
    ((screen_width - content_width) / 2.0).floor()
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::components::shape::ShapeKind;
    use crate::protocol::asset_kind::AssetKind;
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

    /// Every loading-screen entity, texts and shapes alike.
    fn screen_entities(world: &mut World) -> usize {
        world
            .query_filtered::<Entity, With<EngineLoadingScreen>>()
            .iter(world)
            .count()
    }

    /// The bar's shapes as `(position, size, filled)`, top to bottom: the
    /// frame, then the fill.
    fn bar_shapes(world: &mut World) -> Vec<(Vec2, Vec2, bool)> {
        let mut shapes: Vec<_> = world
            .query_filtered::<(&Shape, &ScreenPosition), With<EngineLoadingScreen>>()
            .iter(world)
            .map(|(shape, position)| {
                let ShapeKind::Rect { size } = &shape.kind else {
                    panic!("the bar is made of rects");
                };
                (position.pos, *size, shape.filled)
            })
            .collect();
        shapes.sort_by(|a, b| a.0.y.total_cmp(&b.0.y));
        shapes
    }

    /// A world as [`world_before_font_metrics`], with `queued` loads pending
    /// of which `settled` are answered.
    fn world_with_progress(settled: usize, queued: usize) -> World {
        let mut world = world_before_font_metrics();
        let keys: Vec<String> = (0..queued).map(|i| format!("t{i}")).collect();
        let mut pending = PendingAssets::default();
        for key in &keys {
            pending.queue(AssetKind::Texture, key);
        }
        for key in &keys[..settled] {
            pending.settle(AssetKind::Texture, key);
        }
        world.insert_resource(pending);
        world
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
        assert_eq!(checked, 2);
    }

    /// The name is three cells tall; the subtitle and the bar stack below it
    /// on whole cells without overlapping.
    #[test]
    fn the_title_is_three_cells_tall_and_nothing_overlaps_it() {
        let mut world = world_before_font_metrics();
        world.run_system_once(show_loading_screen).unwrap();

        let rows = rows(&mut world);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].0, "AberredEngine");
        assert_eq!(rows[0].1, 42.0);
        assert_eq!(rows[1].1, 14.0);
        assert!(rows[0].2.y + 42.0 <= rows[1].2.y, "subtitle below the name");
        for row in &rows {
            assert_eq!((row.2.y - rows[0].2.y) % 14.0, 0.0, "on the cell grid");
        }
        let frame_top = bar_shapes(&mut world)[0].0.y;
        assert!(rows[1].2.y + 14.0 <= frame_top, "bar below the subtitle");
    }

    /// The name, the subtitle and the bar stack leave the same margin above
    /// and below, to within a pixel.
    #[test]
    fn the_whole_stack_is_centered_vertically() {
        let mut world = world_before_font_metrics();
        world.run_system_once(show_loading_screen).unwrap();

        let top = rows(&mut world)[0].2.y;
        let bottom = bar_shapes(&mut world)[0].0.y + BAR_HEIGHT;
        assert!(
            (top - (360.0 - bottom)).abs() <= 1.0,
            "{top} above, {bottom} below"
        );
    }

    /// The hold counts from showing the screen to hiding it.
    #[test]
    fn showing_starts_the_hold_and_hiding_ends_it() {
        let mut world = world_before_font_metrics();
        world.run_system_once(show_loading_screen).unwrap();
        assert!(world.contains_resource::<LoadingHold>());
        assert_eq!(screen_entities(&mut world), 4);

        world.run_system_once(hide_loading_screen).unwrap();
        assert!(!world.contains_resource::<LoadingHold>());
        assert_eq!(screen_entities(&mut world), 0, "texts and shapes go");
    }

    #[test]
    fn the_bar_is_an_outlined_frame_and_an_empty_filled_rect() {
        let mut world = world_with_progress(0, 4);
        world.run_system_once(show_loading_screen).unwrap();

        let bar = bar_shapes(&mut world);
        assert_eq!(bar.len(), 2);
        let frame = (bar[0].1, bar[0].2);
        assert_eq!(frame, (Vec2::new(BAR_WIDTH, BAR_HEIGHT), false));
        let fill = (bar[1].1, bar[1].2);
        assert_eq!(fill, (Vec2::new(0.0, BAR_FILL_HEIGHT), true));
    }

    /// The bar needs no text measurement, so it is centered the moment it is
    /// shown, before any font metrics exist.
    #[test]
    fn the_bar_is_centered_before_any_font_metrics_exist() {
        let mut world = world_with_progress(0, 4);
        world.run_system_once(show_loading_screen).unwrap();

        let bar = bar_shapes(&mut world);
        let x = centered_x(640.0, BAR_WIDTH);
        assert_eq!(bar[0].0.x, x);
        assert_eq!(
            bar[1].0,
            Vec2::new(x + BAR_PADDING, bar[0].0.y + BAR_PADDING)
        );
    }

    /// The fill follows the answered loads, and is left alone while the
    /// pending loads do not change.
    #[test]
    fn update_loading_bar_resizes_the_fill_when_loads_are_answered() {
        let mut world = world_with_progress(0, 4);
        world.run_system_once(show_loading_screen).unwrap();
        let mut schedule = Schedule::default();
        schedule.add_systems(update_loading_bar);

        world
            .resource_mut::<PendingAssets>()
            .settle(AssetKind::Texture, "t0");
        schedule.run(&mut world);
        let expected = Vec2::new(bar_fill_width(0.25, BAR_FILL_WIDTH), BAR_FILL_HEIGHT);
        assert_eq!(bar_shapes(&mut world)[1].1, expected);

        let mut fills = world.query_filtered::<&mut Shape, With<LoadingBar>>();
        for mut shape in fills.iter_mut(&mut world) {
            shape.kind = ShapeKind::Rect {
                size: Vec2::new(7.0, 7.0),
            };
        }
        schedule.run(&mut world);
        assert_eq!(bar_shapes(&mut world)[1].1, Vec2::new(7.0, 7.0));

        world
            .resource_mut::<PendingAssets>()
            .settle(AssetKind::Texture, "t1");
        schedule.run(&mut world);
        let expected = Vec2::new(bar_fill_width(0.5, BAR_FILL_WIDTH), BAR_FILL_HEIGHT);
        assert_eq!(bar_shapes(&mut world)[1].1, expected);
    }

    #[test]
    fn the_fill_grows_from_zero_to_full_in_proportion_to_progress() {
        assert_eq!(bar_fill_width(0.0, 252.0), 0.0);
        assert_eq!(bar_fill_width(0.5, 252.0), 126.0);
        assert_eq!(bar_fill_width(1.0, 252.0), 252.0);
    }

    #[test]
    fn the_fill_rounds_down_so_only_complete_progress_fills_the_bar() {
        assert_eq!(bar_fill_width(0.99, 10.0), 9.0);
        assert_eq!(bar_fill_width(0.01, 10.0), 0.0);
    }

    #[test]
    fn the_fill_clamps_progress_outside_the_unit_interval() {
        assert_eq!(bar_fill_width(-1.0, 10.0), 0.0);
        assert_eq!(bar_fill_width(2.0, 10.0), 10.0);
        assert_eq!(bar_fill_width(f32::NAN, 10.0), 0.0);
    }

    #[test]
    fn centered_x_floors_to_a_whole_pixel() {
        assert_eq!(centered_x(100.0, 40.0), 30.0);
        assert_eq!(centered_x(101.0, 40.0), 30.0);
        assert_eq!(centered_x(100.0, 41.0), 29.0);
    }
}
