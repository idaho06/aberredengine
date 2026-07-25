use bevy_ecs::prelude::*;
use crossbeam_channel::{bounded, unbounded};

use super::builder::EngineBuilder;
use super::logic_thread::{LogicInit, logic_thread};
use crate::error::EngineError;
use crate::pacing::StatsWindow;
use crate::protocol::endpoints::{LogicBridge, shutdown_logic};
use crate::protocol::raw_input::InputSample;
use crate::protocol::render_logic::{LogicMsg, RenderMsg};
use crate::protocol::snapshot::{SnapshotConsumer, SnapshotPublisher};
use crate::resources::drawable_snapshot::DrawableSnapshot;
use crate::resources::gameconfig::default_render_fps;
use crate::resources::render::mirrors::RenderGameConfig;
use crate::resources::render::quit_requested::QuitRequested;
use crate::resources::render::scene_table::RenderSceneTable;
use crate::resources::render::thread_stats::RenderStats;

impl EngineBuilder {
    /// Build the engine and run the main loop.
    ///
    /// This consumes the builder and does not return until the game exits.
    /// Startup failures are logged, printed to stderr, and exit the process
    /// with status 1 -- this never silently returns after a failed startup.
    /// Use [`.try_run()`](Self::try_run) instead to handle the error yourself.
    pub fn run(self) {
        if let Err(err) = self.try_run() {
            log::error!("Failed to start engine: {err}");
            eprintln!("Failed to start engine: {err}");
            std::process::exit(1);
        }
    }

    /// Build the engine and run the main loop.
    ///
    /// This variant returns startup errors to the caller instead of logging
    /// them internally.
    ///
    /// Two `World`s run on two threads. The main thread owns the raylib
    /// window plus a small render `World` (GL stores, mirror entities
    /// reconciled from each received [`DrawableSnapshot`] -- there is no
    /// `DrawableSnapshot` resource on the render side); the spawned
    /// logic thread builds the gameplay
    /// `World` inside its own closure (so NonSend `LuaRuntime` is created on,
    /// and pinned to, that thread) and runs its sim schedule on its own
    /// `Pacer`-paced wall clock. Communication is crossbeam channels only
    /// ([`LogicMsg`]/[`RenderMsg`] — fully `Send` enums). Logic-thread
    /// startup errors are logged from that thread and surface as an
    /// immediate `RenderMsg::Quit`, not as an `Err` here.
    pub fn try_run(mut self) -> Result<(), EngineError> {
        crate::protocol::shutdown::install_panic_hook();
        log::info!("Hello, world! This is the Aberred Engine!");

        let use_scene_manager = !self.scenes.is_empty();

        self.validate_builder(use_scene_manager)?;
        let config = self.load_config()?;
        let (rl, thread, render_target) = Self::setup_window(&config)?;

        let (tx_logic, rx_logic) = unbounded::<LogicMsg>();
        let (tx_render, rx_render) = unbounded::<RenderMsg>();
        // Input gets its own dedicated bounded channel, separate
        // from the unbounded LogicMsg channel above — see LogicBridge::tx_input.
        // Capacity 64: sized for the worst supported ratio of render fps to
        // sim_hz, not a round number. sim_hz clamps to >= 15 (period ~66ms);
        // at 240fps that's up to ~16 backlogged samples per sim tick, so 64
        // gives headroom for uncapped-vsync/500fps+ cases too. InputSample is
        // ~300B, so 64 slots costs ~19KB — trivial.
        let (tx_input, rx_input) = bounded::<InputSample>(64);
        // Cloned before `rx_input` moves into `init` below -- the render
        // side keeps its own handle so `sample_and_send_input` can pop a
        // stale sample off a momentarily-full channel (see
        // `LogicBridge::rx_input`'s doc comment).
        let rx_input_render = rx_input.clone();
        // The DrawableSnapshot itself travels via a triple buffer,
        // not the RenderMsg channel above -- sim writes `snap_in`, render
        // reads `snap_out`, latest-wins, no queue growth. Seeded with
        // `DrawableSnapshot::default()`; never read before the sim's first
        // real publish (`Output::update()` gates it) -- the render world
        // seeds `RenderGameConfig` separately with the real loaded config
        // (`setup_render_world`), so this initial buffer value being a bare
        // default is harmless either way.
        let (snap_in, snap_out) =
            triple_buffer::TripleBuffer::new(&DrawableSnapshot::default()).split();

        // Render-side clone of the scene-descriptor table (fn pointers, cheap)
        // for gui/world-draw callback resolution against RenderActiveScene.
        let render_scene_table = use_scene_manager.then(|| {
            RenderSceneTable(
                self.scenes
                    .iter()
                    .map(|(name, desc)| (name.clone(), desc.clone()))
                    .collect(),
            )
        });

        let init = LogicInit {
            config: config.clone(),
            setup_hook: self.setup_hook.take(),
            enter_play_hook: self.enter_play_hook.take(),
            switch_scene_hook: self.switch_scene_hook.take(),
            update_hook: self.update_hook.take(),
            extra_systems: std::mem::take(&mut self.extra_systems),
            extra_observers: std::mem::take(&mut self.extra_observers),
            scenes: std::mem::take(&mut self.scenes),
            initial_scene: self.initial_scene.take(),
            #[cfg(feature = "lua")]
            lua_script: self.lua_script.take(),
            deterministic_seed: self.deterministic_seed,
            window_w: rl.get_screen_width(),
            window_h: rl.get_screen_height(),
            tx_render,
            rx_logic,
            rx_input,
            snapshot_publisher: Some(SnapshotPublisher(snap_in)),
            #[cfg(any(test, feature = "test-support"))]
            stub_audio: false,
            #[cfg(any(test, feature = "test-support"))]
            audio_stub_ends: None,
        };
        let handle = std::thread::Builder::new()
            .name("aberred-logic".into())
            .spawn(move || logic_thread(init))
            .map_err(|source| EngineError::ThreadSpawn { source })?;

        let mut render_world = Self::setup_render_world(
            config,
            rl,
            thread,
            render_target,
            render_scene_table,
            SnapshotConsumer(snap_out),
            LogicBridge {
                tx_logic,
                tx_input,
                rx_input: rx_input_render,
                rx_render,
                handle,
            },
        )?;
        let mut render_schedule = Self::build_render_schedule(&mut render_world)?;
        Self::render_main_loop(&mut render_world, &mut render_schedule);

        Ok(())
    }

    /// Render (main) thread loop: runs the schedule built by
    /// `build_render_schedule` each frame (see that fn's doc comment for the
    /// per-frame steps).
    ///
    /// Shutdown ordering: window close (or `RenderMsg::Quit`) -> send
    /// `LogicMsg::Shutdown` -> join the logic thread (which runs
    /// `shutdown_audio` and drops `LuaRuntime` on its own thread) -> the
    /// render world drops here (`ImguiBridge` teardown with the GL context
    /// alive) -> `RaylibHandle` drops last, closing the window.
    fn render_main_loop(world: &mut World, schedule: &mut Schedule) {
        #[cfg(feature = "tracy")]
        let _tracy = tracy_client::Client::start();

        // No dedicated Pacer here -- raylib's own target_fps/vsync
        // wait paces this loop, from inside the same schedule.run() call
        // this StatsWindow times. That means tick_avg_ms/achieved_hz read as
        // whole-frame time (vsync wait included), unlike SimStats/AudioStats'
        // "work excluding sleep" -- see RenderStats' doc comment. This is the
        // render thread's own implicit fps fallback, no longer shared with a
        // snapshot rate now that PRESENT decimates by sim-tick count instead
        // of wall-clock rate (see GameConfig::snapshot_skip).
        let target_fps = world.resource::<RenderGameConfig>().0.target_fps;
        let mut stats_window = StatsWindow::new(default_render_fps(target_fps));

        while !world
            .non_send::<raylib::RaylibHandle>()
            .window_should_close()
            && !world.resource::<QuitRequested>().0
            && crate::protocol::shutdown::running()
        {
            {
                crate::tracy::tracy_span!("render_schedule_run");
                let tick_start = std::time::Instant::now();
                schedule.run(world);
                let tick_work = tick_start.elapsed();
                if let Some(stats) = stats_window.record(tick_work) {
                    *world.resource_mut::<RenderStats>() = RenderStats(stats);
                }
            }
            world.clear_trackers();
            crate::tracy::tracy_frame_mark!();
        }

        // Shutdown: stop the logic thread first (it owns the audio bridge and
        // LuaRuntime), then let the render world / window drop after return.
        shutdown_logic(world);
    }
}
