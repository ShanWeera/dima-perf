//! Severity-based error handling for the GUI.
//!
//! ERROR/WARNING persist until manually dismissed.
//! SUCCESS/INFO auto-dismiss with content-aware duration (2s + 50ms/char, cap 10s).
//!
//! Ref: "Designing Better Error Messages UX" (Smashing Magazine, 2022),
//! Carbon Design System notification pattern.

use std::time::{Duration, Instant};

/// Severity levels for error messages.
#[derive(Debug, Clone, Copy, PartialEq)]
#[allow(dead_code)]
pub enum ErrorSeverity {
    /// Fatal errors that block the user. Persist until dismissed.
    Error,
    /// Non-fatal issues. Persist until dismissed.
    Warning,
    /// Operation completed successfully. Auto-dismiss.
    Success,
    /// Informational messages. Auto-dismiss.
    Info,
}

/// A single error/notification message.
#[derive(Debug, Clone)]
pub struct ErrorMessage {
    pub text: String,
    pub severity: ErrorSeverity,
    pub created_at: Instant,
    pub dismissed: bool,
}

impl ErrorMessage {
    pub fn error(text: String) -> Self {
        Self {
            text,
            severity: ErrorSeverity::Error,
            created_at: Instant::now(),
            dismissed: false,
        }
    }

    pub fn warning(text: String) -> Self {
        Self {
            text,
            severity: ErrorSeverity::Warning,
            created_at: Instant::now(),
            dismissed: false,
        }
    }

    pub fn success(text: String) -> Self {
        Self {
            text,
            severity: ErrorSeverity::Success,
            created_at: Instant::now(),
            dismissed: false,
        }
    }

    #[allow(dead_code)]
    pub fn info(text: String) -> Self {
        Self {
            text,
            severity: ErrorSeverity::Info,
            created_at: Instant::now(),
            dismissed: false,
        }
    }

    /// Whether this message should auto-dismiss (SUCCESS and INFO only).
    pub fn should_auto_dismiss(&self) -> bool {
        matches!(self.severity, ErrorSeverity::Success | ErrorSeverity::Info)
    }

    /// Content-aware auto-dismiss duration: 2s + 50ms per character, capped at 10s.
    /// Ref: 72Technologies toast heuristic.
    pub fn auto_dismiss_duration(&self) -> Duration {
        let base = Duration::from_secs(2);
        let per_char = Duration::from_millis(50) * self.text.len() as u32;
        let total = base + per_char;
        total.min(Duration::from_secs(10))
    }

    /// Whether this message has expired (auto-dismiss only).
    pub fn is_expired(&self) -> bool {
        if !self.should_auto_dismiss() {
            return false;
        }
        self.created_at.elapsed() >= self.auto_dismiss_duration()
    }
}

/// Collection of active error messages.
#[derive(Debug, Default)]
pub struct ErrorState {
    pub messages: Vec<ErrorMessage>,
}

impl ErrorState {
    pub fn push(&mut self, message: ErrorMessage) {
        self.messages.push(message);
    }

    /// Remove expired auto-dismiss messages and dismissed messages.
    pub fn tick_auto_dismiss(&mut self) {
        self.messages.retain(|m| !m.dismissed && !m.is_expired());
    }

    /// Dismiss a specific message by index.
    pub fn dismiss(&mut self, index: usize) {
        if let Some(msg) = self.messages.get_mut(index) {
            msg.dismissed = true;
        }
    }

    /// Whether there are any visible messages.
    pub fn has_messages(&self) -> bool {
        self.messages
            .iter()
            .any(|m| !m.dismissed && !m.is_expired())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_persists_until_dismissed() {
        let msg = ErrorMessage::error("test".to_string());
        assert!(!msg.should_auto_dismiss());
        assert!(!msg.is_expired());
    }

    #[test]
    fn test_success_auto_dismisses() {
        let msg = ErrorMessage::success("ok".to_string());
        assert!(msg.should_auto_dismiss());
        // Just created, not expired yet
        assert!(!msg.is_expired());
    }

    #[test]
    fn test_auto_dismiss_duration_scales_with_length() {
        let short = ErrorMessage::info("Hi".to_string());
        let long =
            ErrorMessage::info("This is a much longer message with more content".to_string());
        assert!(long.auto_dismiss_duration() > short.auto_dismiss_duration());
    }

    #[test]
    fn test_auto_dismiss_duration_capped_at_10s() {
        let very_long = ErrorMessage::info("x".repeat(1000));
        assert_eq!(very_long.auto_dismiss_duration(), Duration::from_secs(10));
    }

    #[test]
    fn test_error_state_dismiss_by_index() {
        let mut state = ErrorState::default();
        state.push(ErrorMessage::error("first".to_string()));
        state.push(ErrorMessage::error("second".to_string()));
        state.dismiss(0);
        state.tick_auto_dismiss();
        assert_eq!(state.messages.len(), 1);
        assert_eq!(state.messages[0].text, "second");
    }
}
