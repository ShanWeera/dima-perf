//! Notifications. Errors and warnings stay until dismissed; successes and information close
//! by themselves after a time that grows with the length of the text in characters (G56). A
//! repeated message is counted on the existing toast instead of stacking a copy, and at most
//! [`MAX_KEPT`] persistent toasts are kept (the oldest go first), so a burst of failures cannot
//! grow without bound (G56). The app schedules a repaint for the next expiry, so toasts close
//! even when the window is idle (G16).

use std::time::{Duration, Instant};

/// Toasts shown at once; older ones are summarised as "+N more".
pub const MAX_VISIBLE: usize = 4;
/// Toasts kept in total.
pub const MAX_KEPT: usize = 30;

/// Severity.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Severity {
    /// Something failed.
    Error,
    /// Something needs attention.
    Warning,
    /// Something finished.
    Success,
    /// For information.
    Info,
}

impl Severity {
    /// Label shown with the message (severity is never conveyed by colour alone).
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Error => "Error",
            Self::Warning => "Warning",
            Self::Success => "Done",
            Self::Info => "Note",
        }
    }
}

/// One notification.
#[derive(Clone, Debug, PartialEq)]
pub struct Toast {
    /// Stable id (for dismissing).
    pub id: u64,
    /// Severity.
    pub severity: Severity,
    /// The message.
    pub text: String,
    /// Text copied by "Copy details" (error reports, internal errors).
    pub details: Option<String>,
    /// How many times this message was raised.
    pub count: u32,
    /// When it was (last) raised.
    pub created: Instant,
}

impl Toast {
    /// `true` for severities that close by themselves.
    #[must_use]
    pub const fn auto_dismiss(&self) -> bool {
        matches!(self.severity, Severity::Success | Severity::Info)
    }

    /// How long an auto-dismissing toast stays: 4 s plus 50 ms per character, at most 12 s.
    #[must_use]
    pub fn lifetime(&self) -> Duration {
        let chars = u32::try_from(self.text.chars().count()).unwrap_or(u32::MAX);
        (Duration::from_secs(4) + Duration::from_millis(50) * chars).min(Duration::from_secs(12))
    }

    /// When an auto-dismissing toast expires.
    #[must_use]
    pub fn expires_at(&self) -> Option<Instant> {
        self.auto_dismiss().then(|| self.created + self.lifetime())
    }
}

/// The notification list.
#[derive(Clone, Debug, Default)]
pub struct Toasts {
    items: Vec<Toast>,
    next_id: u64,
}

impl Toasts {
    /// Raises a notification at `now`.
    pub fn push(
        &mut self,
        severity: Severity,
        text: impl Into<String>,
        details: Option<String>,
        now: Instant,
    ) {
        let text = text.into();
        if let Some(t) = self
            .items
            .iter_mut()
            .find(|t| t.severity == severity && t.text == text)
        {
            t.count = t.count.saturating_add(1);
            t.created = now;
            t.details = details.or(t.details.take());
            return;
        }
        self.next_id += 1;
        self.items.push(Toast {
            id: self.next_id,
            severity,
            text,
            details,
            count: 1,
            created: now,
        });
        while self.items.len() > MAX_KEPT {
            self.items.remove(0);
        }
    }

    /// Removes expired toasts; returns the next expiry, if any, for scheduling a repaint.
    pub fn tick(&mut self, now: Instant) -> Option<Instant> {
        self.items
            .retain(|t| t.expires_at().is_none_or(|at| at > now));
        self.items.iter().filter_map(Toast::expires_at).min()
    }

    /// Dismisses one toast.
    pub fn dismiss(&mut self, id: u64) {
        self.items.retain(|t| t.id != id);
    }

    /// The toasts, oldest first.
    #[must_use]
    pub fn items(&self) -> &[Toast] {
        &self.items
    }

    /// `true` when there is nothing to show.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lifetimes_count_characters_and_are_capped() {
        let now = Instant::now();
        let mut t = Toasts::default();
        t.push(Severity::Success, "αβγ", None, now);
        let short = t.items()[0].lifetime();
        assert_eq!(
            short,
            Duration::from_millis(4150),
            "3 characters, not 6 bytes"
        );
        t.push(Severity::Info, "x".repeat(1000), None, now);
        assert_eq!(t.items()[1].lifetime(), Duration::from_secs(12));
    }

    #[test]
    fn errors_persist_and_successes_expire() {
        let now = Instant::now();
        let mut t = Toasts::default();
        t.push(Severity::Error, "failed", Some("details".into()), now);
        t.push(Severity::Success, "saved", None, now);
        let next = t.tick(now).unwrap();
        assert!(next > now);
        assert_eq!(t.tick(now + Duration::from_secs(60)), None);
        assert_eq!(t.items().len(), 1);
        assert_eq!(t.items()[0].severity, Severity::Error);
        let id = t.items()[0].id;
        t.dismiss(id);
        assert!(t.is_empty());
    }

    #[test]
    fn repeats_are_grouped_and_the_list_is_bounded() {
        let now = Instant::now();
        let mut t = Toasts::default();
        for _ in 0..5 {
            t.push(Severity::Error, "same", None, now);
        }
        assert_eq!(t.items().len(), 1);
        assert_eq!(t.items()[0].count, 5);
        for i in 0..100 {
            t.push(Severity::Error, format!("e{i}"), None, now);
        }
        assert_eq!(t.items().len(), MAX_KEPT);
        assert_eq!(t.items().last().unwrap().text, "e99");
    }
}
