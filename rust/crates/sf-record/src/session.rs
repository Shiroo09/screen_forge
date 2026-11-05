//! Recording session state: pause/resume, restart, and the progress numbers the
//! status overlay shows.
//!
//! The capture loop owns the frames but not the buttons: the panel and the
//! status overlay flip this state from other threads while the loop is sleeping,
//! so every field is atomic and shared by `Arc` — no lock, no channel, no
//! message pump. `Session` is also a plain unit-testable object; nothing here
//! touches Windows.
//!
//! Pause semantics match `GifRecorder.Pause/Resume` (`Gif/GifRecorder.cs:292,304`):
//! the paused wall time must not reach the output, so the loop sleeps it off the
//! clock instead of lengthening the current frame's delay.

use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU64, AtomicU8, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

const RECORDING: u8 = 0;
const PAUSED: u8 = 1;

/// How long the loop may sleep while paused before re-checking the state. Caps
/// how late a "Devam et" click is noticed.
pub const PAUSE_SLICE: Duration = Duration::from_millis(20);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RecordPhase {
    Recording,
    Paused,
}

/// Phase, restart generation and the live counters. Shared by the capture loop,
/// the panel and the status overlay.
pub struct Session {
    phase: AtomicU8,
    restarts: AtomicU64,
    elapsed_ms: AtomicU64,
    frames: AtomicU64,
    bytes: AtomicU64,
    target_fps: AtomicI32,
    recorded_fps: AtomicU64, // f64 bits
}

impl Session {
    pub const fn new() -> Self {
        Self {
            phase: AtomicU8::new(RECORDING),
            restarts: AtomicU64::new(0),
            elapsed_ms: AtomicU64::new(0),
            frames: AtomicU64::new(0),
            bytes: AtomicU64::new(0),
            target_fps: AtomicI32::new(10),
            recorded_fps: AtomicU64::new(0),
        }
    }

    pub fn phase(&self) -> RecordPhase {
        if self.phase.load(Ordering::SeqCst) == PAUSED {
            RecordPhase::Paused
        } else {
            RecordPhase::Recording
        }
    }

    pub fn is_paused(&self) -> bool {
        self.phase() == RecordPhase::Paused
    }

    pub fn pause(&self) {
        self.phase.store(PAUSED, Ordering::SeqCst);
    }

    pub fn resume(&self) {
        self.phase.store(RECORDING, Ordering::SeqCst);
    }

    /// Bumps the generation the capture loop compares against to notice a
    /// "Yeniden başlat" and drop the frames collected so far.
    pub fn restart(&self) {
        self.restarts.fetch_add(1, Ordering::SeqCst);
    }

    pub fn restart_generation(&self) -> u64 {
        self.restarts.load(Ordering::SeqCst)
    }

    /// Sleeps up to `requested`, in slices, while paused. Returns the wall time
    /// actually spent paused so the caller can subtract it from its own clock.
    pub fn wait_while_paused(&self, requested: Duration) -> Duration {
        if !self.is_paused() || requested.is_zero() {
            return Duration::ZERO;
        }
        let start = Instant::now();
        while self.is_paused() && start.elapsed() < requested {
            std::thread::sleep((requested - start.elapsed()).min(PAUSE_SLICE));
        }
        start.elapsed()
    }

    // ─── Progress ─────────────────────────────────────────────────────────

    /// Milliseconds recorded so far, pause excluded.
    pub fn elapsed_ms(&self) -> u64 {
        self.elapsed_ms.load(Ordering::Relaxed)
    }

    pub fn frames(&self) -> u64 {
        self.frames.load(Ordering::Relaxed)
    }

    /// Bytes the stored frames occupy right now (compressed for GIF).
    pub fn bytes(&self) -> u64 {
        self.bytes.load(Ordering::Relaxed)
    }

    pub fn target_fps(&self) -> i32 {
        self.target_fps.load(Ordering::Relaxed)
    }

    /// Frames actually written per second, measured over the whole recording.
    pub fn recorded_fps(&self) -> f64 {
        f64::from_bits(self.recorded_fps.load(Ordering::Relaxed))
    }

    pub fn set_elapsed_ms(&self, ms: u64) {
        self.elapsed_ms.store(ms, Ordering::Relaxed);
    }

    pub fn set_target_fps(&self, fps: i32) {
        self.target_fps.store(fps, Ordering::Relaxed);
    }

    /// Records one stored frame and the bytes it added.
    pub fn add_frame(&self, stored_bytes: u64) {
        self.frames.fetch_add(1, Ordering::Relaxed);
        self.bytes.fetch_add(stored_bytes, Ordering::Relaxed);
        let ms = self.elapsed_ms();
        if ms > 0 {
            let fps = self.frames() as f64 * 1000.0 / ms as f64;
            self.recorded_fps.store(fps.to_bits(), Ordering::Relaxed);
        }
    }

    /// Restart: everything the panel would otherwise have to reset by hand.
    pub fn reset_progress(&self) {
        self.elapsed_ms.store(0, Ordering::Relaxed);
        self.frames.store(0, Ordering::Relaxed);
        self.bytes.store(0, Ordering::Relaxed);
        self.recorded_fps.store(0, Ordering::Relaxed);
    }
}

impl Default for Session {
    fn default() -> Self {
        Self::new()
    }
}

/// What a recording UI holds: the shared [`Session`] plus every stop path the
/// user has — the panel's İptal button and the status overlay's Durdur button.
/// Both set the same flag, so neither needs to know about the other.
#[derive(Clone, Default)]
pub struct PauseHandle {
    session: Arc<Session>,
    cancel: Arc<AtomicBool>,
}

impl PauseHandle {
    pub fn new() -> Self {
        Self {
            session: Arc::new(Session::new()),
            cancel: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn session(&self) -> &Arc<Session> {
        &self.session
    }

    pub fn pause(&self) {
        self.session.pause();
    }

    pub fn resume(&self) {
        self.session.resume();
    }

    /// Drops the frames collected so far; the capture loop notices next pass.
    pub fn restart(&self) {
        self.session.restart();
    }

    pub fn is_paused(&self) -> bool {
        self.session.is_paused()
    }

    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::SeqCst);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancel.load(Ordering::SeqCst)
    }

    /// Clears the stop flag and any pause before a new recording starts.
    pub fn reset(&self) {
        self.cancel.store(false, Ordering::SeqCst);
        self.session.resume();
        self.session.reset_progress();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_recording_and_toggles() {
        let s = Session::new();
        assert_eq!(s.phase(), RecordPhase::Recording);
        s.pause();
        assert!(s.is_paused());
        s.resume();
        assert!(!s.is_paused());
    }

    #[test]
    fn a_pause_returns_the_time_it_swallowed() {
        let s = Session::new();
        s.pause();
        let paused = s.wait_while_paused(Duration::from_millis(60));
        // The whole request was spent paused, so the caller drops all of it.
        assert!(paused >= Duration::from_millis(45), "paused {paused:?}");
        assert!(paused <= Duration::from_millis(150), "paused {paused:?}");
    }

    #[test]
    fn resuming_early_returns_only_the_paused_part() {
        let s = Session::new();
        s.pause();
        // Resume after ~25 ms; 200 ms was requested, so the sleep is cut short.
        std::thread::scope(|scope| {
            scope.spawn(|| {
                std::thread::sleep(Duration::from_millis(25));
                s.resume();
            });
            let paused = s.wait_while_paused(Duration::from_millis(200));
            assert!(paused < Duration::from_millis(120), "paused {paused:?}");
        });
    }

    #[test]
    fn not_paused_returns_zero() {
        let s = Session::new();
        assert_eq!(s.wait_while_paused(Duration::from_millis(50)), Duration::ZERO);
    }

    #[test]
    fn a_paused_sleep_never_overruns_the_request() {
        let s = Session::new();
        s.pause();
        let start = Instant::now();
        let paused = s.wait_while_paused(Duration::from_millis(30));
        assert!(paused <= Duration::from_millis(60), "slept {paused:?}");
        assert!(start.elapsed() <= Duration::from_millis(60));
    }

    #[test]
    fn restart_bumps_the_generation() {
        let s = Session::new();
        let before = s.restart_generation();
        s.restart();
        assert_eq!(s.restart_generation(), before + 1);
    }

    #[test]
    fn progress_tracks_frames_and_bytes() {
        let s = Session::new();
        s.set_target_fps(15);
        s.set_elapsed_ms(1000);
        s.add_frame(400);
        s.add_frame(600);

        assert_eq!(s.target_fps(), 15);
        assert_eq!(s.frames(), 2);
        assert_eq!(s.bytes(), 1000);
        assert!((s.recorded_fps() - 2.0).abs() < 0.001, "fps {}", s.recorded_fps());

        // Restart drops the lot, so the overlay does not show the old run.
        s.reset_progress();
        assert_eq!(s.frames(), 0);
        assert_eq!(s.bytes(), 0);
        assert_eq!(s.elapsed_ms(), 0);
        assert_eq!(s.recorded_fps(), 0.0);
    }

    #[test]
    fn handle_shares_pause_and_stop_across_clones() {
        let handle = PauseHandle::new();
        let from_ui = handle.clone();
        assert!(!handle.is_cancelled());

        // The overlay holds one clone, the panel another; either can drive it.
        from_ui.pause();
        assert!(handle.is_paused());
        from_ui.restart();
        assert_eq!(handle.session().restart_generation(), 1);
        from_ui.cancel();
        assert!(handle.is_cancelled());

        // A fresh recording clears the previous stop and pause.
        handle.reset();
        assert!(!handle.is_cancelled());
        assert!(!handle.is_paused());
    }
}
