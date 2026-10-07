//! Progress display (D98, D105, D109), chosen with `--progress auto|plain|off`: one plain ASCII
//! line redrawn on a terminal, or appended plain lines (one per phase, and the progress at most
//! every 10 seconds and at least once a minute) for screen readers and logs, or nothing. It
//! polls the library's counters; the analysis never waits for it.
//!
//! The line is redrawn in place (`\r`, no escape sequences), only when its text changes and its
//! whole percentage changed or a second passed, so a screen reader is not flooded and nothing
//! depends on the terminal's encoding or escape support (Sampath et al. 2021, §5.3.2). Every
//! other writer of standard error calls [`clear`] first, so messages never mix with it.
//!
//! Phase lines are printed from the start times the library records, so every phase is printed
//! once, in order and with its exact start time, however briefly it lasted.

use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use dima_core::control::{Phase, Progress, ProgressSnapshot};
use dima_core::human::{self, separators};

use super::args::ProgressArg;
use super::ui::Verbosity;

/// How progress is shown.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// One line redrawn in place (terminal).
    Line,
    /// Appended lines with the elapsed time: one per phase, then the progress periodically.
    Plain,
    /// Nothing.
    Off,
}

/// The display for a run (D109): `-q` hides progress; `plain` and `off` are explicit; `auto`
/// redraws a line on a terminal that can (below `-vv`, whose debug messages would interleave
/// with it) and otherwise prints plain lines only with `-v`.
#[must_use]
pub const fn mode(choice: ProgressArg, v: Verbosity, redraw_terminal: bool) -> Mode {
    if v.quiet {
        return Mode::Off;
    }
    match choice {
        ProgressArg::Off => Mode::Off,
        ProgressArg::Plain => Mode::Plain,
        ProgressArg::Auto if redraw_terminal && v.verbose < 2 => Mode::Line,
        ProgressArg::Auto if v.verbose >= 1 => Mode::Plain,
        ProgressArg::Auto => Mode::Off,
    }
}

/// The progress display of a run. Created before the analysis and started from its
/// `threads_ready` hook ([`Display::launcher`]), so on a system short of threads its optional
/// thread never takes the place of one the run needs (D110).
pub struct Display {
    shared: Arc<Shared>,
}

/// State shared by the display, its launcher and its thread.
struct Shared {
    progress: Arc<Progress>,
    mode: Mode,
    /// Milestones printed so far.
    printed: Mutex<usize>,
    stop: AtomicBool,
    /// The display thread, once launched.
    handle: Mutex<Option<JoinHandle<()>>>,
}

impl Shared {
    /// Starts the display thread, once, unless the display is off or already stopped. A
    /// refused thread only means no progress is shown.
    fn launch(self: &Arc<Self>) {
        if self.mode == Mode::Off {
            return;
        }
        let mut handle = self.handle.lock().unwrap_or_else(PoisonError::into_inner);
        // `stop` is set before the handle is taken, so a launch racing `finish` either sees it
        // or leaves a handle that `finish` joins.
        if handle.is_some() || self.stop.load(Ordering::Relaxed) {
            return;
        }
        let shared = Arc::clone(self);
        *handle = std::thread::Builder::new()
            .name("dima-progress".into())
            .spawn(move || run(&shared.progress, shared.mode, &shared.stop, &shared.printed))
            .ok();
    }
}

impl Display {
    /// A display of `progress`; nothing is shown until it is launched.
    #[must_use]
    pub fn new(progress: Arc<Progress>, mode: Mode) -> Self {
        Self {
            shared: Arc::new(Shared {
                progress,
                mode,
                printed: Mutex::new(0),
                stop: AtomicBool::new(false),
                handle: Mutex::new(None),
            }),
        }
    }

    /// A function that starts the display, for the analysis's `threads_ready` hook.
    pub fn launcher(&self) -> impl Fn() + Send + Sync + 'static {
        let shared = Arc::clone(&self.shared);
        move || shared.launch()
    }

    /// Enters the writing phase; its milestone (and any not yet printed) is printed before this
    /// returns, so it precedes the messages about the files written.
    pub fn writing(&self) {
        let s = &self.shared;
        s.progress.set_phase(Phase::Writing);
        if s.mode == Mode::Plain {
            phase_lines(&s.progress, &s.printed);
        }
    }

    /// Stops and clears the display.
    pub fn finish(self) {
        drop(self);
    }

    fn stop_thread(&self) {
        let s = &self.shared;
        s.stop.store(true, Ordering::Relaxed);
        let handle = s
            .handle
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take();
        if let Some(h) = handle {
            // Wake it from its tick, so stopping never waits.
            h.thread().unpark();
            let _ = h.join();
        }
        clear();
    }
}

impl Drop for Display {
    fn drop(&mut self) {
        self.stop_thread();
    }
}

/// How often the counters are polled.
const TICK: Duration = Duration::from_millis(100);
/// No line for runs shorter than this (cargo's precedent), so quick runs print nothing.
const FIRST_DRAW: Duration = Duration::from_millis(500);
/// Without a change of the whole percentage, the line is redrawn at most this often.
const REDRAW: Duration = Duration::from_secs(1);
/// Phases without a percentage show their elapsed time after this long.
const SHOW_ELAPSED: Duration = Duration::from_secs(2);
/// Plain mode: a progress line needs this long since the previous line...
const PLAIN_MIN_GAP: Duration = Duration::from_secs(10);
/// ...and one is printed at least this often, so a long phase is never silent.
const PLAIN_MAX_GAP: Duration = Duration::from_secs(60);
/// Width assumed when the terminal's is unknown.
const DEFAULT_WIDTH: usize = 80;

/// What is on the terminal's current line: the length of the progress line drawn there, 0 for
/// none. Shared by the display thread and every other writer of standard error.
static SCREEN: Mutex<usize> = Mutex::new(0);

fn screen() -> MutexGuard<'static, usize> {
    SCREEN.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Erases the progress line, if one is shown, so that a message can be printed on a clean line;
/// the display redraws it at its next tick. Returns whether a line was erased.
pub fn clear() -> bool {
    erase(&mut screen())
}

fn erase(shown: &mut usize) -> bool {
    if *shown == 0 {
        return false;
    }
    let mut err = std::io::stderr().lock();
    let _ = write!(err, "\r{:width$}\r", "", width = *shown);
    let _ = err.flush();
    *shown = 0;
    true
}

/// Draws `text` over the current line.
fn draw(shown: &mut usize, text: &str) {
    let mut err = std::io::stderr().lock();
    let pad = shown.saturating_sub(text.len());
    let _ = write!(err, "\r{text}{:pad$}", "");
    let _ = err.flush();
    *shown = text.len();
}

/// Prints the line of every phase started and not yet printed, in order. It stops at the first
/// phase that has not started: the library enters "done" before the front end enters
/// "writing", and the writing line must still be printed when it starts.
fn phase_lines(progress: &Progress, printed: &Mutex<usize>) {
    let mut next = printed.lock().unwrap_or_else(PoisonError::into_inner);
    for phase in Phase::ALL.iter().skip(*next) {
        let Some(t) = progress.phase_started(*phase) else {
            break;
        };
        if *phase != Phase::Done {
            let _ = writeln!(
                std::io::stderr(),
                "[{}] {}",
                human::duration(t),
                phase.label()
            );
        }
        *next += 1;
    }
}

/// The content of the progress line: what is happening, how far it is (whole percent, when
/// known) and further facts, most important first.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Status {
    /// The phase, e.g. `counting k-mers`.
    pub label: &'static str,
    /// Whole percent done, when the total is known.
    pub percent: Option<u64>,
    /// Further facts, dropped from the end when the line is too long.
    pub items: Vec<String>,
}

/// The status of a run from a snapshot of its counters and the time spent in the phase.
#[must_use]
pub fn status(s: &ProgressSnapshot, in_phase: Duration) -> Status {
    let label = s.phase.label();
    let mut items = Vec::new();
    let percent = match s.phase {
        Phase::Counting => {
            if s.records > 0 {
                items.push(human::plural(s.records, "sequence", "sequences"));
            }
            match s.bytes_total {
                Some(total) if total > 0 => {
                    items.push(format!(
                        "{} of {}",
                        human::bytes(s.bytes_read),
                        human::bytes(total)
                    ));
                    // Reading ends a little before counting does: 100% waits for the next phase.
                    Some(percent(s.bytes_read, total).min(99))
                }
                _ => {
                    items.push(human::bytes(s.bytes_read));
                    None
                }
            }
        }
        Phase::Finalising if s.positions_total > 0 => {
            items.push(format!(
                "{} of {} positions",
                s.positions_done, s.positions_total
            ));
            Some(percent(s.positions_done, s.positions_total))
        }
        Phase::Opening | Phase::Finalising | Phase::Writing | Phase::Done => None,
    };
    if percent.is_none() && items.is_empty() && in_phase >= SHOW_ELAPSED {
        // Whole seconds: the line changes at most once a second.
        let secs = in_phase.as_secs();
        items.push(if secs < 60 {
            format!("{secs} s")
        } else {
            human::duration(Duration::from_secs(secs))
        });
    }
    Status {
        label,
        percent,
        items,
    }
}

/// Whole percent of `part` in `total` (rounded down, at most 100).
fn percent(part: u64, total: u64) -> u64 {
    let p = u128::from(part.min(total)) * 100 / u128::from(total.max(1));
    u64::try_from(p).unwrap_or(100)
}

/// The line for `status`, at most `width - 1` characters (the last column is left free, so the
/// terminal never wraps): items are dropped from the end, then the text is cut.
#[must_use]
pub fn render(status: &Status, width: usize) -> String {
    let max = width.saturating_sub(1).max(1);
    let head = match status.percent {
        Some(p) => format!("{}: {p}%", status.label),
        None => format!("{}...", status.label),
    };
    let mut items = status.items.as_slice();
    loop {
        let line = if items.is_empty() {
            head.clone()
        } else if status.percent.is_some() {
            format!("{head}{}{}", separators::ITEM, items.join(separators::ITEM))
        } else {
            format!("{head} {}", items.join(separators::ITEM))
        };
        if line.chars().count() <= max || items.is_empty() {
            let cut: String = line.chars().take(max).collect();
            return cut.trim_end().to_owned();
        }
        items = &items[..items.len() - 1];
    }
}

/// The terminal's width in columns.
fn terminal_width() -> usize {
    terminal_size::terminal_size_of(std::io::stderr())
        .map_or(DEFAULT_WIDTH, |(w, _)| usize::from(w.0))
}

/// What was last drawn, for the redraw rule.
struct Drawn {
    text: String,
    percent: Option<u64>,
    at: Instant,
}

/// Plain mode: whether to append a progress line, `since_last` after the previous line and with
/// the phase's whole percentage now and at that line: when it crossed a multiple of 10 at least
/// 10 seconds later, or after a minute in any case.
fn plain_due(since_last: Duration, percent: Option<u64>, last_percent: Option<u64>) -> bool {
    let decile = |p: Option<u64>| p.map(|p| p / 10);
    since_last >= PLAIN_MAX_GAP
        || (since_last >= PLAIN_MIN_GAP && decile(percent) > decile(last_percent))
}

/// Whether the line should be redrawn now: always after a message erased it; otherwise only
/// when its text changed and its whole percentage changed or a second passed.
fn redraw_due(last: Option<&Drawn>, text: &str, percent: Option<u64>, erased: bool) -> bool {
    match last {
        None => true,
        Some(_) if erased => true,
        Some(d) => d.text != text && (d.percent != percent || d.at.elapsed() >= REDRAW),
    }
}

fn run(progress: &Progress, mode: Mode, stop: &AtomicBool, printed: &Mutex<usize>) {
    let started = Instant::now();
    let mut phase = progress.snapshot().phase;
    let mut phase_start = Instant::now();
    let mut last: Option<Drawn> = None;
    // Plain mode: when the last line was printed, and the phase's percentage then.
    let mut last_line = Instant::now();
    let mut last_percent: Option<u64> = None;
    while !stop.load(Ordering::Relaxed) {
        let s = progress.snapshot();
        let new_phase = s.phase != phase;
        if new_phase {
            phase = s.phase;
            phase_start = Instant::now();
        }
        match mode {
            Mode::Plain => {
                phase_lines(progress, printed);
                let st = status(&s, Duration::ZERO);
                if new_phase {
                    // The phase line was just printed.
                    last_line = Instant::now();
                    last_percent = st.percent;
                } else if phase != Phase::Done
                    && plain_due(last_line.elapsed(), st.percent, last_percent)
                {
                    let _ = writeln!(
                        std::io::stderr(),
                        "[{}] {}",
                        human::duration(progress.elapsed()),
                        render(&st, usize::MAX)
                    );
                    last_line = Instant::now();
                    last_percent = st.percent;
                }
            }
            Mode::Line if started.elapsed() >= FIRST_DRAW => {
                if phase == Phase::Done {
                    // Between the analysis and the writing of the outputs: nothing to show.
                    erase(&mut screen());
                    last = None;
                } else {
                    let st = status(&s, phase_start.elapsed());
                    let text = render(&st, terminal_width());
                    let mut shown = screen();
                    // A message printed since the last draw erased the line.
                    let erased = *shown == 0;
                    if redraw_due(last.as_ref(), &text, st.percent, erased) {
                        draw(&mut shown, &text);
                        last = Some(Drawn {
                            text,
                            percent: st.percent,
                            at: Instant::now(),
                        });
                    }
                }
            }
            Mode::Line | Mode::Off => {}
        }
        std::thread::park_timeout(TICK);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot(phase: Phase) -> ProgressSnapshot {
        ProgressSnapshot {
            phase,
            bytes_read: 5_600_000,
            bytes_total: Some(9_100_000),
            records: 1652,
            positions_done: 2086,
            positions_total: 3364,
        }
    }

    #[test]
    fn lines() {
        let line = |s: &ProgressSnapshot, t: u64| render(&status(s, Duration::from_secs(t)), 200);
        assert_eq!(
            line(&snapshot(Phase::Counting), 0),
            "counting k-mers: 61%, 1652 sequences, 5.6 MB of 9.1 MB"
        );
        assert_eq!(
            line(&snapshot(Phase::Finalising), 0),
            "computing statistics: 62%, 2086 of 3364 positions"
        );
        assert_eq!(line(&snapshot(Phase::Writing), 1), "writing...");
        assert_eq!(line(&snapshot(Phase::Writing), 3), "writing... 3 s");
        assert_eq!(line(&snapshot(Phase::Writing), 65), "writing... 1 min 05 s");
        assert_eq!(line(&snapshot(Phase::Opening), 0), "opening...");
        // Standard input: no total, no percentage; no sequences counted yet.
        let stdin = ProgressSnapshot {
            bytes_total: None,
            records: 0,
            ..snapshot(Phase::Counting)
        };
        assert_eq!(line(&stdin, 0), "counting k-mers... 5.6 MB");
        // Everything read, still counting: 99% until the next phase.
        let read = ProgressSnapshot {
            bytes_read: 9_100_000,
            ..snapshot(Phase::Counting)
        };
        assert!(line(&read, 0).starts_with("counting k-mers: 99%,"));
        // Nothing but ASCII, whatever the phase.
        for p in Phase::ALL {
            assert!(line(&snapshot(p), 5).is_ascii());
        }
    }

    #[test]
    fn narrow_terminals() {
        let st = status(&snapshot(Phase::Counting), Duration::ZERO);
        assert_eq!(render(&st, 40), "counting k-mers: 61%, 1652 sequences");
        assert_eq!(render(&st, 25), "counting k-mers: 61%");
        assert_eq!(render(&st, 10), "counting");
        for w in 0..120 {
            assert!(render(&st, w).chars().count() < w.max(2), "width {w}");
        }
    }

    #[test]
    fn redraw_rule() {
        let d = Drawn {
            text: "a: 5%".into(),
            percent: Some(5),
            at: Instant::now(),
        };
        assert!(redraw_due(None, "a: 5%", Some(5), true));
        assert!(!redraw_due(Some(&d), "a: 5%", Some(5), false), "unchanged");
        assert!(
            !redraw_due(Some(&d), "a: 5%, 2 MB", Some(5), false),
            "same percentage, too soon"
        );
        assert!(redraw_due(Some(&d), "a: 6%", Some(6), false));
        assert!(
            redraw_due(Some(&d), "a: 5%", Some(5), true),
            "after a message"
        );
        let old = Drawn {
            at: Instant::now() - REDRAW,
            ..d
        };
        assert!(redraw_due(Some(&old), "a: 5%, 2 MB", Some(5), false));
    }

    #[test]
    fn plain_rule() {
        let s = Duration::from_secs;
        // A new tenth, but too soon after the previous line.
        assert!(!plain_due(s(5), Some(20), Some(10)));
        assert!(plain_due(s(10), Some(20), Some(10)));
        // The same tenth: only after a minute.
        assert!(!plain_due(s(30), Some(19), Some(10)));
        assert!(plain_due(s(60), Some(19), Some(10)));
        // No percentage (standard input, writing): once a minute.
        assert!(!plain_due(s(59), None, None));
        assert!(plain_due(s(60), None, None));
    }

    #[test]
    fn modes() {
        let v = |quiet, verbose| Verbosity { quiet, verbose };
        use ProgressArg::{Auto, Off, Plain};
        assert_eq!(mode(Auto, v(false, 0), true), Mode::Line);
        assert_eq!(mode(Auto, v(false, 1), true), Mode::Line);
        assert_eq!(mode(Auto, v(false, 2), true), Mode::Plain);
        assert_eq!(mode(Auto, v(false, 0), false), Mode::Off);
        assert_eq!(mode(Auto, v(false, 1), false), Mode::Plain);
        assert_eq!(mode(Plain, v(false, 0), true), Mode::Plain);
        assert_eq!(mode(Plain, v(false, 0), false), Mode::Plain);
        assert_eq!(mode(Off, v(false, 2), true), Mode::Off);
        for choice in [Auto, Plain, Off] {
            for terminal in [false, true] {
                assert_eq!(mode(choice, v(true, 0), terminal), Mode::Off, "-q");
            }
        }
    }

    #[test]
    fn percentages() {
        assert_eq!(percent(0, 0), 0);
        assert_eq!(percent(1, 3), 33);
        assert_eq!(percent(3, 3), 100);
        assert_eq!(percent(5, 3), 100);
        assert_eq!(percent(u64::MAX, u64::MAX), 100);
    }
}
