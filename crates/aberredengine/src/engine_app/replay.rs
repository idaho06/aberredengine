//! Replay recording/playback runtime.
//!
//! [`ReplayRecorder`] streams `TickInput`s to a file as `logic_thread_main`
//! collects them; [`ReplayPlayer`] streams them back out, standing in for
//! the live `rx_input`/`rx_logic` collection stage. Both stream
//! entry-by-entry (`src/protocol/replay.rs`'s length-prefixed postcard
//! encoding) -- neither buffers a whole session in memory.

use std::fs::File;
use std::io::{self, BufReader, BufWriter, Read, Write};
use std::path::Path;

use aberred_core::error::EngineError;
use aberred_core::protocol::replay::{
    REPLAY_FORMAT_VERSION, REPLAY_MAGIC, ReplayEntry, ReplayHeader, config_digest,
};
use aberred_core::protocol::tick_input::TickInput;
use aberred_core::resources::gameconfig::GameConfig;

fn postcard_io_err(e: postcard::Error) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, e)
}

fn write_len_prefixed<W: Write>(writer: &mut W, bytes: &[u8]) -> io::Result<()> {
    writer.write_all(&(bytes.len() as u32).to_le_bytes())?;
    writer.write_all(bytes)
}

/// Reads one length-prefixed frame. `Ok(None)` on a clean end-of-file
/// (nothing read at all) -- any other truncation is a hard `Err`.
fn read_len_prefixed<R: Read>(reader: &mut R) -> io::Result<Option<Vec<u8>>> {
    let mut len_buf = [0u8; 4];
    match reader.read_exact(&mut len_buf) {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(e) => return Err(e),
    }
    let len = u32::from_le_bytes(len_buf) as usize;
    let mut buf = vec![0u8; len];
    reader.read_exact(&mut buf)?;
    Ok(Some(buf))
}

/// Standalone header-vs-config check (no file/thread I/O), so it's directly
/// unit-testable. Called from `EngineBuilder::try_run` right after
/// `ReplayPlayer::open_header`, before the logic thread is spawned.
///
/// The file magic is *not* checked here — "is this even a replay file"
/// belongs to opening (see [`ReplayPlayer::open_header`], which has the path
/// to name in the error); this fn only validates a successfully-read header
/// against the config it's about to be replayed into.
pub(crate) fn validate_replay_header(
    header: &ReplayHeader,
    config: &GameConfig,
) -> Result<(), EngineError> {
    if header.format_version != REPLAY_FORMAT_VERSION {
        return Err(EngineError::ReplayVersionMismatch {
            found: header.format_version,
            expected: REPLAY_FORMAT_VERSION,
        });
    }
    if header.sim_hz != config.sim_hz {
        return Err(EngineError::ReplaySimHzMismatch {
            found: header.sim_hz,
            expected: config.sim_hz,
        });
    }
    let expected_digest = config_digest(config);
    if header.config_digest != expected_digest {
        return Err(EngineError::ReplayConfigMismatch {
            found: header.config_digest,
            expected: expected_digest,
        });
    }
    Ok(())
}

/// Streaming writer: header once at `create`, then one length-prefixed
/// [`ReplayEntry`] per [`record_tick`](Self::record_tick)/
/// [`record_checkpoint`](Self::record_checkpoint) call, then a closing
/// [`ReplayEntry::End`] on [`finish`](Self::finish). Must be finalized
/// (`finish`) on every `logic_thread_main` exit path -- a file without an
/// `End` entry is a truncated replay; see that fn's `finalize_recorder`
/// helper.
pub(crate) struct ReplayRecorder {
    writer: BufWriter<File>,
    /// Consecutive empty ticks not yet flushed as a `ReplayEntry::EmptyRun`
    /// -- flushed lazily, right before the next non-empty entry (a `Tick` or
    /// a `Checkpoint`) is written, or at `finish`.
    pending_empty_run: u32,
    ticks_written: u64,
    /// Reused encode buffer for `write_entry` -- called up to once per sim
    /// tick while recording, so a fresh `Vec` per call would allocate at up
    /// to 240Hz; this keeps one allocation that stabilizes at the largest
    /// entry size instead.
    scratch: Vec<u8>,
}

impl ReplayRecorder {
    pub(crate) fn create(path: &Path, header: &ReplayHeader) -> io::Result<Self> {
        let file = File::create(path)?;
        let mut writer = BufWriter::new(file);
        let bytes = postcard::to_allocvec(header).map_err(postcard_io_err)?;
        write_len_prefixed(&mut writer, &bytes)?;
        Ok(Self {
            writer,
            pending_empty_run: 0,
            ticks_written: 0,
            scratch: Vec::new(),
        })
    }

    fn write_entry(&mut self, entry: &ReplayEntry) -> io::Result<()> {
        self.scratch.clear();
        self.scratch = postcard::to_extend(entry, std::mem::take(&mut self.scratch))
            .map_err(postcard_io_err)?;
        write_len_prefixed(&mut self.writer, &self.scratch)
    }

    fn flush_pending_empty_run(&mut self) -> io::Result<()> {
        if self.pending_empty_run > 0 {
            let n = self.pending_empty_run;
            self.pending_empty_run = 0;
            self.write_entry(&ReplayEntry::EmptyRun(n))?;
        }
        Ok(())
    }

    /// Record one tick's `TickInput`. Empty ticks are folded into a
    /// run-length count instead of writing an entry per tick -- most ticks
    /// in a real session carry no new input.
    pub(crate) fn record_tick(&mut self, tick_input: &TickInput) {
        self.ticks_written += 1;
        if tick_input.is_empty() {
            self.pending_empty_run += 1;
            return;
        }
        if let Err(e) = self
            .flush_pending_empty_run()
            .and_then(|()| self.write_entry(&ReplayEntry::Tick(tick_input.clone())))
        {
            log::error!("replay recorder: failed to write tick entry: {e}");
        }
    }

    /// Record a periodic state-hash checkpoint. Callers must invoke this
    /// only for the tick they just called `record_tick` for (checkpoints
    /// are read back in file order, immediately after the input entry for
    /// the same tick -- see `ReplayPlayer::verify_checkpoint`).
    pub(crate) fn record_checkpoint(&mut self, tick: u64, hash: u64) {
        if let Err(e) = self
            .flush_pending_empty_run()
            .and_then(|()| self.write_entry(&ReplayEntry::Checkpoint { tick, hash }))
        {
            log::error!("replay recorder: failed to write checkpoint entry: {e}");
        }
    }

    /// Flush any trailing empty-run + write the closing [`ReplayEntry::End`].
    /// Must run on every `logic_thread_main` exit path or the file is left
    /// without one, which playback can only report as a read error.
    /// `tainted` is the recording session's
    /// [`DeterminismTaint`](aberred_core::resources::determinism_taint::DeterminismTaint).
    pub(crate) fn finish(mut self, final_hash: u64, tainted: bool) -> io::Result<()> {
        self.flush_pending_empty_run()?;
        self.write_entry(&ReplayEntry::End {
            total_ticks: self.ticks_written,
            final_hash,
            tainted,
        })?;
        self.writer.flush()
    }
}

/// Streaming reader, standing in for live `rx_input`/`rx_logic` collection.
/// `logic_thread_main` still drains `rx_logic` for real during playback, but
/// only its non-sim-visible messages are applied (font/texture loads and
/// overlay edits keep happening); live `ScreenSize`/`SignalIntent`s are
/// discarded along with `rx_input` outright, so every sim-visible fact comes
/// from the file. This type only supplies [`collect`](Self::collect)'s
/// `TickInput` fields and answers [`verify_checkpoint`](Self::verify_checkpoint).
pub(crate) struct ReplayPlayer {
    reader: BufReader<File>,
    pending_empty_run: u32,
    finished: bool,
    /// Reused decode buffer for `next_entry` -- called up to once per sim
    /// tick during playback, so a fresh `Vec` per call would allocate at up
    /// to 240Hz; this keeps one allocation that stabilizes at the largest
    /// entry size instead.
    scratch: Vec<u8>,
}

impl ReplayPlayer {
    /// Open `path`, read and return its header, without validating it
    /// against a `GameConfig` -- see [`validate_replay_header`] for that,
    /// called separately by `EngineBuilder::try_run`.
    pub(crate) fn open_header(path: &Path) -> Result<(ReplayHeader, Self), EngineError> {
        let open_err = |message: String| EngineError::ReplayOpen {
            path: path.to_path_buf(),
            message,
        };
        let file = File::open(path).map_err(|e| open_err(e.to_string()))?;
        let mut reader = BufReader::new(file);
        let bytes = read_len_prefixed(&mut reader)
            .map_err(|e| open_err(e.to_string()))?
            .ok_or_else(|| open_err("file is empty (no header)".into()))?;
        let header: ReplayHeader =
            postcard::from_bytes(&bytes).map_err(|e| open_err(e.to_string()))?;
        if header.magic != REPLAY_MAGIC {
            return Err(open_err(
                "not an Aberred Engine replay file (bad magic)".into(),
            ));
        }
        Ok((
            header,
            Self {
                reader,
                pending_empty_run: 0,
                finished: false,
                scratch: Vec::new(),
            },
        ))
    }

    /// Read one length-prefixed frame into `self.scratch`, resizing it in
    /// place rather than allocating fresh. `Ok(false)` on a clean
    /// end-of-file.
    fn read_len_prefixed_into_scratch(&mut self) -> io::Result<bool> {
        let mut len_buf = [0u8; 4];
        match self.reader.read_exact(&mut len_buf) {
            Ok(()) => {}
            Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => return Ok(false),
            Err(e) => return Err(e),
        }
        let len = u32::from_le_bytes(len_buf) as usize;
        self.scratch.resize(len, 0);
        self.reader.read_exact(&mut self.scratch)?;
        Ok(true)
    }

    fn next_entry(&mut self) -> io::Result<Option<ReplayEntry>> {
        if !self.read_len_prefixed_into_scratch()? {
            return Ok(None);
        }
        let entry = postcard::from_bytes(&self.scratch).map_err(postcard_io_err)?;
        Ok(Some(entry))
    }

    /// Handle the closing [`ReplayEntry::End`]: log the session summary and
    /// mark playback finished. Shared by [`collect`](Self::collect) and
    /// [`verify_checkpoint`](Self::verify_checkpoint) -- either can be the
    /// one to read it, depending on whether `collect` was draining a pending
    /// empty-run that tick, and the summary must be logged exactly once
    /// regardless of which.
    fn note_end(&mut self, total_ticks: u64, final_hash: u64, tainted: bool) {
        log::info!(
            "replay: playback finished after {total_ticks} recorded ticks \
             (final hash {final_hash:#x})"
        );
        if tainted {
            log::warn!(
                "replay: this recording was made in a session flagged by \
                 DeterminismTaint -- it is not guaranteed bit-exact reproducible, \
                 so any divergence reported above may be explained by that rather \
                 than by a regression"
            );
        }
        self.finished = true;
    }

    /// Reset `out` for tick `tick` (same 0-indexed-before-
    /// `update_world_time`-increments convention `collect_tick_input_live`
    /// uses -- callers pass `world.resource::<WorldTime>().frame_count`) and
    /// fill its `samples`/`capture`/`intents`/`screen_size` from the next
    /// recorded tick (the recorded `TickInput::tick` value itself isn't
    /// trusted to overwrite `out.tick`). Once the log is exhausted, every
    /// subsequent call leaves `out` empty (frozen on the last tick) -- see
    /// [`is_finished`](Self::is_finished) for detecting the transition.
    pub(crate) fn collect(&mut self, out: &mut TickInput, tick: u64) {
        out.reset(tick);
        if self.finished {
            return;
        }
        if self.pending_empty_run > 0 {
            self.pending_empty_run -= 1;
            return;
        }
        match self.next_entry() {
            Ok(Some(ReplayEntry::EmptyRun(n))) => {
                if n == 0 {
                    log::warn!("replay: ignoring zero-length EmptyRun entry");
                } else {
                    self.pending_empty_run = n - 1;
                }
            }
            Ok(Some(ReplayEntry::Tick(ti))) => {
                out.samples = ti.samples;
                out.capture = ti.capture;
                out.intents = ti.intents;
                out.screen_size = ti.screen_size;
            }
            Ok(Some(ReplayEntry::Checkpoint { .. })) => {
                log::error!(
                    "replay: found a Checkpoint entry where a Tick/EmptyRun was expected -- \
                     file may be corrupt; ending playback"
                );
                self.finished = true;
            }
            Ok(Some(ReplayEntry::End {
                total_ticks,
                final_hash,
                tainted,
            })) => self.note_end(total_ticks, final_hash, tainted),
            Ok(None) => {
                log::warn!(
                    "replay: reached end of file with no End entry -- the recording \
                     was not finalized (truncated file); ending playback"
                );
                self.finished = true;
            }
            Err(e) => {
                log::error!("replay: failed to read next entry: {e}; ending playback");
                self.finished = true;
            }
        }
    }

    /// Read the checkpoint entry recorded for `tick` (immediately following
    /// that tick's input entry in the file -- see [`ReplayRecorder::record_checkpoint`])
    /// and compare it against `actual_hash`. Caller must invoke this on the
    /// exact same tick cadence recording used (`checkpoint_countdown` in
    /// `logic_thread_main`). Returns
    /// `Some((expected, actual))` on a mismatch, `None` if it matched (or
    /// the check itself couldn't be completed, logged either way).
    pub(crate) fn verify_checkpoint(&mut self, tick: u64, actual_hash: u64) -> Option<(u64, u64)> {
        // Playback deliberately keeps ticking past the recording's end (the
        // sim freezes on the last tick's input), and the caller's checkpoint
        // countdown keeps firing every `sim_hz` ticks forever -- without
        // this, every one of those ticks would re-hit EOF and log about it.
        if self.finished {
            return None;
        }
        match self.next_entry() {
            Ok(Some(ReplayEntry::Checkpoint {
                tick: expected_tick,
                hash: expected_hash,
            })) => {
                if expected_tick != tick {
                    log::error!(
                        "replay: checkpoint tick mismatch (file says {expected_tick}, sim is \
                         at {tick}) -- file may be corrupt or checkpoint cadence changed"
                    );
                }
                (expected_hash != actual_hash).then_some((expected_hash, actual_hash))
            }
            Ok(Some(ReplayEntry::End {
                total_ticks,
                final_hash,
                tainted,
            })) => {
                // The recording simply ended before this checkpoint tick --
                // not a corrupt file. Reachable whenever `collect` was
                // draining a pending empty-run this tick and so didn't read
                // the entry itself.
                self.note_end(total_ticks, final_hash, tainted);
                None
            }
            Ok(Some(other)) => {
                log::error!(
                    "replay: expected a Checkpoint entry, found {other:?} -- file may be corrupt"
                );
                None
            }
            Ok(None) => {
                log::warn!("replay: end of file reached while expecting a checkpoint");
                self.finished = true;
                None
            }
            Err(e) => {
                log::error!("replay: failed to read checkpoint entry: {e}");
                None
            }
        }
    }

    /// True once the recorded log is exhausted (or a read error/corrupt
    /// entry ended playback early) -- from here on [`collect`](Self::collect)
    /// always leaves `out` empty.
    pub(crate) fn is_finished(&self) -> bool {
        self.finished
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aberred_core::protocol::raw_input::RawDeviceSnapshot;
    use tempfile::NamedTempFile;

    fn test_header() -> ReplayHeader {
        ReplayHeader {
            magic: REPLAY_MAGIC,
            format_version: REPLAY_FORMAT_VERSION,
            engine_build_id: "test".into(),
            seed: 1,
            sim_hz: 240.0,
            config_digest: 0,
            scene_id: "".into(),
            game_version: "test".into(),
        }
    }

    #[test]
    fn header_roundtrips() {
        let file = NamedTempFile::new().unwrap();
        let header = test_header();
        let rec = ReplayRecorder::create(file.path(), &header).unwrap();
        rec.finish(42, false).unwrap();

        let (read_header, _player) = ReplayPlayer::open_header(file.path()).unwrap();
        assert_eq!(read_header.seed, header.seed);
        assert_eq!(read_header.sim_hz, header.sim_hz);
    }

    #[test]
    fn empty_ticks_compress_to_one_run_length_entry() {
        let file = NamedTempFile::new().unwrap();
        let header = test_header();
        let mut rec = ReplayRecorder::create(file.path(), &header).unwrap();
        for _ in 0..1000 {
            rec.record_tick(&TickInput::default());
        }
        rec.finish(0, false).unwrap();

        let (_h, mut player) = ReplayPlayer::open_header(file.path()).unwrap();
        for _ in 0..1000 {
            let mut out = TickInput::default();
            player.collect(&mut out, 0);
            assert!(out.is_empty());
        }
        assert!(!player.is_finished());
    }

    #[test]
    fn non_empty_tick_roundtrips() {
        let file = NamedTempFile::new().unwrap();
        let header = test_header();
        let mut rec = ReplayRecorder::create(file.path(), &header).unwrap();
        rec.record_tick(&TickInput::default());
        let mut ti = TickInput::default();
        ti.samples.push(RawDeviceSnapshot::default());
        rec.record_tick(&ti);
        rec.finish(0, false).unwrap();

        let (_h, mut player) = ReplayPlayer::open_header(file.path()).unwrap();
        let mut out = TickInput::default();
        player.collect(&mut out, 0);
        assert!(out.is_empty());
        let mut out = TickInput::default();
        player.collect(&mut out, 0);
        assert_eq!(out.samples.len(), 1);
    }

    #[test]
    fn checkpoint_mismatch_is_reported() {
        let file = NamedTempFile::new().unwrap();
        let header = test_header();
        let mut rec = ReplayRecorder::create(file.path(), &header).unwrap();
        rec.record_tick(&TickInput::default());
        rec.record_checkpoint(0, 123);
        rec.finish(123, false).unwrap();

        let (_h, mut player) = ReplayPlayer::open_header(file.path()).unwrap();
        let mut out = TickInput::default();
        player.collect(&mut out, 0);
        let mismatch = player.verify_checkpoint(0, 999);
        assert_eq!(mismatch, Some((123, 999)));
    }

    #[test]
    fn checkpoint_match_reports_none() {
        let file = NamedTempFile::new().unwrap();
        let header = test_header();
        let mut rec = ReplayRecorder::create(file.path(), &header).unwrap();
        rec.record_tick(&TickInput::default());
        rec.record_checkpoint(0, 123);
        rec.finish(123, false).unwrap();

        let (_h, mut player) = ReplayPlayer::open_header(file.path()).unwrap();
        let mut out = TickInput::default();
        player.collect(&mut out, 0);
        let mismatch = player.verify_checkpoint(0, 123);
        assert_eq!(mismatch, None);
    }

    #[test]
    fn finished_after_log_exhausted() {
        let file = NamedTempFile::new().unwrap();
        let header = test_header();
        let mut rec = ReplayRecorder::create(file.path(), &header).unwrap();
        rec.record_tick(&TickInput::default());
        rec.finish(0, false).unwrap();

        let (_h, mut player) = ReplayPlayer::open_header(file.path()).unwrap();
        assert!(!player.is_finished());
        let mut out = TickInput::default();
        player.collect(&mut out, 0);
        assert!(!player.is_finished());
        player.collect(&mut out, 0);
        assert!(player.is_finished());
        assert!(out.is_empty());
    }

    #[test]
    fn open_header_rejects_bad_magic() {
        let file = NamedTempFile::new().unwrap();
        let mut header = test_header();
        header.magic = *b"NOPE";
        ReplayRecorder::create(file.path(), &header)
            .unwrap()
            .finish(0, false)
            .unwrap();

        let Err(EngineError::ReplayOpen { path, .. }) =
            ReplayPlayer::open_header(file.path()).map(|(h, _)| h)
        else {
            panic!("expected a ReplayOpen error for a bad-magic file");
        };
        assert_eq!(
            path.as_path(),
            file.path(),
            "the bad-magic error must name the file it failed on -- it used to \
             report an empty path, since the check lived in validate_replay_header \
             which never sees one"
        );
    }

    /// A recording with no ticks at all must be recognized as finished on the
    /// very first `collect`. Regression net for the closing entry being a
    /// bare `ReplayTrailer { total_ticks: 0, final_hash: 0 }`, which decoded
    /// as a perfectly valid `ReplayEntry::EmptyRun(0)` and silently consumed
    /// an extra playback tick instead of ending the log.
    #[test]
    fn zero_tick_file_finishes_on_first_collect() {
        let file = NamedTempFile::new().unwrap();
        ReplayRecorder::create(file.path(), &test_header())
            .unwrap()
            .finish(0, false)
            .unwrap();

        let (_h, mut player) = ReplayPlayer::open_header(file.path()).unwrap();
        let mut out = TickInput::default();
        player.collect(&mut out, 0);
        assert!(out.is_empty());
        assert!(player.is_finished());
    }

    /// The closing entry must also be recognized when `verify_checkpoint`
    /// is the reader that reaches it -- which happens whenever `collect` was
    /// draining a pending empty-run on the same tick.
    #[test]
    fn verify_checkpoint_treats_end_entry_as_clean_finish() {
        let file = NamedTempFile::new().unwrap();
        let mut rec = ReplayRecorder::create(file.path(), &test_header()).unwrap();
        for _ in 0..3 {
            rec.record_tick(&TickInput::default());
        }
        rec.finish(0, false).unwrap();

        let (_h, mut player) = ReplayPlayer::open_header(file.path()).unwrap();
        let mut out = TickInput::default();
        player.collect(&mut out, 0);
        assert!(!player.is_finished());
        assert_eq!(player.verify_checkpoint(0, 7), None);
        assert!(player.is_finished());
        // ...and once finished, further checkpoint ticks must not keep
        // re-reading (and re-warning about) the exhausted file.
        assert_eq!(player.verify_checkpoint(1, 7), None);
    }

    #[test]
    fn validate_header_rejects_version_mismatch() {
        let mut header = test_header();
        header.format_version = REPLAY_FORMAT_VERSION + 1;
        let config = GameConfig::new();
        assert!(matches!(
            validate_replay_header(&header, &config),
            Err(EngineError::ReplayVersionMismatch { .. })
        ));
    }

    #[test]
    fn validate_header_rejects_sim_hz_mismatch() {
        let mut header = test_header();
        header.sim_hz = 60.0;
        let mut config = GameConfig::new();
        config.sim_hz = 240.0;
        assert!(matches!(
            validate_replay_header(&header, &config),
            Err(EngineError::ReplaySimHzMismatch { .. })
        ));
    }

    #[test]
    fn validate_header_rejects_config_digest_mismatch() {
        let mut config = GameConfig::new();
        config.sim_hz = 240.0;
        let mut header = test_header();
        header.sim_hz = config.sim_hz;
        header.config_digest = config_digest(&config) ^ 1;
        assert!(matches!(
            validate_replay_header(&header, &config),
            Err(EngineError::ReplayConfigMismatch { .. })
        ));
    }

    #[test]
    fn validate_header_accepts_matching_header() {
        let config = GameConfig::new();
        let mut header = test_header();
        header.sim_hz = config.sim_hz;
        header.config_digest = config_digest(&config);
        assert!(validate_replay_header(&header, &config).is_ok());
    }
}
