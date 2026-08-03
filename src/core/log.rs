//! Leveled diagnostics for the engine.
//!
//! Deliberately tiny: one routing point ([`emit`]), a process-wide threshold, and
//! four macros. Diagnostics are for the *maintainer* — anything the player needs to
//! see belongs in the notification tiers on `Game` (`push_ambient`, `push_warning`,
//! `push_toast`, `push_cue`), not here.
//!
//! Everything goes to stderr, so a scripted/headless run can separate diagnostics
//! from a tool's real stdout output.
//!
//! Verbosity follows the `--debug` flag: a normal session shows errors and warnings
//! only, `--debug` shows everything. Level checks happen *before* the format
//! arguments are evaluated, so a suppressed `log_debug!` in a hot tick costs one
//! relaxed atomic load.

use std::fmt;
use std::io::Write;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::{Mutex, MutexGuard, PoisonError};

/// Severity, ordered most severe first so the threshold is a simple `<=`.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
#[repr(u8)]
pub enum Level {
    /// The game lost something it cannot recover: a failed save write, a subsystem
    /// that will not start. The player is likely about to notice.
    Error = 0,
    /// The game recovered by substituting a default or skipping data — damaged saves,
    /// unknown entity names, out-of-range geometry. Each one is a lead for a bug.
    Warn = 1,
    /// One-off lifecycle milestones: world init, level loads, save writes.
    Info = 2,
    /// Per-tick or per-entity detail, only useful when reproducing something.
    Debug = 3,
}

impl Level {
    /// Fixed-width tag so multi-line runs stay aligned in a terminal.
    fn tag(self) -> &'static str {
        match self {
            Level::Error => "ERROR",
            Level::Warn => "WARN ",
            Level::Info => "INFO ",
            Level::Debug => "DEBUG",
        }
    }
}

/// Current threshold. Warn by default: a normal play session stays quiet, but a
/// player who runs from a terminal still sees the things worth reporting.
static THRESHOLD: AtomicU8 = AtomicU8::new(Level::Warn as u8);

/// Set the verbosity threshold; messages at this level or more severe are emitted.
pub fn set_level(level: Level) {
    THRESHOLD.store(level as u8, Ordering::Relaxed);
}

/// Wire verbosity to the `--debug` flag (see `crate::run`).
pub fn set_debug(debug: bool) {
    set_level(if debug { Level::Debug } else { Level::Warn });
}

/// The active threshold.
pub fn level() -> Level {
    match THRESHOLD.load(Ordering::Relaxed) {
        0 => Level::Error,
        1 => Level::Warn,
        2 => Level::Info,
        _ => Level::Debug,
    }
}

/// Whether `level` would currently be emitted. The macros call this first so
/// suppressed call sites never evaluate their format arguments.
#[inline]
pub fn enabled(level: Level) -> bool {
    (level as u8) <= THRESHOLD.load(Ordering::Relaxed)
}

/// Set while a [`capture`] block is active, so the common path stays lock-free.
static CAPTURING: AtomicBool = AtomicBool::new(false);
/// Lines collected by the active [`capture`] block.
static CAPTURED: Mutex<Vec<String>> = Mutex::new(Vec::new());
/// Serializes [`capture`] blocks against each other — the threshold and the capture
/// buffer are process-wide, so two concurrent tests would otherwise interleave.
static CAPTURE_LOCK: Mutex<()> = Mutex::new(());

/// A poisoned diagnostics mutex is not worth aborting a game over; a previous panic
/// while logging leaves the buffer usable.
fn unpoison<'a, T>(
    r: Result<MutexGuard<'a, T>, PoisonError<MutexGuard<'a, T>>>,
) -> MutexGuard<'a, T> {
    r.unwrap_or_else(PoisonError::into_inner)
}

/// The single routing point. Prefer the macros; this is public only so they can
/// expand to it from any module.
pub fn emit(level: Level, args: fmt::Arguments<'_>) {
    if !enabled(level) {
        return;
    }
    if CAPTURING.load(Ordering::Relaxed) {
        unpoison(CAPTURED.lock()).push(format!("{} {}", level.tag(), args));
        return;
    }
    // A failed diagnostic write must never take the game down, so the result is
    // dropped: there is nowhere left to report it to.
    let mut err = std::io::stderr().lock();
    let _ = writeln!(err, "[{}] {}", level.tag(), args);
}

/// Restores the capture globals when a [`capture`] block ends — including when the
/// closure panics, which a test asserting that a *former* panic is gone may well do.
/// RAII rather than `catch_unwind`, so the restore cannot be skipped.
struct Restore {
    previous: Level,
}

impl Drop for Restore {
    fn drop(&mut self) {
        CAPTURING.store(false, Ordering::Relaxed);
        set_level(self.previous);
    }
}

/// Run `f` with diagnostics captured at `level`, returning its value and the lines
/// emitted. For tests: asserting on a warning is how we prove a recovery path ran.
///
/// Blocks are serialized against each other and the previous threshold is restored on
/// the way out, panic or not. Not reentrant — `f` must not call `capture` again.
pub fn capture<R>(level: Level, f: impl FnOnce() -> R) -> (R, Vec<String>) {
    let _lock = unpoison(CAPTURE_LOCK.lock());
    let previous = self::level();
    unpoison(CAPTURED.lock()).clear();
    set_level(level);
    CAPTURING.store(true, Ordering::Relaxed);
    let _restore = Restore { previous };

    let out = f();

    let lines = std::mem::take(&mut *unpoison(CAPTURED.lock()));
    (out, lines)
}

/// Something was lost and the game could not recover it.
#[macro_export]
macro_rules! log_error {
    ($($arg:tt)*) => {
        if $crate::core::log::enabled($crate::core::log::Level::Error) {
            $crate::core::log::emit($crate::core::log::Level::Error, format_args!($($arg)*));
        }
    };
}

/// The game recovered — say what was wrong *and* what was substituted.
#[macro_export]
macro_rules! log_warn {
    ($($arg:tt)*) => {
        if $crate::core::log::enabled($crate::core::log::Level::Warn) {
            $crate::core::log::emit($crate::core::log::Level::Warn, format_args!($($arg)*));
        }
    };
}

/// A lifecycle milestone worth seeing once.
#[macro_export]
macro_rules! log_info {
    ($($arg:tt)*) => {
        if $crate::core::log::enabled($crate::core::log::Level::Info) {
            $crate::core::log::emit($crate::core::log::Level::Info, format_args!($($arg)*));
        }
    };
}

/// Reproduction detail; suppressed unless `--debug`.
#[macro_export]
macro_rules! log_debug {
    ($($arg:tt)*) => {
        if $crate::core::log::enabled($crate::core::log::Level::Debug) {
            $crate::core::log::emit($crate::core::log::Level::Debug, format_args!($($arg)*));
        }
    };
}
