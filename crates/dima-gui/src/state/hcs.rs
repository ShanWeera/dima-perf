//! The conserved-region (HCS) parameters and results: one state for the setup option and the
//! workspace strip (G40). The threshold defaults to the library's 100% and accepts (50, 100]
//! (D44, G41), parsed and checked by the library (`Percent`, exact comparison). Edits are
//! debounced and computed by a worker (P5); a result is used only for the session and
//! parameters it was computed for.

use std::sync::Arc;
use std::time::{Duration, Instant};

use dima_core::config::Percent;
use dima_core::hcs::HcsParams;
use dima_core::model::{HcsRegion, HcsResults};

use super::settings::Settings;

/// Quiet time after the last edit before recomputing.
pub const DEBOUNCE: Duration = Duration::from_millis(250);

/// HCS parameters and results.
#[derive(Clone, Debug, PartialEq)]
pub struct HcsState {
    /// The threshold as typed.
    pub text: String,
    /// The last valid parameters.
    pub params: HcsParams,
    /// Why `text` is not valid, if it is not.
    pub error: Option<String>,
    /// When an edit is waiting to be computed.
    pub pending_since: Option<Instant>,
    /// The computation in flight: generation, session id and parameters.
    pub running: Option<(u64, u64, HcsParams)>,
    /// The latest result, with the session id and parameters it belongs to.
    pub result: Option<(u64, HcsParams, Arc<HcsResults>)>,
    /// Why the latest computation failed.
    pub failure: Option<String>,
}

impl HcsState {
    /// From the remembered settings (an invalid stored threshold falls back to 100%).
    #[must_use]
    pub fn from_settings(s: &Settings) -> Self {
        let threshold = Percent::hcs_threshold(&s.hcs_threshold)
            .unwrap_or(dima_core::config::DEFAULT_HCS_THRESHOLD);
        Self {
            text: threshold.to_string(),
            params: HcsParams {
                threshold,
                eligible_only: s.hcs_eligible_only,
            },
            error: None,
            pending_since: None,
            running: None,
            result: None,
            failure: None,
        }
    }

    /// Copies the parameters into the settings.
    pub fn store(&self, s: &mut Settings) {
        s.hcs_threshold = self.params.threshold.to_string();
        s.hcs_eligible_only = self.params.eligible_only;
    }

    /// The user edited the threshold text or the eligible-only switch. An invalid threshold
    /// cancels a pending recomputation: nothing changes until the text is valid again.
    pub fn edited(&mut self, now: Instant) {
        match Percent::hcs_threshold(&self.text) {
            Ok(t) => {
                self.error = None;
                self.params.threshold = t;
                self.pending_since = Some(now);
            }
            Err(e) => {
                self.error = Some(e.report());
                self.pending_since = None;
            }
        }
    }

    /// `true` when a recomputation should start now.
    #[must_use]
    pub fn due(&self, now: Instant, pointer_down: bool) -> bool {
        self.pending_since
            .is_some_and(|t| !pointer_down && now.duration_since(t) >= DEBOUNCE)
    }

    /// The result for a session at the current parameters.
    #[must_use]
    pub fn current(&self, session: u64) -> Option<&Arc<HcsResults>> {
        match &self.result {
            Some((id, p, r)) if *id == session && *p == self.params => Some(r),
            _ => None,
        }
    }
}

/// The region covering alignment column `x` (regions are in column order).
#[must_use]
pub fn region_at(regions: &[HcsRegion], x: f64) -> Option<&HcsRegion> {
    let k = regions.partition_point(|r| {
        #[allow(clippy::cast_precision_loss)]
        let end = r.end_column as f64 + 0.5;
        end < x
    });
    let r = regions.get(k)?;
    #[allow(clippy::cast_precision_loss)]
    let start = r.start_column as f64 - 0.5;
    (x >= start).then_some(r)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_and_validation() {
        let mut h = HcsState::from_settings(&Settings::default());
        assert_eq!(h.text, "100");
        assert!(!h.params.eligible_only);
        let now = Instant::now();
        h.text = "50".into();
        h.edited(now);
        assert!(h.error.as_deref().unwrap().contains("(50, 100]"));
        assert_eq!(
            h.params.threshold.to_string(),
            "100",
            "the last valid value stays"
        );
        assert_eq!(h.pending_since, None);
        // A valid edit followed by an invalid one leaves nothing pending.
        h.text = "99".into();
        h.edited(now);
        assert!(h.pending_since.is_some());
        h.text = "abc".into();
        h.edited(now);
        assert_eq!(h.pending_since, None);
        assert_eq!(h.params.threshold.to_string(), "99");
        h.text = "95.5".into();
        h.edited(now);
        assert_eq!(h.error, None);
        assert_eq!(h.params.threshold.to_string(), "95.5");
        assert!(!h.due(now, false));
        assert!(!h.due(now + DEBOUNCE, true), "not while dragging");
        assert!(h.due(now + DEBOUNCE, false));
        let mut s = Settings::default();
        h.store(&mut s);
        assert_eq!(s.hcs_threshold, "95.5");
        let bad = Settings {
            hcs_threshold: "12".into(),
            ..Settings::default()
        };
        assert_eq!(HcsState::from_settings(&bad).text, "100");
    }

    #[test]
    fn results_belong_to_a_session_and_parameters() {
        let mut h = HcsState::from_settings(&Settings::default());
        let r =
            Arc::new(dima_core::hcs::compute(&crate::tests::fixtures::mers(), &h.params).unwrap());
        h.result = Some((7, h.params, Arc::clone(&r)));
        assert!(h.current(7).is_some());
        assert!(h.current(8).is_none());
        h.params.eligible_only = true;
        assert!(h.current(7).is_none(), "stale after a parameter change");
        // Lookup by column.
        let first = &r.regions[0];
        #[allow(clippy::cast_precision_loss)]
        let x = first.start_column as f64;
        assert_eq!(region_at(&r.regions, x).map(|g| g.id), Some(first.id));
        assert_eq!(region_at(&r.regions, -3.0).map(|g| g.id), None);
    }
}
