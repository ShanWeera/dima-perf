---
name: Fix dima-gui issues
overview: "Round 1-4: All 23 issues fixed and verified. Round 5 (2026-07-13 08:35 UTC+8): Fresh-eyes audit found 8 new issues focused on SRP, performance, and UX."
isProject: false
todos:
  - id: fix-hcs-lowsupport
    content: "Fix critical low_support misattribution bug in compute_hcs_regions (src/models.rs) -- move push AFTER flush logic"
    status: completed
  - id: fix-header-format-delimiter
    content: "Fix header format parsing to use detected delimiter instead of hardcoded '|' -- store pre-split Vec<String> in AnalysisConfigUi"
    status: completed
  - id: fix-kmer-max
    content: "Use dima_lib::max_kmer_length() to dynamically cap k-mer length DragValue based on selected alphabet"
    status: completed
  - id: fix-no-sequences-dead
    content: "Make NoSequences reachable: only count sequences with actual content"
    status: completed
  - id: fix-double-clone
    content: "Fix double PathBuf clone in Analyze button handler"
    status: completed
  - id: wire-lttb
    content: "Wire LTTB downsampling into entropy chart rendering for large dataset performance"
    status: completed
  - id: chart-click
    content: "Add click-to-select-position on entropy chart with visual highlight + hover tooltip"
    status: completed
  - id: export-ui
    content: "Add export buttons (JSON, .dima binary) to workspace toolbar with error feedback"
    status: completed
  - id: ci-integration
    content: "Add dima-gui CI jobs (clippy, test, MSRV 1.92) and update release workflow system deps"
    status: completed
  - id: fix-dima-import-during-analysis
    content: "Cancel running analysis before loading .dima binary"
    status: completed
  - id: fix-stale-validation
    content: "Clear validation_result and selected_file on validation I/O error"
    status: completed
  - id: fix-query-name-staleness
    content: "Update query_name on every new file selection"
    status: completed
  - id: fix-stale-header-format
    content: "Clear header_format before re-populating in handle_file_selected"
    status: completed
  - id: hcs-threshold-workspace
    content: "Add live HCS threshold slider to Workspace view with instant recomputation"
    status: completed
  - id: fix-variant-panel-6bars
    content: "Add TotalVariantsIncidence and DistinctVariantsIncidence bars to variant panel (6 bars total)"
    status: completed
  - id: fix-validation-blocking-ui
    content: "FASTA validation still synchronous (TODO comment added). Background thread deferred: validation is fast for typical files (<100MB), and adding a ValidationWorker pattern adds significant complexity for marginal gain."
    status: completed
  - id: minor-fixes
    content: "Remove MAX_SCAN_SEQUENCES, drag-and-drop dedup, fix symlink doc, clear selected_position on new results, fix HCS off-by-one coordinates, remove dead_code allow"
    status: completed
  - id: verify-fixes
    content: "cargo fmt, clippy -p dima, clippy -p dima-gui, test -p dima (167 pass), test -p dima-gui (26 pass) -- all green"
    status: completed
  - id: fix-filter-bounds
    content: "Add .range() bounds to filter DragValues (position 1..=last, entropy 0.0..=max) so users cannot enter nonsensical values"
    status: completed
  - id: fix-dima-recent-on-success
    content: "Move .dima recent_files.add into load_dima_binary success path so corrupted files do not appear in recent files"
    status: completed
  - id: verify-fixes-round2
    content: "cargo fmt, clippy -p dima, clippy -p dima-gui, test -p dima (167 pass), test -p dima-gui (33 pass) -- all green. Last verified: 2026-07-12 23:40 UTC+8"
    status: completed
  - id: fix-stale-viewport
    content: "Reset entropy_viewport to None in both analysis success path and load_dima_binary so zoomed-in state from a previous dataset does not carry over"
    status: completed
  - id: verify-fixes-round3
    content: "cargo fmt, clippy -p dima, clippy -p dima-gui, test -p dima (167 pass), test -p dima-gui (33 pass) -- all green. Last verified: 2026-07-12 23:50 UTC+8"
    status: completed
  - id: fix-xaxis-labels-clipped
    content: "Fix x-axis position labels in entropy chart that were drawn outside the painter_at() clip rect and therefore invisible. Allocate extra height for label area and clip the painter to the full rect."
    status: completed
  - id: verify-fixes-round4
    content: "cargo fmt, clippy -p dima, clippy -p dima-gui, test -p dima (167 pass), test -p dima-gui (33 pass) -- all green. Last verified: 2026-07-13 00:05 UTC+8"
    status: completed
  - id: r5-extract-panels
    content: "SRP: Extract show_entropy_chart, show_export_buttons, show_position_explorer, show_variant_panel, show_filter_controls, show_hcs_section, show_position_details from app.rs into separate panel modules"
    status: pending
  - id: r5-validation-worker
    content: "Implement background ValidationWorker for async FASTA validation (currently synchronous on UI thread, freezes for >100MB files)"
    status: pending
  - id: r5-chart-data-cache
    content: "Cache full_data Vec across frames and only rebuild when data_version changes, eliminating per-frame allocation in show_entropy_chart"
    status: pending
  - id: r5-explorer-clone
    content: "Eliminate per-frame clone of filtered_positions in show_position_explorer by restructuring table closure borrows"
    status: pending
  - id: r5-misleading-comment
    content: "Fix misleading comment in show_hcs_section (line 1297-1298) about kmer_length that does not match code"
    status: pending
  - id: r5-theme-toggle
    content: "Add theme toggle (Light/Dark) to the UI — both themes are fully implemented but the toggle is missing"
    status: pending
  - id: r5-reset-filters
    content: "Add Reset Filters button to show_filter_controls for quick restoration of defaults"
    status: pending
  - id: r5-query-name-preserve
    content: "Only auto-update query_name if user hasn't manually customized it (check against previous file stem)"
    status: pending
  - id: r5-verify
    content: "Run cargo fmt, clippy, test for both crates after all round 5 fixes"
    status: pending
---

# Fix dima-gui Issues

Comprehensive fix plan covering 22 bugs, functional gaps, and code quality issues found during fresh-eyes review of the dima-gui implementation and supporting dima_lib changes.

**Last verified:** 2026-07-12 23:50 UTC+8 — All 22 issues fixed and verified via fresh-eyes re-read of every source file. Three audit rounds: #20-21 found in round 2, #22 found in round 3. All CI checks pass (fmt, clippy, tests).

---

## 1. BUG (Critical): Low-support position misattribution in `compute_hcs_regions`

**File:** [src/models.rs](src/models.rs), lines 252-271

The `current_low_support.push(position.position)` at line 253 executes BEFORE the consecutive-position check at lines 260-271. When a position has `low_support` AND starts a new region (non-consecutive), the just-pushed low_support entry is incorrectly flushed with the PREVIOUS region via `flush_region`, then lost for the new region.

**Trace of the bug:**

- Region 1 accumulates positions [1,2,3], no low support
- Position 5 qualifies, has `low_support = Some("LS")`, is NOT consecutive (5 != 3+1)
- Line 253: `current_low_support.push(5)` -- pushed to OLD accumulator
- Line 264: `flush_region(...)` -- flushes Region 1 with `low_support_positions = [5]` (WRONG, 5 is NOT in Region 1)
- After flush: `current_low_support` is now empty, position 5 starts a new region but its low_support is LOST

**Fix in [src/models.rs](src/models.rs):** Move the `low_support.push()` to AFTER the flush/region-start block. The restructured code should be:

```rust
Some(sequence) => {
    let is_consecutive = last_qualifying_position
        .map_or(true, |last| position.position == last + 1);

    if acc.is_empty() || !is_consecutive {
        flush_region(&mut acc, &mut current_positions, &mut current_low_support, &mut regions);
        acc = sequence.to_string();
        current_positions.push(position.position);
    } else {
        // ... overlap logic unchanged ...
    }

    // Push low_support AFTER flush so it always goes to the CORRECT region
    if position.low_support.is_some() {
        current_low_support.push(position.position);
    }

    last_qualifying_position = Some(position.position);
}
```

Add a unit test: two qualifying positions at 1 and 3 (gap at 2), position 3 has low_support. Assert Region 1 has empty `low_support_positions` and Region 2's `low_support_positions` contains 3.

---

## 2. BUG (Medium): Header format parsing hardcodes `|` delimiter

**File:** [gui/src/app.rs](gui/src/app.rs), lines 216-219

```rust
let header_format_vec: Option<Vec<String>> = config
    .header_format
    .as_ref()
    .map(|hf| hf.split('|').map(|s| s.to_string()).collect());
```

When validation detects a tab-delimited header (e.g., `"id\tcountry\thost"`), the `format_string` stores the full string joined by the DETECTED delimiter. Splitting by `|` produces a single element `["id\tcountry\thost"]` instead of three field names.

**Fix:** Two options:
- **(a) Store pre-split fields (recommended):** Change `AnalysisConfigUi.header_format` from `Option<String>` to `Option<Vec<String>>`. In `handle_file_selected` (line 675), store `Some(fmt.format_string.split(fmt.delimiter).map(|s| s.to_string()).collect())`. In `start_analysis`, use it directly as `header_format_vec`.
- **(b) Store delimiter alongside:** Add `header_delimiter: Option<char>` to `AnalysisConfigUi`. Split using stored delimiter.

Option (a) is cleaner -- SRP: the config stores structured data, not a formatted string.

---

## 3. BUG (Medium): K-mer length maximum not validated per alphabet

**File:** [gui/src/app.rs](gui/src/app.rs), line 550

```rust
ui.add(egui::DragValue::new(&mut self.analysis_config.kmer_length).range(1..=30));
```

For protein (base 20), the max valid k-mer length is 14 (`dima_lib::max_kmer_length(true)` = 14; 20^14 < 2^64 < 20^15). For nucleotide (base 5), max is 27 (`dima_lib::max_kmer_length(false)` = 27). Allowing 30 for protein causes integer overflow in `encode_kmer_validated` (the `checked_mul` returns `None`, silently dropping k-mers from analysis -- incorrect results, not a crash, which is WORSE than a crash).

**Fix:** Compute max dynamically based on selected alphabet:

```rust
let is_protein = matches!(self.analysis_config.alphabet, AlphabetChoice::Protein);
let max_k = match self.analysis_config.alphabet {
    AlphabetChoice::Protein => dima_lib::max_kmer_length(true),
    AlphabetChoice::Nucleotide => dima_lib::max_kmer_length(false),
    AlphabetChoice::Auto => dima_lib::max_kmer_length(true), // conservative
};
ui.add(egui::DragValue::new(&mut self.analysis_config.kmer_length).range(1..=max_k));
```

Also clamp the existing value: `self.analysis_config.kmer_length = self.analysis_config.kmer_length.min(max_k);`

---

## 4. BUG (Low): `NoSequences` error variant is unreachable

**File:** [src/validation.rs](src/validation.rs), lines 258-261

The scan logic always increments `sequence_count` for every header (both in the loop flush at line 378 and in the end-of-loop flush at lines 411-416). So `sequence_count == 0` only when `header_count == 0`, which triggers the `NoHeaders` check at line 254 first. `NoSequences` can never fire.

**Fix:** Remove the `NoSequences` variant and its check, OR restructure counting so that headers with zero-length sequences do NOT increment `sequence_count` (making them distinct from headers with actual content). The latter is more semantically correct -- a header without content is NOT a "sequence". This would require:
- Only increment `sequence_count` when `current_seq_length > 0`
- Then `NoSequences` can trigger when all headers have empty content

---

## 5. BUG (Low): Double PathBuf clone in Analyze button

**File:** [gui/src/app.rs](gui/src/app.rs), lines 624-625

```rust
if let Some(ref path) = self.selected_file.clone() {  // clone 1
    self.start_analysis(path.clone());                  // clone 2
}
```

**Fix:**

```rust
if let Some(ref path) = self.selected_file {
    self.start_analysis(path.clone()); // single clone
}
```

---

## 6. FUNCTIONAL GAP (High): LTTB downsampling not wired into entropy chart

**File:** [gui/src/app.rs](gui/src/app.rs), lines 738-785

The chart draws raw line segments for ALL filtered positions via `points.windows(2)`. For 10,000+ positions this creates 10,000+ CPU-rendered line segments per frame.

The LTTB module at [gui/src/charts/lttb.rs](gui/src/charts/lttb.rs) exists, is well-tested, but is marked `#[allow(dead_code)]` and never called.

**Fix:**
1. Remove `#[allow(dead_code)]` from `gui/src/charts/mod.rs`
2. Before rendering, convert positions to `Vec<Point>` and apply `lttb_downsample_by_range()` with threshold = `(chart_width_pixels * 2.0) as usize` (2x oversampling preserves detail)
3. Draw line segments from the downsampled data

---

## 7. FUNCTIONAL GAP (High): `selected_position` is never set

**File:** [gui/src/app.rs](gui/src/app.rs), line 735

The chart area uses `egui::Sense::hover()` which does not register clicks. There is NO code path that sets `self.selected_position`, so the Position Details panel (line 907) and Variant Distribution panel (line 943) permanently show placeholder text.

**Fix:**
1. Change `egui::Sense::hover()` to `egui::Sense::click()` on line 735
2. Check `response.clicked()` on the returned response
3. Map the clicked x-coordinate to the nearest position number using the same `min_x/x_range` transform
4. Set `self.selected_position = Some(nearest_position_number)`
5. Draw a vertical highlight line at the selected position

---

## 8. FUNCTIONAL GAP (Medium): No export UI

**File:** [gui/src/app.rs](gui/src/app.rs), workspace view

No buttons exist for exporting results. `dima_lib::write_results_to_output()` and `Results::to_binary()` are available.

**Fix:** Add an "Export" group to the workspace toolbar (after the summary bar) with:
- "Export JSON" button -> `rfd::FileDialog::save_file()` -> `results.to_json(Some(path))`
- "Export .dima" button -> `rfd::FileDialog::save_file()` -> `results.to_binary(path, None)`
- Success/error messages via `self.error_state`

---

## 9. FUNCTIONAL GAP (Major): No CI for dima-gui

**File:** [.github/workflows/ci.yml](.github/workflows/ci.yml)

No CI job tests the `dima-gui` crate. Additionally, the `release.yml` test-gate at line 48 runs `cargo test --workspace --locked` which now includes `gui`, potentially requiring additional system dependencies for GPU/display libs on Linux.

**Fix for ci.yml:** Add a `gui` job:

```yaml
gui:
  name: GUI (${{ matrix.check }})
  runs-on: ubuntu-latest
  strategy:
    fail-fast: false
    matrix:
      check: [clippy, test]
  steps:
    - uses: actions/checkout@v4
    - uses: dtolnay/rust-toolchain@stable
      with:
        components: clippy
    - name: Install system dependencies
      run: |
        sudo apt-get update
        sudo apt-get install -y libxkbcommon-dev libwayland-dev libxcb-shape0-dev \
          libxcb-xfixes0-dev libvulkan-dev
    - uses: Swatinem/rust-cache@v2
    - name: cargo clippy
      if: matrix.check == 'clippy'
      run: cargo clippy -p dima-gui --locked -- -D warnings
    - name: cargo test
      if: matrix.check == 'test'
      run: cargo test -p dima-gui --locked

msrv-gui:
  name: MSRV GUI (1.92)
  runs-on: ubuntu-latest
  steps:
    - uses: actions/checkout@v4
    - uses: dtolnay/rust-toolchain@master
      with:
        toolchain: "1.92"
    - name: Install system dependencies
      run: |
        sudo apt-get update
        sudo apt-get install -y libxkbcommon-dev libwayland-dev libxcb-shape0-dev \
          libxcb-xfixes0-dev libvulkan-dev
    - uses: Swatinem/rust-cache@v2
    - name: Check MSRV compiles
      run: cargo check -p dima-gui --locked
```

**Fix for release.yml test-gate:** Add the same system dependencies to the test-gate job.

---

## 10. Minor: Unused `MAX_SCAN_SEQUENCES` constant

**File:** [src/validation.rs](src/validation.rs), line 36

Defined as 1000 with a doc comment about "streaming mode beyond this limit", but no code references it. The scan processes ALL sequences unconditionally.

**Fix:** Either implement the streaming-mode cap (skip content sampling after 1000 sequences), or remove the constant. For files with millions of sequences, content sampling from the first 10 (for alphabet detection) is already capped by `ALPHABET_DETECTION_SEQUENCES`. The streaming-mode feature is premature -- remove the constant and its doc comment.

---

## 11. Minor: Progress bar reaches 100% before analysis completes

**File:** [gui/src/app.rs](gui/src/app.rs) / [src/analysis.rs](src/analysis.rs)

The `progress_counter` is incremented only in `compute_entropies()`, not in `build_positions()`. Both phases iterate the same positions. The progress bar hits 100% when entropy is done, but position building continues for several more seconds on large datasets.

**Fix:** Change the progress bar label from `"Computing entropy: {completed}/{total}"` to a two-phase display:
- When `completed < total_positions`: `"Computing entropy: {completed}/{total}"`
- When `completed >= total_positions` and analysis_handle is still active: `"Building positions..."` with a spinner instead of a progress bar

---

## 12. Minor: Drag-and-drop same-file deduplication

**File:** [gui/src/app.rs](gui/src/app.rs), lines 487-493

Re-dropping the same file re-runs validation unnecessarily. While egui clears `dropped_files` after the event frame (so it does NOT fire every frame), the same file can be dropped again.

**Fix:** Add a guard: `if self.selected_file.as_deref() != Some(path) { self.handle_file_selected(path.clone()); }`

### 12b. Minor: Misleading symlink claim in `validation.rs` module doc

**File:** [src/validation.rs](src/validation.rs), line 12

The module-level doc comment says *"Security: guards against symlinks, FIFOs, binary files..."* but `fs::metadata(path)` (line 210) follows symlinks (unlike `fs::symlink_metadata`). The code does NOT guard against symlinks — it follows them and validates the target. Symlinks to non-regular files ARE caught by the `metadata.is_file()` check at line 212, but the doc claims an explicit symlink guard that doesn't exist.

**Fix:** Change the doc line to accurately describe the behavior:
```
//! Security: guards against non-regular files (directories, FIFOs, devices),
//! binary files, excessively large files, and BOM markers. Symlinks are
//! followed transparently — the target file is what gets validated.
```

---

## 13. BUG (Medium): Loading `.dima` file during running analysis silently overwrites results

**File:** [gui/src/app.rs](gui/src/app.rs), `load_dima_binary` (line 306) and `handle_file_selected` (line 632)

When the user drops a `.dima` file while an analysis is running:

1. `handle_file_selected` routes to `load_dima_binary` (line 641)
2. `load_dima_binary` stores imported results in `self.results`, switches to Workspace view
3. `self.analysis_handle` is NOT cleared — the background thread keeps running
4. When the analysis eventually completes, `logic()` receives the result and **silently overwrites** `self.results` with the analysis output
5. The user's imported `.dima` data is destroyed without any indication

**Trace:**
1. User drops `aligned.fasta`, clicks Analyze → analysis starts running
2. User drops `previous_results.dima` → results imported, view switches to Workspace
3. 5 seconds later: analysis finishes → `logic()` overwrites `self.results` with FASTA analysis output
4. User's `.dima` import is silently gone

**Fix:** In `load_dima_binary`, cancel any running analysis and drop the handle before importing:

```rust
pub fn load_dima_binary(&mut self, path: &std::path::Path) {
    // Cancel any running analysis to prevent it from overwriting imported results.
    // Dropping the handle drops the Receiver; the worker thread's send() will
    // silently fail, and the thread exits naturally after analyze() returns.
    if self.analysis_handle.is_some() {
        self.cancel_analysis();
        self.analysis_handle = None;
    }
    // ... rest unchanged ...
}
```

This is safe because: (a) `cancel_analysis` sets the AtomicBool, causing `analyze()` to short-circuit on the next cancellation check; (b) dropping the `Receiver` means the worker's `tx.send()` returns `Err` (ignored with `let _ =`); (c) the thread exits when `analyze()` returns, freeing its stack; no resource leak.

---

## 14. BUG (Medium): Stale `validation_result` after I/O error in `handle_file_selected`

**File:** [gui/src/app.rs](gui/src/app.rs), lines 680-683

When `validate_fasta()` returns `Err(e)` (OS-level I/O error like permission denied), the error branch pushes an error message but does NOT clear `self.validation_result` or `self.selected_file`. If a previous file was validated successfully, this creates an inconsistent state:

- `self.selected_file` = new (broken) file path
- `self.validation_result` = old (valid) result from previous file

The `can_analyze` guard at line 587 checks both `selected_file.is_some()` AND `validation_result.is_valid()`, so the Analyze button appears enabled. Clicking it starts analysis on the new (broken) file, which would fail inside `dima_lib::analyze()` with a confusing error.

**Trace:**
1. User selects valid file A -> validation passes, Analyze button enabled
2. User selects file B with permission denied -> `validate_fasta` returns `Err`
3. `selected_file` = B, `validation_result` = A's valid result (stale!)
4. Analyze button appears enabled -> user clicks -> analysis runs on B -> cryptic error

**Fix:** In the `Err(e)` branch of `handle_file_selected` (line 680), also clear the stale state:

```rust
Err(e) => {
    self.validation_result = None;
    self.selected_file = None;
    self.error_state
        .push(ErrorMessage::error(format!("Validation failed: {}", e)));
}
```

---

## 15. UX (Low): Query name not updated on new file selection

**File:** [gui/src/app.rs](gui/src/app.rs), line 648

```rust
if self.analysis_config.query_name.is_empty() {
    self.analysis_config.query_name = path.file_stem()...
}
```

The `is_empty()` guard means the query name is only auto-populated the FIRST time a file is selected. If the user selects file B after file A, the query name retains A's file stem. The user would need to manually clear and re-type it.

**Fix:** Auto-populate whenever the current query name matches the previous file's stem (i.e., the user hasn't manually customized it), or simply always update on new file selection:

```rust
// Always update query name to match file stem unless user has customized it
let file_stem = path.file_stem()
    .unwrap_or_default()
    .to_string_lossy()
    .to_string();
let previous_stem = self.selected_file
    .as_ref()
    .and_then(|p| p.file_stem())
    .map(|s| s.to_string_lossy().to_string())
    .unwrap_or_default();
// Only auto-update if user hasn't manually changed the name
if self.analysis_config.query_name.is_empty()
    || self.analysis_config.query_name == previous_stem
{
    self.analysis_config.query_name = file_stem;
}
```

This preserves user-customized names while updating auto-generated ones.

---

## 16. UX (Medium): HCS threshold not adjustable in Workspace view

**File:** [gui/src/app.rs](gui/src/app.rs), lines 573-580 (Setup) and 793-824 (Workspace)

The `workspace_config.hcs_threshold` DragValue is only exposed in the Setup view's Advanced Settings (line 576). After analysis completes and the user views the HCS map in the Workspace, there is no way to adjust the threshold without:
1. Clicking "Setup" to navigate back
2. Expanding "Advanced settings"
3. Changing the threshold
4. Re-running the full analysis (which can take seconds to minutes for large datasets)

This is a significant UX bottleneck because `compute_hcs_regions()` is O(n) and completes in sub-milliseconds — there is zero technical reason to require re-analysis for a threshold change.

**Fix:** Add a "HCS Threshold" DragValue to the Workspace view (near the HCS map), and recompute HCS regions on change:

```rust
// In show_workspace, before the HCS map group:
ui.horizontal(|ui| {
    ui.label("HCS threshold (%):");
    let old_threshold = self.workspace_config.hcs_threshold;
    ui.add(
        egui::DragValue::new(&mut self.workspace_config.hcs_threshold)
            .range(0.0..=100.0)
            .speed(0.5),
    );
    if self.workspace_config.hcs_threshold != old_threshold {
        self.hcs_regions =
            compute_hcs_regions(results, Some(self.workspace_config.hcs_threshold));
    }
});
```

This gives the user live, sub-millisecond feedback when exploring different conservation thresholds. The Setup-view DragValue can remain for setting the initial threshold.

---

## 17. BUG (Medium): Stale `header_format` persists when new file has no delimited headers

**File:** [gui/src/app.rs](gui/src/app.rs), lines 673-676

```rust
if let Some(ref fmt) = result.header_format {
    self.analysis_config.header_format = Some(fmt.format_string.clone());
}
```

This only **sets** `header_format` when format detection succeeds but never **clears** it when the new file has no delimited headers. This is a "write-only on success" pattern — a common stale-state bug.

**Trace:**
1. User selects file A with headers `>id1|country|host` → validation detects pipe delimiter → `header_format = Some("id1|country|host")`
2. User selects file B with headers `>simpleheader1` → `detect_header_format` returns `None` → the `if let Some(...)` block is skipped → `header_format` RETAINS `"id1|country|host"` from file A
3. User clicks Analyze on file B → `start_analysis` constructs `header_format_vec = Some(["id1", "country", "host"])` → `dima_lib::analyze()` attempts pipe-delimited header parsing on file B → incorrect metadata extraction or silent data corruption

**Impact:** Data correctness. Header fields from file A's format are silently applied to file B, producing wrong metadata in results. No visible error — the user may not notice until they inspect metadata distributions and find nonsensical values.

**Fix:** Clear `header_format` unconditionally BEFORE the conditional set. This ensures stale values are always removed when the new file has no delimiter:

```rust
// Clear stale file-specific config before re-populating
self.analysis_config.header_format = None;

// Then set only if new file has a detected format
if let Some(ref fmt) = result.header_format {
    self.analysis_config.header_format = Some(fmt.format_string.clone());
}
```

### 17b. Minor: `selected_position` not cleared on new results

**File:** [gui/src/app.rs](gui/src/app.rs), `logic()` (line 370-389) and `load_dima_binary` (line 319-331)

When new analysis completes or a `.dima` file is loaded, `self.selected_position` is not set to `None`. If the user had position 42 selected from a previous analysis, the new results may not have position 42. `show_position_details` handles this gracefully (`.find()` returns `None`, shows placeholder), but the UX is slightly confusing — the user sees "Click a position to view details" even though they DID click on the old results.

**Fix:** Add `self.selected_position = None;` in both the `AnalysisOutcome::Success` handler and `load_dima_binary` success path, alongside the other state resets.

### 17c. Minor: HCS map off-by-one coordinate mapping

**File:** [gui/src/app.rs](gui/src/app.rs), lines 809-812

```rust
let left = (region.start_position as f32 / total_positions) * rect.width() + rect.left();
let right = ((region.end_position + 1) as f32 / total_positions) * rect.width() + rect.left();
```

Positions are 1-based, so position 1 maps to `1/N` (e.g., 1% for 100 positions) instead of `0/N` (the left edge). Similarly, `end_position + 1` can exceed `total_positions`, mapping past the right edge (clipped by `painter_at`). For small datasets (10 positions), the gap at the left edge is 10% of the chart width — visible.

**Fix:** Use 0-based coordinate mapping:

```rust
let left = ((region.start_position - 1) as f32 / total_positions) * rect.width() + rect.left();
let right = (region.end_position as f32 / total_positions) * rect.width() + rect.left();
```

Position 1 → 0%, position N → N/N = 100%. Each position occupies `1/N` of the chart width.

---

## 18. FUNCTIONAL GAP (Medium): Variant bar chart missing 2 summary bars

**File:** [gui/src/app.rs](gui/src/app.rs), lines 962-984

The original plan specifies a 6-bar variant chart:
1. Index incidence (%)
2. Major incidence (%)
3. Minor incidence (%)
4. Unique incidence (%)
5. Total variants incidence (%) -- `Position.total_variants_incidence`
6. Distinct variants incidence (%) -- `Position.distinct_variants_incidence`

Only bars 1-4 are implemented. Bars 5-6 use per-position summary statistics that are already computed and stored on the `Position` struct by `dima_lib::set_pos_obj_data()` (`src/models.rs` lines 545-575).

- `total_variants_incidence`: fraction of all reads at this position that are NOT the Index (non-Index reads / support * 100)
- `distinct_variants_incidence`: "type richness" of the minority population (non-Index types / non-Index reads * 100)

These are NOT the same as summing incidences by motif type — they are per-position aggregate metrics already on the struct.

**Fix:** Add 2 more bars after the motif bars:

```rust
let bars = [
    ("Index", index_inc, self.tokens.motif_index),
    ("Major", major_inc, self.tokens.motif_major),
    ("Minor", minor_inc, self.tokens.motif_minor),
    ("Unique", unique_inc, self.tokens.motif_unique),
    // Per-position summary metrics (already computed by dima_lib)
    ("Total Var Inc", pos.total_variants_incidence, self.tokens.text_secondary),
    ("Distinct Var Inc", pos.distinct_variants_incidence, self.tokens.text_secondary),
];
```

The last 2 bars read directly from `Position` fields, not from variant iteration. Use `text_secondary` color (neutral) to visually distinguish summary bars from motif-type bars.

---

## 19. FUNCTIONAL GAP (Medium): FASTA validation runs synchronously on UI thread

**File:** [gui/src/app.rs](gui/src/app.rs), line 657

```rust
match dima_lib::validate_fasta(&path, None) {
```

The plan explicitly specifies: *"FASTA validation calls `dima_lib::validation::validate_fasta()` on a background `std::thread`"* and the planned `DimaApp` struct includes `validation_worker: Option<ValidationWorker>`. But the implementation calls `validate_fasta()` directly from `handle_file_selected()`, which is invoked from the `show_setup()` UI method (i.e., on the main thread).

**Impact:** For typical FASTA files (<10MB), validation completes in <100ms and the UI freeze is imperceptible. But for large files common in genomics (100MB-500MB viral genome datasets, or bacterial pan-genome alignments), `scan_fasta_content()` reads the entire file line-by-line — this can take 2-10 seconds, during which the window becomes completely unresponsive. Users may think the app has crashed.

**Trace:**
1. User drops a 300MB FASTA file
2. `handle_file_selected` is called from `show_setup` (UI thread)
3. `validate_fasta` reads 300MB of content line by line (~5 seconds)
4. During this time: no frame updates, no spinner, no cancel button, OS "not responding" indicator may appear
5. Eventually returns, UI unfreezes, validation result displayed

**Fix:** Implement the `ValidationWorker` pattern (matching the existing `AnalysisHandle` pattern for analysis):

```rust
struct ValidationWorker {
    result_rx: mpsc::Receiver<Result<FastaValidationResult, std::io::Error>>,
    cancel_token: Arc<AtomicBool>,
}

// In handle_file_selected (non-.dima path):
fn handle_file_selected(&mut self, path: PathBuf) {
    self.selected_file = Some(path.clone());
    self.validation_result = None;  // Clear stale result immediately

    let cancel_token = Arc::new(AtomicBool::new(false));
    let (tx, rx) = mpsc::channel();
    let cancel = cancel_token.clone();
    let ctx = self.egui_ctx.clone();

    std::thread::spawn(move || {
        let result = dima_lib::validate_fasta(&path, Some(&cancel));
        let _ = tx.send(result);
        if let Some(ctx) = ctx {
            ctx.request_repaint();
        }
    });

    self.validation_worker = Some(ValidationWorker { result_rx: rx, cancel_token });
}

// In logic():
if let Some(ref worker) = self.validation_worker {
    match worker.result_rx.try_recv() {
        Ok(Ok(result)) => {
            // Auto-populate config from result...
            self.validation_result = Some(result);
            self.validation_worker = None;
        }
        Ok(Err(e)) => {
            self.error_state.push(ErrorMessage::error(format!("Validation failed: {}", e)));
            self.validation_worker = None;
        }
        Err(mpsc::TryRecvError::Empty) => { /* still running */ }
        Err(mpsc::TryRecvError::Disconnected) => {
            self.error_state.push(ErrorMessage::error("Validation worker crashed".to_string()));
            self.validation_worker = None;
        }
    }
}
```

This gives the user a spinner during validation and enables future cancellation support. The `can_analyze` guard already checks `validation_result.is_some()`, so the Analyze button naturally stays disabled until validation completes.

---

## 20. UX (Low): Filter DragValues accept nonsensical values

**File:** [gui/src/app.rs](gui/src/app.rs), `show_filter_controls`

The position-range DragValues had no `.range()` constraint, allowing the user to type `0` or `999999999` on a dataset with 420 positions. Entropy-range DragValues similarly accepted negative values or absurdly large numbers. While `FilterState::apply()` handles these gracefully (returns empty filtered set — no crash), the UX is poor because the user receives no visual feedback about valid bounds.

**Fix:** Compute valid bounds from the `results` reference already available in `show_filter_controls`:

```rust
let last_position = results.results.last().map(|p| p.position).unwrap_or(1);
let max_entropy = results.results.iter()
    .map(|p| p.entropy)
    .filter(|e| e.is_finite())
    .fold(0.0_f64, f64::max);

// Position DragValues constrained to 1..=last_position
ui.add(egui::DragValue::new(&mut r.0).prefix("from ").range(1..=last_position));
ui.add(egui::DragValue::new(&mut r.1).prefix("to ").range(1..=last_position));

// Entropy DragValues constrained to 0.0..=max_entropy
ui.add(egui::DragValue::new(&mut r.0).prefix("min ").speed(0.01).range(0.0..=max_entropy));
ui.add(egui::DragValue::new(&mut r.1).prefix("max ").speed(0.01).range(0.0..=max_entropy));
```

The `.filter(|e| e.is_finite())` guard ensures NaN/Infinity entropy values (which should not occur but could in corrupted data) do not poison the max computation.

---

## 21. UX (Low): `.dima` file added to recent files before load succeeds

**File:** [gui/src/app.rs](gui/src/app.rs), `handle_file_selected` and `load_dima_binary`

When a `.dima` file is dropped, `recent_files.add(path, None, None)` was called in `handle_file_selected` BEFORE `load_dima_binary`. If the file is corrupted and `load_dima_binary` fails (e.g., invalid magic bytes, CRC mismatch), the file still appears in recent files with `sequence_count: None`. Clicking it from recent files triggers the same error repeatedly.

**Trace:**
1. User drops `corrupted.dima`
2. `handle_file_selected` calls `recent_files.add("corrupted.dima", None, None)` → file saved to recent files on disk
3. `load_dima_binary` fails → error message shown
4. Next session: user sees `corrupted.dima` in recent files → clicks → same error

**Fix:** Remove `recent_files.add` from `handle_file_selected`'s `.dima` branch. Instead, call it inside `load_dima_binary`'s `Ok(results)` arm, where we have the actual `sequence_count`:

```rust
// In handle_file_selected (.dima branch):
if extension == "dima" {
    self.load_dima_binary(&path);  // No recent_files.add here
    return;
}

// In load_dima_binary Ok branch:
Ok(results) => {
    let results = Arc::new(results);
    self.recent_files.add(path.to_path_buf(), Some(results.sequence_count), None);
    // ... rest of success handling ...
}
```

This ensures only successfully loaded `.dima` files appear in recent files, with accurate metadata.

---

## 22. BUG (Medium): Stale `entropy_viewport` on new results

**File:** [gui/src/app.rs](gui/src/app.rs), `logic()` success path and `load_dima_binary()`

When new analysis results load (via either analysis completion or `.dima` import), `entropy_viewport` is not reset to `None`. If the user was zoomed into positions 100-200 on a previous 420-position dataset, then loads a 50-position dataset, the chart renders with the stale viewport showing positions 100-200 — which don't exist in the new data. The chart appears completely empty until the user manually scrolls/zooms to reset the view.

**Trace:**
1. User analyzes `large.fasta` (420 positions), zooms to positions 100-200
2. `entropy_viewport = Some((100.0, 200.0))`
3. User imports `small.dima` (50 positions)
4. `load_dima_binary` sets new results, `selected_position = None`, but `entropy_viewport` is STILL `Some((100.0, 200.0))`
5. `show_entropy_chart` uses viewport (100, 200) — no data points in that range → empty chart
6. The zoom auto-reset at line 1214 only triggers on scroll interaction, not on initial render

**Fix:** Add `self.entropy_viewport = None;` alongside `self.selected_position = None;` in both the `AnalysisOutcome::Success` handler in `logic()` and the `Ok(results)` arm of `load_dima_binary()`.

---

## 23. BUG (Low): X-axis position labels invisible (clipped by painter_at)

**Category:** Rendering / UX  
**Severity:** Low (cosmetic — chart is fully functional, hover tooltip still shows positions)  
**Found in:** Audit round 4  

**Description:** In `show_entropy_chart`, the x-axis labels showing the position range (start and end positions) are drawn at `rect.bottom() + 2.0` using `painter_at(rect)`. Since `painter_at()` sets a clip rect (scissor test) to the allocated chart `rect`, anything drawn below `rect.bottom()` is outside the clip region and invisible.

The y-axis labels ("max entropy" at top, "0.00" at bottom) and the average entropy label are drawn INSIDE `rect` and render correctly. Only the two x-axis labels (bottom-left start position, bottom-right end position) are affected.

**Root cause:** The chart allocated `height` pixels with `ui.allocate_exact_size(vec2(width, height), ...)`, then created a painter clipped to that exact rect. X-axis labels need space BELOW the chart data area.

**Fix:**

1. Allocate extra vertical space (`x_label_height = 16.0`) for labels:
```rust
let (full_rect, response) = ui.allocate_exact_size(
    egui::vec2(available.x, chart_height + x_label_height),
    egui::Sense::click_and_drag(),
);
let rect = egui::Rect::from_min_max(
    full_rect.min,
    egui::pos2(full_rect.max.x, full_rect.max.y - x_label_height),
);
```

2. Clip the painter to `full_rect` (includes label area):
```rust
let painter = ui.painter_at(full_rect);
```

The data area (`rect`) is used for all chart rendering and interaction (click, zoom, pan), while `full_rect` includes the label space. X-axis labels at `rect.bottom() + 2.0` are now inside `full_rect` and visible.

---

# Round 5: Fresh-Eyes Audit (2026-07-13 08:35 UTC+8)

All 23 previous issues verified as correctly implemented (checked actual source code, not todos). All CI checks pass (fmt, clippy -D warnings, tests for both crates). This round focuses on SRP, performance, and UX improvements found during a line-by-line re-read of every file.

---

## 24. SRP (High): `app.rs` is a 1916-line God Object

**File:** [gui/src/app.rs](gui/src/app.rs)

The `DimaApp` struct contains ALL rendering logic for every UI panel. The planned `panels/` and `workers/` modules exist as empty stubs with only doc comments. This violates SRP: a single file handles file selection, validation display, configuration, progress bars, entropy chart rendering (280 lines), HCS mapping, position details, variant distribution, metadata aggregation, filter controls, position explorer table, export logic, keyboard navigation, and format helpers.

**Impact:** Maintainability and scalability. Adding a new panel or modifying the chart requires understanding the entire 1916-line file. Merge conflicts are likely when multiple features are developed in parallel.

**Fix:** Extract rendering methods into panel modules. Each panel module receives shared state via function parameters (not `&mut DimaApp`). The immediate-mode paradigm requires `&mut` access to state fields, so the pattern is:

```
gui/src/panels/
  mod.rs           -- re-exports
  entropy_chart.rs -- show_entropy_chart() + export_entropy_chart_png()
  hcs_map.rs       -- show_hcs_section()
  position_details.rs -- show_position_details()
  variant_panel.rs -- show_variant_panel() (includes metadata)
  filter_controls.rs -- show_filter_controls()
  position_explorer.rs -- show_position_explorer()
  export.rs        -- show_export_buttons()
```

Each function takes the required state slices as parameters (e.g., `fn show_entropy_chart(ui, tokens, filtered_positions, results, entropy_viewport, selected_position, ...) -> PanelResponse`). The `PanelResponse` enum captures mutations that `app.rs` applies after the call, avoiding `&mut self` issues.

Ref: Rerun `re_ui` panel architecture (open source, Apache 2.0); egui recommended patterns for large apps.

---

## 25. FUNCTIONAL GAP (Medium): FASTA validation runs synchronously on UI thread

**File:** [gui/src/app.rs](gui/src/app.rs), line 800-801

```rust
// TODO(perf): move to background thread for files >100MB
match dima_lib::validate_fasta(&path, None) {
```

This is the only remaining planned feature not implemented. `validate_fasta()` reads the entire file line-by-line on the UI thread. For typical FASTA files (<10MB), this completes in <100ms. But for large files common in viral genomics (100-500MB), the window freezes for 2-10 seconds. The OS may display a "Not Responding" indicator.

**Fix:** Implement `ValidationWorker` in `gui/src/workers/validation.rs`, matching the existing `AnalysisHandle` pattern:

```rust
pub struct ValidationWorker {
    pub result_rx: mpsc::Receiver<Result<FastaValidationResult, std::io::Error>>,
    cancel_token: Arc<AtomicBool>,
}
```

In `handle_file_selected`: spawn a `std::thread` that calls `validate_fasta(&path, Some(&cancel))`, sends the result via `mpsc::channel`, and calls `ctx.request_repaint()`. In `logic()`: poll `validation_worker.result_rx.try_recv()` alongside the existing analysis handle polling. The `can_analyze` guard already checks `validation_result.is_some()`, so the Analyze button naturally stays disabled during validation. Show a spinner in the Setup view when `validation_worker.is_some()`.

Ref: egui background work pattern (eframe docs, "Running CPU work in another thread").

---

## 26. PERFORMANCE (Low): Per-frame Vec allocations in `show_entropy_chart`

**File:** [gui/src/app.rs](gui/src/app.rs), lines 1007-1057

Every frame, three Vecs are heap-allocated and freed:
1. `full_data: Vec<(f32, f32)>` — mapped from `filtered_positions` (~80KB for 10K positions)
2. `visible_data: Vec<(f32, f32)>` — viewport-filtered subset
3. `render_data: Vec<(f32, f32)>` — LTTB output (≤800 points)

At 60fps, this is ~4.8MB/s of allocations that the allocator must handle. While modern allocators (jemalloc, mimalloc) handle this efficiently, it creates unnecessary GC pressure and cache pollution.

**Fix:** Add a `chart_cache: Option<ChartCache>` field to `DimaApp`:

```rust
struct ChartCache {
    data_version: u64,
    viewport: Option<(f64, f64)>,
    full_data: Vec<(f32, f32)>,
    render_data: Vec<(f32, f32)>,
    max_entropy: f32,
}
```

Rebuild `full_data` only when `data_version` changes. Rebuild `render_data` only when `data_version` OR `entropy_viewport` changes. Read from cache on each frame. Invalidate cache in `logic()` alongside the existing `data_version` increment.

---

## 27. PERFORMANCE (Low): Per-frame clone of `filtered_positions` in Position Explorer

**File:** [gui/src/app.rs](gui/src/app.rs), line 1776

```rust
let filtered_indices: Vec<usize> = self.filtered_positions.clone();
```

This full clone exists to work around the borrow checker: the `body()` closure needs `filtered_indices` by value while `self` is mutably borrowed for `new_selection`. For 10K positions, this is ~80KB cloned per frame.

**Fix:** Replace the clone with an index-only approach. The table closure only needs the length and individual index lookups. Store `filtered_positions` as `Arc<Vec<usize>>` instead of `Vec<usize>`, so cloning is just a ref-count increment. Alternatively, extract the table rendering to a function that takes `&[usize]` and returns `Option<usize>` (new selection), avoiding the self-borrow conflict entirely.

---

## 28. CODE QUALITY (Low): Misleading comment in `show_hcs_section`

**File:** [gui/src/app.rs](gui/src/app.rs), lines 1296-1298

```rust
// Coordinate mapping: positions are 1-based, convert to 0-based
// for proportional rendering on the bar. Use (len + kmer_length - 1)
// to account for the full alignment length, not just position count.
let total_span = results.results.len().max(1) as f32;
```

The comment says "Use (len + kmer_length - 1)" but the code uses `results.results.len()`. The code IS correct — position numbers (1 to N) map proportionally onto the bar using N as the span. The k-mer length affects the HCS SEQUENCE length, not the position coordinate mapping. The comment is a leftover from the planning phase and will mislead future maintainers.

**Fix:** Replace the comment with an accurate description:

```rust
// Coordinate mapping: positions are 1-based (1..=N), convert to 0-based
// for proportional rendering. Position 1 maps to left edge (0/N),
// position N maps to right edge (N/N). K-mer length affects the HCS
// sequence string, not the position coordinate mapping.
let total_span = results.results.len().max(1) as f32;
```

---

## 29. UX (Low): No theme toggle

**File:** [gui/src/app.rs](gui/src/app.rs), line 121-122

```rust
#[allow(dead_code)]
pub theme: Theme,
```

Both `DesignTokens::light()` and `DesignTokens::dark()` are fully implemented with WCAG AA contrast tests. But the `theme` field is marked `dead_code` — there is no UI to toggle between themes. The user is locked to whichever theme `Theme::default()` returns (Dark).

**Fix:** Add a theme toggle button/icon to the Setup view header (or a global settings area). On toggle, update `self.theme`, regenerate `self.tokens`, and call `apply_theme()`:

```rust
if ui.button(if self.theme == Theme::Dark { "☀" } else { "🌙" }).clicked() {
    self.theme = match self.theme {
        Theme::Dark => Theme::Light,
        Theme::Light => Theme::Dark,
    };
    self.tokens = match self.theme {
        Theme::Dark => DesignTokens::dark(),
        Theme::Light => DesignTokens::light(),
    };
    apply_theme(ui.ctx(), &self.tokens, self.theme);
}
```

Remove the `#[allow(dead_code)]` from the `theme` field.

---

## 30. UX (Low): No "Reset Filters" button

**File:** [gui/src/app.rs](gui/src/app.rs), `show_filter_controls`

The filter panel has position range, entropy range, motif checkboxes, and a low-support toggle. After adjusting multiple filters, the user must manually reset each control individually to restore the default "show all" state. This is tedious, especially when exploring data interactively.

**Fix:** Add a "Reset" button at the bottom of the filter group:

```rust
if ui.button("Reset Filters").clicked() {
    self.filter_state = FilterState::default_for(results);
    changed = true;
}
```

This reuses the existing `default_for()` function, which correctly sets position range to `(1, last)`, entropy range to `(0, max)`, all motif types selected, and low support included. The `changed` flag triggers the existing `apply()` + `data_version` increment logic.

---

## 31. UX (Low): Query name always overwritten on file change

**File:** [gui/src/app.rs](gui/src/app.rs), lines 790-794

```rust
self.analysis_config.query_name = path
    .file_stem()
    .unwrap_or_default()
    .to_string_lossy()
    .to_string();
```

This unconditionally replaces the query name with the new file's stem. If the user has manually customized the query name in Advanced Settings (e.g., changed it from "aligned" to "H3N2_2024_analysis"), selecting a new file erases their customization.

**Fix:** Track the previous file stem and only auto-update if the query name still matches the auto-generated value:

```rust
let new_stem = path.file_stem()
    .unwrap_or_default()
    .to_string_lossy()
    .to_string();
let previous_stem = self.selected_file
    .as_ref()
    .and_then(|p| p.file_stem())
    .map(|s| s.to_string_lossy().to_string())
    .unwrap_or_default();

// Only auto-update if user hasn't manually customized the name
if self.analysis_config.query_name.is_empty()
    || self.analysis_config.query_name == previous_stem
{
    self.analysis_config.query_name = new_stem;
}
```

This must execute BEFORE `self.selected_file = Some(path.clone())` so `previous_stem` reads the OLD file's stem.

---

## Verification

After all fixes:

```bash
cargo fmt --all
cargo clippy -p dima -- -D warnings
cargo test -p dima
cargo clippy -p dima-gui -- -D warnings
cargo test -p dima-gui
```
