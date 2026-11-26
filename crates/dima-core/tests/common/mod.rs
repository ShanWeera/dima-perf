//! Helpers shared by the integration tests that compare the library with the independent
//! Python reference implementation (`reference.rs`, live) and with values it computed
//! (`paper.rs`), so both apply one documented accuracy rule (docs, "Numerical accuracy").

/// Largest relative difference accepted between a number and the reference value. The test
/// cases differ by at most 2.6e-15 for the entropies and 6e-15 for similarity-weighted entropy
/// (8.2e-16 in the paper test), a last-bit difference in a platform's `log` moves the reference
/// by at most 6e-16, and every legitimate change measured (summation order, a one-pass
/// regression, the deferred last-digit fix) moved the values by at most 8e-15; a realistic
/// numerical regression of 1.2e-13 is still caught.
const RELATIVE_TOLERANCE: f64 = 1e-13;

/// Asserts that `got` agrees with the reference value `want` within [`RELATIVE_TOLERANCE`],
/// relative to `want` at every magnitude, so a zero must be exactly zero.
#[track_caller]
pub fn assert_agrees(what: &str, got: f64, want: f64) {
    assert!(
        (got - want).abs() <= RELATIVE_TOLERANCE * want.abs(),
        "{what}: {got} vs reference {want}"
    );
}
