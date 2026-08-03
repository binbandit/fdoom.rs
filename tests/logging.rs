//! Tests for the engine's leveled diagnostics (`fdoom::core::log`).
//!
//! The threshold and the capture buffer are process-wide, so every test that
//! observes or mutates them runs inside `log::capture`, which serializes blocks
//! against each other and restores the previous threshold on the way out.

use std::sync::atomic::{AtomicUsize, Ordering};

use fdoom::core::log::{self, Level};

#[test]
fn levels_are_ordered_most_severe_first() {
    // The threshold check is a `<=`, so this ordering is load-bearing.
    assert!(Level::Error < Level::Warn);
    assert!(Level::Warn < Level::Info);
    assert!(Level::Info < Level::Debug);
}

#[test]
fn capture_collects_every_level_with_its_tag() {
    let ((), lines) = log::capture(Level::Debug, || {
        fdoom::log_error!("boom {}", 1);
        fdoom::log_warn!("careful {}", 2);
        fdoom::log_info!("fyi {}", 3);
        fdoom::log_debug!("detail {}", 4);
    });

    assert_eq!(lines.len(), 4, "all four levels pass a Debug threshold");
    assert_eq!(lines[0], "ERROR boom 1");
    assert_eq!(lines[1], "WARN  careful 2");
    assert_eq!(lines[2], "INFO  fyi 3");
    assert_eq!(lines[3], "DEBUG detail 4");
}

#[test]
fn threshold_suppresses_less_severe_levels() {
    let ((), lines) = log::capture(Level::Warn, || {
        fdoom::log_error!("kept");
        fdoom::log_warn!("kept");
        fdoom::log_info!("dropped");
        fdoom::log_debug!("dropped");
    });

    assert_eq!(lines.len(), 2, "Info and Debug are below a Warn threshold");
    assert!(lines.iter().all(|l| l.ends_with("kept")), "{lines:?}");
}

#[test]
fn suppressed_call_sites_do_not_evaluate_their_arguments() {
    // The whole point of gating inside the macro: a `log_debug!` in a hot tick must
    // cost one atomic load, not a formatted string. If this regresses, per-tick
    // diagnostics start costing real frame time in normal play.
    static EVALUATED: AtomicUsize = AtomicUsize::new(0);

    fn expensive() -> usize {
        EVALUATED.fetch_add(1, Ordering::Relaxed)
    }

    let ((), lines) = log::capture(Level::Error, || {
        fdoom::log_debug!("never rendered: {}", expensive());
        fdoom::log_info!("never rendered: {}", expensive());
    });

    assert!(lines.is_empty(), "nothing at/below Error was logged");
    assert_eq!(
        EVALUATED.load(Ordering::Relaxed),
        0,
        "suppressed call sites must not evaluate format arguments"
    );
}

#[test]
fn set_debug_picks_the_debug_or_warn_threshold() {
    // Mutating the global threshold inside a capture block keeps this serialized
    // against the other tests; capture restores the pre-block value regardless.
    let ((), _) = log::capture(Level::Warn, || {
        log::set_debug(true);
        assert_eq!(log::level(), Level::Debug, "--debug shows everything");

        log::set_debug(false);
        assert_eq!(log::level(), Level::Warn, "a normal session stays quiet");
    });
}

#[test]
fn enabled_matches_the_active_threshold() {
    let ((), _) = log::capture(Level::Info, || {
        assert!(log::enabled(Level::Error));
        assert!(log::enabled(Level::Warn));
        assert!(log::enabled(Level::Info));
        assert!(!log::enabled(Level::Debug));
    });
}

#[test]
fn capture_cleans_up_even_when_the_closure_panics() {
    // Tests in this lane assert that a *former* panic is gone; if one of them ever
    // panics again it must not wedge every later test. Deliberately asserted through
    // a *following* capture rather than by reading the global threshold directly —
    // that read would race with any other test currently inside a capture block.
    let hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {})); // this panic is expected; keep output clean
    let result = std::panic::catch_unwind(|| {
        log::capture(Level::Debug, || panic!("deliberate"));
    });
    std::panic::set_hook(hook);

    assert!(result.is_err(), "capture propagates the closure's panic");

    // Reaching this at all proves the capture lock was released. The assertions prove
    // the rest of the state came back: the threshold is the block's own (not the
    // panicking block's Debug), and no line leaked across from it.
    let ((), lines) = log::capture(Level::Warn, || {
        fdoom::log_warn!("after");
        fdoom::log_debug!("must stay filtered");
    });
    assert_eq!(lines, vec!["WARN  after".to_string()]);
}
