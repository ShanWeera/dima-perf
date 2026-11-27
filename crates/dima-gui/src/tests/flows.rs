//! egui_kittest flows through the real interface (plan §11, step 10d): the app is driven by
//! clicks and keys on its AccessKit tree, its workers run on real threads, and the results are
//! compared with the library's. No GPU is needed (the chart falls back to egui shapes) and no
//! native dialog is ever opened.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use egui::accesskit::Role;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;

use dima_core::model::Results;

use super::fixtures;
use crate::app::DimaApp;
use crate::app::msg::{Effect, Msg};
use crate::persist::ConfigStore;
use crate::state::{Purpose, View};
use crate::workers::export::ExportKind;
use crate::workers::inspect::Opened;

type H = Harness<'static, DimaApp>;

/// The smallest window the app allows (`app::run`).
const MIN_WINDOW: egui::Vec2 = egui::vec2(980.0, 620.0);

fn harness() -> H {
    harness_sized(egui::vec2(1500.0, 1100.0))
}

fn harness_sized(size: egui::Vec2) -> H {
    harness_with(size, 1.0)
}

/// A harness of `size` points on a screen of `ppp` physical pixels per point.
fn harness_with(size: egui::Vec2, ppp: f32) -> H {
    let mut app = DimaApp::new(ConfigStore::in_memory(), false);
    app.state.native_dialogs = false;
    // The harness runs a first pass while it is built; the app's fonts and style are installed
    // in that pass (as `DimaApp::from_creation_context` does before the first frame) and the
    // app draws from the next one, when the fonts are bound.
    let mut installed = false;
    let mut h = Harness::builder()
        .with_size(size)
        .with_pixels_per_point(ppp)
        .with_max_steps(200)
        .build_ui_state(
            move |ui, app: &mut DimaApp| {
                if installed {
                    app.frame(ui, None);
                } else {
                    crate::theme::fonts::install(ui.ctx());
                    crate::theme::style::install(ui.ctx());
                    ui.ctx()
                        .all_styles_mut(|s| s.interaction.tooltip_delay = 0.0);
                    installed = true;
                }
            },
            app,
        );
    h.run_steps(2);
    h
}

/// Steps the app until `cond` holds (workers run on real threads).
fn wait_until(h: &mut H, what: &str, cond: impl Fn(&DimaApp) -> bool) {
    let start = Instant::now();
    while !cond(h.state()) {
        assert!(
            start.elapsed() < Duration::from_secs(300),
            "timed out waiting for {what}"
        );
        h.step();
        std::thread::sleep(Duration::from_millis(5));
    }
    h.run_steps(3);
}

fn has_label(h: &H, text: &str) -> bool {
    h.query_all_by_label_contains(text).next().is_some()
}

fn open(h: &mut H, path: &Path) {
    if h.state().state.view != View::Setup {
        h.get_by_label("Setup").click();
        h.run_steps(2);
    }
    h.state_mut().state.setup.path_text = path.display().to_string();
    h.run_steps(1);
    h.get_by_label("Open").click();
    // The click is handled in the next pass, which starts the job.
    h.step();
    assert!(
        h.state().state.setup.opening.is_some() || h.state().state.setup.open_error.is_some(),
        "the click started opening the file"
    );
    wait_until(h, "the file to open", |a| {
        a.state.setup.opening.is_none()
            && (a.state.setup.inspection.is_some()
                || a.state.session.is_some()
                || a.state.setup.open_error.is_some())
    });
}

/// Shows `results` as if they had been opened from a results file (no file is read), without
/// notifications.
fn load(h: &mut H, results: &Results) {
    let path = PathBuf::from("results.json");
    let effects = crate::app::update::update(&mut h.state_mut().state, Msg::OpenPath(path.clone()));
    let Some(Effect::Open { generation, .. }) = effects.first() else {
        panic!("an open effect: {effects:?}");
    };
    let generation = *generation;
    h.state_mut().dispatch(
        Msg::Opened {
            generation,
            path,
            outcome: Ok(Opened::Results(Box::new(results.clone()))),
        },
        None,
    );
    h.state_mut().state.toasts = crate::state::toasts::Toasts::default();
    h.run_steps(4);
}

/// Turns the mouse wheel by `delta` points (negative y scrolls down, negative x right) with the
/// pointer at `at`.
fn wheel(h: &mut H, at: egui::Pos2, delta: egui::Vec2) {
    h.hover_at(at);
    h.event(egui::Event::MouseWheel {
        unit: egui::MouseWheelUnit::Point,
        delta,
        phase: egui::TouchPhase::Move,
        modifiers: egui::Modifiers::NONE,
    });
    h.run_steps(3);
}

/// Turns the mouse wheel over the conserved-region heading (in the results column, outside the
/// chart and the table) until the column is scrolled to its end, where the position table is
/// whole in the window.
fn scroll_column_to_table(h: &mut H) {
    let header = |h: &H| h.get_by_role_and_label(Role::Button, "Position").rect();
    for _ in 0..40 {
        let before = header(h);
        let at = h.get_by_label("Conserved regions").rect().center();
        wheel(h, at, egui::vec2(0.0, -60.0));
        if header(h) == before {
            let window = h.ctx.content_rect();
            assert!(
                header(h).bottom() + 4.0 * 22.0 < window.bottom(),
                "the table's header and four rows are in the window: {:?}",
                header(h)
            );
            return;
        }
    }
    panic!("the column never reached its end");
}

/// A label or control: its role, its text (a control's name, a label's or a field's value)
/// and its rectangle.
type Place = (Role, String, egui::Rect);

/// Every label and control above `bottom`, in tree order.
fn places_above(h: &H, bottom: f32) -> Vec<Place> {
    use egui_kittest::kittest::NodeT;
    h.root()
        .children_recursive()
        .filter_map(|n| {
            let a = n.accesskit_node();
            let role = a.role();
            if !(is_control(role)
                || matches!(role, Role::Label | Role::SpinButton | Role::TextInput))
            {
                return None;
            }
            let text = a.label().or_else(|| a.value()).unwrap_or_default();
            let r = n.rect();
            (r.top() < bottom).then(|| (role, text.to_owned(), r))
        })
        .collect()
}

/// A button, check box, radio button or combo box.
fn is_control(role: Role) -> bool {
    matches!(
        role,
        Role::Button | Role::CheckBox | Role::RadioButton | Role::ComboBox
    )
}

/// Panics unless every label and control of `now` is where it was in `before`: a control keeps
/// its whole rectangle, and so does a text that did not change; a text that changed keeps its
/// top and the edge it is aligned to (the left, or the right for the count at the end of the
/// filter bar).
fn assert_in_place(before: &[Place], now: &[Place], context: &str) {
    let roles = |p: &[Place]| p.iter().map(|(role, ..)| *role).collect::<Vec<_>>();
    assert_eq!(
        roles(now),
        roles(before),
        "{context}: the same labels and controls"
    );
    for (b, n) in before.iter().zip(now) {
        let kept = if is_control(b.0) || b.1 == n.1 {
            b.2 == n.2
        } else {
            b.2.top() == n.2.top() && (b.2.left() == n.2.left() || b.2.right() == n.2.right())
        };
        assert!(kept, "{context}: {b:?} moved to {n:?}");
    }
}

/// Panics naming the first two of `items` whose rectangles overlap.
fn assert_no_overlap(items: &[(String, egui::Rect)]) {
    for (i, (a, ra)) in items.iter().enumerate() {
        for (b, rb) in &items[i + 1..] {
            let common = ra.intersect(*rb);
            assert!(
                common.width() < 0.5 || common.height() < 0.5,
                "{a:?} {ra:?} overlaps {b:?} {rb:?}"
            );
        }
    }
}

fn analyse(h: &mut H) {
    h.get_by_label_contains("Analyze").click();
    h.step();
    assert!(h.state().state.running(), "the click started the analysis");
    wait_until(h, "the analysis", |a| !a.state.running());
}

fn copied(h: &H) -> Vec<String> {
    h.output()
        .platform_output
        .commands
        .iter()
        .filter_map(|c| match c {
            egui::OutputCommand::CopyText(t) => Some(t.clone()),
            _ => None,
        })
        .collect()
}

/// A DNA alignment of 150 sequences of 300 columns: columns 101–140 are gaps everywhere (no
/// support for the k-mers overlapping them) and columns 201–220 are gaps in 80 sequences (low
/// support), with headers `id|host|year`.
fn synthetic_dna(dir: &Path) -> PathBuf {
    let mut state: u64 = 0x2545_F491_4F6C_DD1D;
    let mut text = String::new();
    for i in 0..150 {
        text.push_str(&format!(">s{i}|host{}|{}\n", i % 3, 2010 + i % 5));
        for c in 1..=300 {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            let b = if (101..=140).contains(&c) || ((201..=220).contains(&c) && i < 80) {
                '-'
            } else if (state >> 60) < 13 && c % 7 != 0 {
                // Mostly conserved columns with some variation.
                ['A', 'C', 'G', 'T'][c % 4]
            } else {
                ['A', 'C', 'G', 'T'][((state >> 33) % 4) as usize]
            };
            text.push(b);
            if c % 60 == 0 {
                text.push('\n');
            }
        }
        text.push('\n');
    }
    let p = dir.join("synthetic_dna.fasta");
    std::fs::write(&p, text).unwrap();
    p
}

#[test]
fn setup_inspect_analyse_workspace_on_mers() {
    let mut h = harness();
    open(&mut h, &fixtures::mers_path());
    assert!(
        has_label(&h, "191 sequences"),
        "exact count: the probe read the whole file"
    );
    assert!(has_label(&h, "1353 columns"));
    assert!(has_label(&h, "Protein (detected from"));
    assert!(
        h.query_by_value("Auto (detected: Protein)").is_some(),
        "the alphabet choice names what Auto detected"
    );
    analyse(&mut h);
    let a = h.state();
    assert_eq!(a.state.view, View::Workspace);
    let s = a.state.session.as_ref().expect("results");
    assert_eq!(
        *s.results,
        *fixtures::mers(),
        "the app's results are the library's"
    );
    for chip in [
        "191 sequences",
        "1345 positions",
        "k = 9",
        "T = 100 (support > T eligible)",
        "mean 0.1261 bits",
        "lowest 0.0000 at position 7, 455 positions tied",
        "highest 1.5878 at position 26",
        "low support 0 (NS 0, LS 0, ELS 0)",
        "total variants at 1:",
    ] {
        assert!(has_label(&h, chip), "chip {chip:?}");
    }
    // Back and forth between the screens (G7).
    h.get_by_label("Setup").click();
    h.run_steps(2);
    assert_eq!(h.state().state.view, View::Setup);
    h.get_by_label_contains("Back to the results").click();
    h.run_steps(2);
    assert_eq!(h.state().state.view, View::Workspace);
}

#[test]
fn large_files_show_an_estimate() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("many.fa");
    let mut text = String::new();
    for i in 0..12_000 {
        text.push_str(&format!(">s{i}\nACGTACGTACGTACGTACGTACGTACGTAC\n"));
    }
    std::fs::write(&p, text).unwrap();
    let mut h = harness();
    open(&mut h, &p);
    assert!(
        has_label(&h, "sequences (estimated)"),
        "D85: ~N for large files"
    );
    assert!(has_label(&h, "Checked the first 10000 records"));
}

#[test]
fn a_run_failure_shows_the_error_card() {
    let dir = tempfile::tempdir().unwrap();
    let text = std::fs::read_to_string(fixtures::mers_path()).unwrap();
    // The 5th record gets a character that is not an amino acid.
    let mut records: Vec<String> = text
        .split('>')
        .filter(|r| !r.is_empty())
        .map(|r| format!(">{r}"))
        .collect();
    records.truncate(20);
    let bad = records[4].replacen("\nM", "\nM1", 1);
    assert_ne!(bad, records[4]);
    records[4] = bad;
    let p = dir.path().join("malformed.fasta");
    std::fs::write(&p, records.concat()).unwrap();
    let mut h = harness();
    open(&mut h, &p);
    analyse(&mut h);
    wait_until(&mut h, "the record excerpt", |a| {
        a.state
            .setup
            .failure
            .as_ref()
            .is_some_and(|c| c.excerpt_pending.is_none())
    });
    assert!(h.state().state.session.is_none());
    assert!(has_label(&h, "The analysis stopped"));
    assert!(has_label(&h, "No results were written."));
    assert!(has_label(&h, "character '1'"), "the offending character");
    assert!(has_label(&h, "Hint:"));
    let card = h.state().state.setup.failure.as_ref().unwrap();
    assert_eq!(card.location.as_ref().and_then(|l| l.record), Some(5));
    assert!(
        card.excerpt
            .as_ref()
            .unwrap()
            .around_column
            .as_ref()
            .unwrap()
            .contains("[1]")
    );
    h.get_by_label("Copy details").click();
    h.run_steps(1);
    let c = copied(&h);
    assert!(
        c.iter()
            .any(|t| t.contains("record: 5") && t.contains("No results were written")),
        "{c:?}"
    );
}

#[test]
fn header_editor() {
    let mut h = harness();
    open(&mut h, &fixtures::mers_path());
    assert!(!has_label(&h, "Header fields"));
    h.get_by_label("Stratify variants by FASTA header fields")
        .click();
    h.run_steps(2);
    assert!(has_label(&h, "Header fields"));
    assert!(
        has_label(&h, "identifier-like"),
        "accessions are all different (D57)"
    );
    // Two fields named alike: the library's error, inline, and Analyze is blocked (G8).
    h.state_mut().state.setup.header.rows[1].name = "Host".into();
    h.state_mut().state.setup.header.rows[2].name = "Host".into();
    h.run_steps(2);
    assert!(has_label(&h, "appears more than once"));
    assert!(
        h.state()
            .state
            .setup
            .options
            .problems(&h.state().state.setup.header, None)
            .iter()
            .any(|p| p.contains("more than once"))
    );
    assert!(
        !has_label(&h, "hint:"),
        "the library's message without the command line's hint"
    );
    // Paste a format.
    h.get_by_label("Paste format\u{2026}").click();
    h.run_steps(2);
    h.state_mut().dispatch(
        Msg::HeaderPasted("Accession|Host|Country|Year".into()),
        None,
    );
    h.run_steps(2);
    assert_eq!(
        h.state().state.setup.header.names(),
        ["Accession", "Host", "Country", "Year"]
    );
    assert!(!has_label(&h, "appears more than once"));
    h.get_by_label("Copy as --header-format").click();
    h.run_steps(1);
    assert!(
        copied(&h)
            .iter()
            .any(|t| t == "--header-format 'Accession|Host|Country|Year'")
    );
    // Stratify by Host and Year only, then run: the metadata is in the header-format order.
    h.state_mut().state.setup.header.rows[0].stratify = false;
    h.state_mut().state.setup.header.rows[2].stratify = false;
    analyse(&mut h);
    let s = h.state().state.session.as_ref().unwrap();
    assert_eq!(s.fields, ["Host", "Year"], "G54");
    assert_eq!(
        s.results.run.parameters.metadata_fields.value,
        Some(vec!["Host".to_owned(), "Year".to_owned()])
    );
    assert!(has_label(&h, "Metadata of"));
}

#[test]
fn no_support_bands_and_low_support_lines() {
    let dir = tempfile::tempdir().unwrap();
    let p = synthetic_dna(dir.path());
    let mut h = harness();
    open(&mut h, &p);
    analyse(&mut h);
    let s = h.state().state.session.as_ref().expect("results");
    let ns = s.results.support_label_counts.no_support;
    let ls = s.results.support_label_counts.low;
    assert!(ns > 0 && ls > 0, "NS {ns}, LS {ls}");
    assert_eq!(s.series.ns_spans.len(), 1);
    assert!(
        s.series
            .paths
            .iter()
            .flat_map(|p| &p.points)
            .all(|pt| s.results.results[pt.index].entropy.is_some()),
        "NS is never a point"
    );
    assert!(has_label(&h, "NS: no usable support"), "legend");
    assert!(has_label(&h, "support \u{2264} 100: uncorrected"), "legend");
    let chart = h.get_by_label_contains("Entropy chart:");
    assert!(
        format!("{chart:?}").contains("1 span with no usable support"),
        "counted in words that fit one: {chart:?}"
    );
    // Hover the middle of the band: its tooltip names the span.
    let span = s.series.ns_spans[0];
    let (lo, hi) = s.extent().unwrap();
    let rect = chart.rect();
    let left = rect.left() + crate::panels::entropy_chart::LEFT_MARGIN;
    let right = rect.right() - crate::panels::entropy_chart::RIGHT_MARGIN;
    #[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]
    let x =
        left + ((((span.first + span.last) as f64 / 2.0) - lo) / (hi - lo)) as f32 * (right - left);
    h.hover_at(egui::pos2(x, rect.center().y));
    h.run_steps(3);
    let expected = format!(
        "Positions {}\u{2013}{}: no usable support",
        span.first, span.last
    );
    assert!(has_label(&h, &expected), "tooltip {expected:?}");
}

#[test]
fn low_support_segments_on_mers_nucleotides() {
    let mut h = harness();
    open(&mut h, &fixtures::mers_nt_path());
    analyse(&mut h);
    let s = h.state().state.session.as_ref().expect("results");
    assert_eq!(*s.results, *fixtures::mers_nt());
    assert_eq!(s.results.support_label_counts.low, 29);
    assert_eq!(
        s.series.count(crate::charts::series::Style::LowSupport),
        29,
        "29 LS positions drawn dashed"
    );
    assert!(has_label(&h, "support \u{2264} 100: uncorrected"));
    assert!(has_label(&h, "29 with support \u{2264} 100"));
}

#[test]
fn filters_sorting_and_keyboard() {
    let mut h = harness();
    open(&mut h, &fixtures::mers_path());
    analyse(&mut h);
    let sel = |h: &H| h.state().state.session.as_ref().unwrap().selection.position;
    assert_eq!(sel(&h), Some(1));
    h.key_press(egui::Key::ArrowDown);
    h.run_steps(2);
    assert_eq!(sel(&h), Some(2));
    h.key_press(egui::Key::End);
    h.run_steps(2);
    assert_eq!(sel(&h), Some(1345));
    // Sort by entropy, descending: Home selects the highest (position 26).
    h.get_by_role_and_label(Role::Button, "Entropy").click();
    h.run_steps(2);
    h.get_by_role_and_label(Role::Button, "Entropy \u{23F6}")
        .click();
    h.run_steps(2);
    h.key_press(egui::Key::Home);
    h.run_steps(2);
    assert_eq!(sel(&h), Some(26), "keyboard follows the sorted table (G11)");
    // A motif filter narrows the table and keeps the sort (G47).
    h.get_by_role_and_label(Role::CheckBox, "Index").click();
    h.run_steps(4);
    let s = h.state().state.session.as_ref().unwrap();
    assert!(s.shown.len() < 1345 && !s.shown.is_empty());
    assert!(s.sort.is_some());
    assert!(has_label(
        &h,
        &format!("{} of 1345 positions", s.shown.len())
    ));
    h.get_by_label("Reset").click();
    h.run_steps(4);
    assert_eq!(h.state().state.session.as_ref().unwrap().shown.len(), 1345);
    // The palette jumps to a position (Ctrl/Cmd+K).
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::K);
    h.run_steps(2);
    assert!(has_label(&h, "Go to setup"));
    h.key_press(egui::Key::Escape);
    h.run_steps(2);
}

#[test]
fn diagnostics_and_conserved_regions() {
    let mut h = harness();
    open(&mut h, &fixtures::mers_nt_path());
    analyse(&mut h);
    wait_until(&mut h, "conserved regions", |a| {
        a.state
            .session
            .as_ref()
            .is_some_and(|s| a.state.hcs.current(s.id).is_some())
    });
    assert!(has_label(&h, "10 regions, 1 flagged"), "D86 at 100%");
    assert!(
        has_label(&h, "Conserved regions: 10 regions, 1 flagged"),
        "strip"
    );
    // Diagnostics: the flagged region's warning, then the input validation record.
    h.get_by_label_contains("1 warning").click();
    h.run_steps(3);
    assert!(has_label(&h, "hcs_low_support_regions"));
    assert!(has_label(&h, "Input validation"));
    // Eligible positions only: 9 regions, none flagged.
    h.get_by_label("Eligible positions only").click();
    wait_until(&mut h, "recomputed regions", |a| {
        a.state.hcs.params.eligible_only
            && a.state
                .session
                .as_ref()
                .is_some_and(|s| a.state.hcs.current(s.id).is_some())
    });
    assert!(has_label(&h, "9 regions, 0 flagged"));
    // The threshold: 95% through the exact-value field.
    h.state_mut().state.hcs.text = "95".into();
    h.state_mut().dispatch(Msg::HcsEdited, None);
    wait_until(&mut h, "regions at 95%", |a| {
        a.state.hcs.params.threshold.to_string() == "95"
            && a.state
                .session
                .as_ref()
                .is_some_and(|s| a.state.hcs.current(s.id).is_some())
    });
    let s = h.state().state.session.as_ref().unwrap();
    let r = h.state().state.hcs.current(s.id).unwrap();
    let expected = dima_core::hcs::compute(&s.results, &h.state().state.hcs.params).unwrap();
    assert_eq!(**r, expected, "the library's regions");
}

#[test]
fn exports_write_atomically_and_reopen() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = harness();
    open(&mut h, &fixtures::mers_path());
    analyse(&mut h);
    wait_until(&mut h, "conserved regions", |a| {
        a.state
            .session
            .as_ref()
            .is_some_and(|s| a.state.hcs.current(s.id).is_some())
    });
    // The menu lists every export.
    h.get_by_label("Export").click();
    h.run_steps(2);
    for k in ExportKind::ALL {
        let label = k.label(crate::charts::metric::ChartMetric::Entropy);
        assert!(has_label(&h, label), "{label}");
    }
    h.key_press(egui::Key::Escape);
    h.run_steps(2);
    let save = |h: &mut H, kind: ExportKind, name: &str| -> PathBuf {
        h.state_mut().dispatch(Msg::Export(kind), None);
        h.run_steps(2);
        let session = h.state().state.session.as_ref().unwrap().id;
        let purpose = Purpose::Export {
            kind,
            session,
            metric: crate::charts::metric::ChartMetric::Entropy,
        };
        assert_eq!(h.state().state.dialog.pending, Some(purpose));
        let path = dir.path().join(name);
        h.state_mut().dispatch(
            Msg::DialogClosed {
                purpose,
                path: Some(path.clone()),
                failed_fast: false,
            },
            None,
        );
        wait_until(h, "the export", |a| a.state.export_busy.is_none());
        path
    };
    let json = save(&mut h, ExportKind::ResultsJson, "mers.json.zst");
    let svg = save(&mut h, ExportKind::Svg, "chart");
    let png = save(&mut h, ExportKind::Png, "chart.png");
    let report = save(&mut h, ExportKind::RunReport, "run");
    let tables = save(&mut h, ExportKind::TsvFolder, "tables");
    assert!(
        std::fs::read_to_string(dir.path().join("chart.svg"))
            .unwrap()
            .contains("</svg>"),
        "{svg:?}"
    );
    assert!(image::load_from_memory(&std::fs::read(&png).unwrap()).is_ok());
    assert!(tables.join("mers_spike_aa.variants.tsv").is_file());
    let v: serde_json::Value =
        serde_json::from_slice(&std::fs::read(dir.path().join("run.json")).unwrap()).unwrap();
    assert_eq!(v["status"], "completed", "{report:?}");
    assert_eq!(
        v["outputs"][0]["name"], "mers.json.zst",
        "exported data files are outputs"
    );
    // Open the exported results again (content-sniffed, G64): same results, opened origin.
    open(&mut h, &json);
    let s = h.state().state.session.as_ref().unwrap();
    assert!(matches!(s.origin, crate::state::session::Origin::Opened(_)));
    assert_eq!(*s.results, *fixtures::mers());
    assert!(has_label(&h, "opened from mers.json.zst"));
}

/// At the smallest window the results header keeps each chip on one line (it used to break
/// them into columns a few characters wide, which pushed the chart and the table out of the
/// window), nothing above the chart overlaps anything else, the chart is in the window, and
/// the table is reached by scrolling the column, whose wheel zooms the chart without scrolling.
#[test]
fn the_workspace_fits_the_smallest_window() {
    for results in [fixtures::mers(), fixtures::mers_nt(), fixtures::mers_swe()] {
        let mut h = harness_sized(MIN_WINDOW);
        load(&mut h, &results);
        let s = h.state().state.session.as_ref().expect("results");
        for (text, _) in crate::panels::summary::chips(&s.results, s) {
            let chip = h.get_by_label(&text).rect();
            assert!(chip.height() < 24.0, "chip {text:?} is {chip:?}");
            assert!(chip.right() <= MIN_WINDOW.x, "chip {text:?} is {chip:?}");
        }
        let chart = h.get_by_label_contains("Entropy chart:").rect();
        assert!(chart.bottom() < MIN_WINDOW.y, "chart {chart:?}");
        let places: Vec<(String, egui::Rect)> = places_above(&h, chart.top())
            .into_iter()
            .map(|(_, text, rect)| (text, rect))
            .collect();
        assert_no_overlap(&places);
        // The wheel over the chart zooms it (in, then out) and leaves the column where it is,
        // although the column could scroll down.
        h.hover_at(chart.center());
        for (dy, zoomed) in [(120.0, true), (-120.0, false)] {
            h.event(egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: egui::vec2(0.0, dy),
                phase: egui::TouchPhase::Move,
                modifiers: egui::Modifiers::NONE,
            });
            h.run_steps(4);
            let viewport = h.state().state.session.as_ref().unwrap().viewport;
            assert_eq!(viewport.is_some(), zoomed, "{viewport:?}");
            assert_eq!(h.get_by_label_contains("Entropy chart:").rect(), chart);
        }
        // The table is below the window until the column scrolls to it, with the wheel
        // outside the chart.
        scroll_column_to_table(&mut h);
        let header = h.get_by_role_and_label(Role::Button, "Position").rect();
        assert!(
            header.top() >= 0.0 && header.bottom() < MIN_WINDOW.y,
            "table header {header:?}"
        );
    }
}

/// Similarity-weighted entropy is an Advanced option: once ticked, the section's title counts it
/// even when closed, the choice is remembered, the run computes the values and Run details says
/// so.
#[test]
fn the_similarity_weighted_option_is_counted_remembered_and_run() {
    use egui::accesskit::Toggled;
    use egui_kittest::kittest::NodeT;
    const OPTION: &str = "Compute similarity-weighted entropy (Wu 2025)";
    let mut h = harness();
    open(&mut h, &fixtures::mers_path());
    assert!(!has_label(&h, OPTION), "the section starts closed");
    h.get_by_role_and_label(Role::Button, "Advanced").click();
    h.run_steps(2);
    let option = h.get_by_role_and_label(Role::CheckBox, OPTION);
    assert_eq!(option.accesskit_node().toggled(), Some(Toggled::False));
    assert_eq!(
        option.accesskit_node().description(),
        Some(crate::panels::help_text::similarity_weighted_entropy()),
        "its tooltip, for screen readers too"
    );
    option.click();
    h.run_steps(2);
    let option = h.get_by_role_and_label(Role::CheckBox, OPTION);
    assert_eq!(option.accesskit_node().toggled(), Some(Toggled::True));
    assert!(
        h.state().state.settings.similarity_weighted_entropy,
        "remembered (D73)"
    );
    assert!(
        has_label(&h, "Similarity-weighted changed; default: off"),
        "its dot"
    );
    // Closed again, the title still says that an option inside differs from its default.
    h.get_by_role_and_label(Role::Button, "Advanced (1 changed)")
        .click();
    h.run_steps(2);
    assert!(!has_label(&h, OPTION));
    assert!(
        h.query_by_role_and_label(Role::Button, "Advanced (1 changed)")
            .is_some()
    );
    analyse(&mut h);
    let s = h.state().state.session.as_ref().expect("results");
    assert_eq!(*s.results, *fixtures::mers_swe(), "the library's results");
    h.get_by_label("Run details").click();
    h.run_steps(3);
    assert!(h.query_by_label("computed").is_some(), "Run details");
    assert!(h.query_by_label("not computed").is_none());
}

/// The chart's metric switch: shown only when the run computed similarity-weighted entropy, as
/// a radio group named by its visible "Metric:" label, with radio buttons that carry their
/// state and their tooltip as AccessKit description. A switch re-plots the chart under its new
/// name and moves no label or control above it, nor the chart: at the smallest window, at a
/// common one, at a width where the filter bar fits one row only with the shorter metric name,
/// on screens of 2 and 1.5 pixels per point, where a value field would otherwise show another
/// number of decimals for each metric, and after a narrowed range, whose short count would let
/// the count and Reset fit at the end of the bar's row until the switch lengthens it.
#[test]
fn the_metric_switch_moves_no_control() {
    use crate::charts::metric::ChartMetric;
    use egui::accesskit::Toggled;
    use egui_kittest::kittest::NodeT;
    for (size, ppp, narrowed) in [
        (MIN_WINDOW, 1.0, false),
        (egui::vec2(1300.0, 800.0), 1.0, false),
        (egui::vec2(1200.0, 800.0), 1.0, false),
        (MIN_WINDOW, 2.0, false),
        (egui::vec2(1300.0, 800.0), 1.5, false),
        (egui::vec2(1250.0, 800.0), 1.0, true),
    ] {
        let mut h = harness_with(size, ppp);
        load(&mut h, &fixtures::mers());
        assert!(
            h.query_by_role(Role::RadioGroup).is_none(),
            "not computed: no switch"
        );
        assert!(!has_label(&h, "Metric:"));
        load(&mut h, &fixtures::mers_swe());
        let group = h.get_by_role_and_label(Role::RadioGroup, "Metric:");
        let radios: Vec<String> = group
            .children()
            .filter(|n| n.accesskit_node().role() == Role::RadioButton)
            .filter_map(|n| n.accesskit_node().label())
            .collect();
        assert_eq!(radios, ["Entropy", "Similarity-weighted entropy"]);
        let state = |h: &H, m: ChartMetric| {
            h.get_by_role_and_label(Role::RadioButton, m.name())
                .accesskit_node()
                .toggled()
        };
        assert_eq!(state(&h, ChartMetric::Entropy), Some(Toggled::True));
        assert_eq!(
            state(&h, ChartMetric::SimilarityWeighted),
            Some(Toggled::False)
        );
        for m in ChartMetric::ALL {
            let radio = h.get_by_role_and_label(Role::RadioButton, m.name());
            assert_eq!(
                radio.accesskit_node().description(),
                Some(crate::panels::help_text::chart_metric(m))
            );
        }
        if narrowed {
            // The five highest entropies: a count of one digit.
            let session = h.state_mut().state.session.as_mut().unwrap();
            let mut values: Vec<f64> = session
                .results
                .results
                .iter()
                .filter_map(|p| p.entropy)
                .collect();
            values.sort_by(|a, b| b.total_cmp(a));
            session.filter.value_range.0 = values[4];
            session.apply_filters();
            assert!(session.shown.len() < 10, "{}", session.shown.len());
        }
        // No tooltip in the way: the pointer leaves the window before each look.
        h.event(egui::Event::PointerGone);
        h.run_steps(2);
        let chart = h.get_by_label_contains("chart: ").rect();
        let before = places_above(&h, chart.top());
        assert!(
            before.len() > 30
                && before
                    .iter()
                    .filter(|(role, ..)| *role == Role::SpinButton)
                    .count()
                    == 4,
            "the filter bar's fields are compared: {before:?}"
        );
        for (to, name) in [
            (
                ChartMetric::SimilarityWeighted,
                "Similarity-weighted entropy chart:",
            ),
            (ChartMetric::Entropy, "Entropy chart:"),
        ] {
            h.get_by_role_and_label(Role::RadioButton, to.name())
                .click();
            h.run_steps(1);
            h.event(egui::Event::PointerGone);
            h.run_steps(3);
            assert_eq!(h.state().state.session.as_ref().unwrap().metric, to);
            assert_eq!(state(&h, to), Some(Toggled::True));
            // The filter bar's fields are named by what they bound, the value fields by the
            // metric they filter (their text is only the value), once: as the name, not again
            // as a description.
            let fields: Vec<(Option<String>, Option<String>)> = h
                .root()
                .children_recursive()
                .filter(|n| n.accesskit_node().role() == Role::SpinButton)
                .take(4)
                .map(|n| {
                    let a = n.accesskit_node();
                    (a.label(), a.description())
                })
                .collect();
            let names = [
                "First position shown".to_owned(),
                "Last position shown".to_owned(),
                format!("Lowest {} shown, in bits", to.in_sentence()),
                format!("Highest {} shown, in bits", to.in_sentence()),
            ];
            assert_eq!(
                fields,
                names.map(|name| (Some(name), None)),
                "{size:?} at {ppp}"
            );
            // Every field of the screen has a name, the variant table's minimum incidence too.
            let unnamed: Vec<Option<String>> = h
                .root()
                .children_recursive()
                .map(|n| n.accesskit_node())
                .filter(|a| a.role() == Role::SpinButton && a.label().is_none())
                .map(|a| a.value())
                .collect();
            assert!(unnamed.is_empty(), "unnamed fields {unnamed:?}");
            assert_in_place(
                &before,
                &places_above(&h, chart.top()),
                &format!("{size:?} at {ppp} pixels per point, switching to {to:?}"),
            );
            assert_eq!(
                h.get_by_label_contains(name).rect(),
                chart,
                "{size:?} at {ppp}"
            );
        }
    }
}

/// With similarity-weighted entropy the results show its chip after the mean, its column after
/// entropy (sortable, "Sort by similarity-weighted entropy" as tooltip and AccessKit
/// description) and three facts of the selected position whose values line up with the
/// others; without it, none of them.
#[test]
fn similarity_weighted_values_in_the_chip_the_table_and_the_facts() {
    use egui_kittest::kittest::NodeT;
    const FACTS: [&str; 3] = [
        "Similarity-weighted entropy",
        "Normalised observed entropy",
        "Normalised\nsimilarity-weighted entropy",
    ];
    let mut h = harness();
    load(&mut h, &fixtures::mers());
    assert!(!has_label(&h, "similarity-weighted mean"));
    assert!(
        h.query_by_role_and_label(Role::Button, "Similarity-weighted")
            .is_none()
    );
    for key in FACTS {
        assert!(h.query_by_label(key).is_none(), "{key:?}");
    }
    load(&mut h, &fixtures::mers_swe());
    assert!(has_label(
        &h,
        "similarity-weighted mean 0.0363 bits (observed 0.1207)"
    ));
    let header = h.get_by_role_and_label(Role::Button, "Similarity-weighted");
    assert_eq!(
        header.accesskit_node().description().as_deref(),
        Some("Sort by similarity-weighted entropy")
    );
    let entropy = h.get_by_role_and_label(Role::Button, "Entropy").rect();
    assert!(
        (header.rect().left() - entropy.right()) < 60.0 && header.rect().left() > entropy.left(),
        "right after Entropy: {entropy:?} {:?}",
        header.rect()
    );
    assert_eq!(
        h.get_by_role_and_label(Role::Button, "Entropy")
            .accesskit_node()
            .description()
            .as_deref(),
        Some("Sort by entropy"),
        "every sortable header"
    );
    header.click();
    h.run_steps(2);
    let sort = h.state().state.session.as_ref().unwrap().sort.unwrap();
    assert_eq!(
        sort.column,
        crate::state::sort::position_columns::SIMILARITY_WEIGHTED
    );
    // The facts of position 25, the worked example.
    h.state_mut().dispatch(
        Msg::Select(25, crate::state::session::SelectSource::Palette),
        None,
    );
    h.run_steps(3);
    let mut keys = Vec::new();
    for key in FACTS {
        let k = h.get_by_role_and_label(Role::Label, key);
        assert_eq!(
            k.accesskit_node().description(),
            Some(match key {
                "Similarity-weighted entropy" => {
                    crate::panels::help_text::similarity_weighted_fact()
                }
                "Normalised observed entropy" => {
                    crate::panels::help_text::normalised_observed_fact()
                }
                _ => crate::panels::help_text::normalised_similarity_weighted_fact(),
            })
        );
        keys.push(k.rect());
    }
    // One key column for both grids: the values start where the older facts' values start
    // (the inspector is the rightmost place a value is shown).
    let value_left = |text: &str| {
        h.query_all_by_label(text)
            .map(|n| n.rect().left())
            .fold(f32::MIN, f32::max)
    };
    let observed = h
        .get_by_role_and_label(Role::Label, "Observed entropy")
        .rect();
    assert_eq!(observed.left(), keys[0].left());
    let support = value_left("170");
    assert!(support > keys[0].right(), "{support} {:?}", keys[0]);
    for value in ["0.4275 bits", "0.4282", "0.1287"] {
        assert_eq!(value_left(value), support, "{value:?}");
    }
    assert!(
        keys[2].height() > keys[0].height() * 1.5,
        "the two-line key"
    );
    // Each key is level with its value, in both grids.
    let value_rect = |text: &str| {
        h.query_all_by_label(text)
            .map(|n| n.rect())
            .max_by(|a, b| a.left().total_cmp(&b.left()))
            .unwrap()
    };
    for (key, value) in [
        (keys[0], value_rect("0.4275 bits")),
        (keys[1], value_rect("0.4282")),
        (keys[2], value_rect("0.1287")),
        (
            h.get_by_role_and_label(Role::Label, "Support").rect(),
            value_rect("170"),
        ),
    ] {
        assert!(
            (key.center().y - value.center().y).abs() < 1.5,
            "{key:?} and {value:?}"
        );
    }
}

/// At the smallest window, with the coverage and similarity-weighted columns, the table is wider
/// than its column: it scrolls sideways to the Top variant, a header that gains the keyboard
/// focus scrolls into view, and moving the selection with the keyboard keeps the sideways
/// scroll.
#[test]
fn the_table_scrolls_sideways_at_the_smallest_window() {
    let mut results = (*fixtures::mers_swe()).clone();
    results.coverage_summary.min_coverage = Some(0.95);
    let mut h = harness_sized(MIN_WINDOW);
    load(&mut h, &results);
    scroll_column_to_table(&mut h);
    let column = h.get_by_label_contains("Entropy chart:").rect().x_range();
    let top = |h: &H| h.get_by_role_and_label(Role::Button, "Top variant").rect();
    assert!(
        top(&h).right() > column.max,
        "off to the right: {:?}",
        top(&h)
    );
    assert!(has_label(&h, "Coverage"));
    // The wheel turned sideways over the table (a touchpad, or Shift and the wheel).
    let sideways = |h: &mut H, dx: f32| {
        let at = h
            .get_by_role_and_label(Role::Button, "Support")
            .rect()
            .center();
        wheel(h, at, egui::vec2(dx, 0.0));
    };
    for _ in 0..20 {
        if top(&h).right() <= column.max {
            break;
        }
        sideways(&mut h, -40.0);
    }
    let shown = top(&h);
    assert!(
        column.contains(shown.left()) && column.contains(shown.right()),
        "reachable: {shown:?} in {column:?}"
    );
    // The keyboard moves the selection and the table to its row, not sideways.
    let position = h.get_by_role_and_label(Role::Button, "Position").rect();
    let before = h.state().state.session.as_ref().unwrap().selection.position;
    for _ in 0..3 {
        h.key_press(egui::Key::ArrowDown);
        h.run_steps(2);
    }
    assert_ne!(
        h.state().state.session.as_ref().unwrap().selection.position,
        before
    );
    assert_eq!(
        h.get_by_role_and_label(Role::Button, "Position").rect(),
        position,
        "the sideways scroll is kept"
    );
    // Back to the left edge, then the focus on Top variant brings it into view.
    for _ in 0..20 {
        sideways(&mut h, 40.0);
    }
    assert!(top(&h).right() > column.max);
    h.get_by_role_and_label(Role::Button, "Top variant").focus();
    h.run_steps(4);
    let focused = top(&h);
    assert!(
        column.contains(focused.left()) && column.contains(focused.right()),
        "the focused header scrolled into view: {focused:?} in {column:?}"
    );
}

/// The keyboard route to the metric switch that the guide gives: Tab from the top of the
/// Results screen reaches the "Metric:" row through the top bar, the filter bar and the panel on
/// the right, and Space selects. Every stop on the way shows its focus: no table cell is one
/// (each used to be, with nothing drawn); the only stops that draw nothing are the variant
/// table's three column-resize handles, which egui_extras makes focusable. Leaving a value
/// field that shows the full range keeps the range exactly: egui stores the text a field showed
/// when it loses the focus, and the rounded maximum hid the highest position, enabled Reset and
/// made a later switch say the range had been reset.
#[test]
fn the_metric_switch_by_tab_and_space_keeps_the_value_range() {
    use crate::charts::metric::ChartMetric;
    use egui_kittest::kittest::NodeT;
    fn session(h: &H) -> &crate::state::session::Session {
        h.state().state.session.as_ref().unwrap()
    }
    let mut h = harness();
    load(&mut h, &fixtures::mers_swe());
    let total = session(&h).results.results.len();
    let max = session(&h).bounds.max_entropy;
    let shown = format!("{max:.4}");
    assert_ne!(
        shown.parse::<f64>().ok(),
        Some(max),
        "the field rounds the maximum"
    );
    // A field by the text it shows, anything else by its name or text.
    let focused = |h: &H| {
        h.root()
            .children_recursive()
            .find(|n| n.is_focused())
            .map(|n| {
                let a = n.accesskit_node();
                let text = if a.role() == Role::SpinButton {
                    a.value()
                } else {
                    a.label().or_else(|| a.value())
                };
                (a.role(), text.unwrap_or_default())
            })
    };
    // Nothing changed, nothing hidden, Reset off and no note.
    let unchanged = |h: &H, metric: ChartMetric, step: &str| {
        let s = session(h);
        assert_eq!(s.metric, metric, "{step}");
        assert_eq!(
            s.filter.value_range,
            (0.0, s.bounds.max_for(metric)),
            "{step}: the full range, exactly"
        );
        assert_eq!(s.shown.len(), total, "{step}: every position shown");
        let reset = h.get_by_role_and_label(Role::Button, "Reset");
        assert!(reset.accesskit_node().is_disabled(), "{step}: Reset is off");
        assert!(h.state().state.toasts.is_empty(), "{step}: no note");
    };
    let entropy = Some((Role::RadioButton, "Entropy".to_owned()));
    let mut crossed = Vec::new();
    while focused(&h) != entropy {
        // 31 stops today: 9 in the top bar, 9 in the filter bar, then the variant search and
        // minimum incidence, the 4 variant headers, the 3 variants of position 1 and the 3
        // resize handles; every cell was a stop before (44).
        assert!(
            crossed.len() < 40,
            "Tab never reached the switch: {crossed:?}"
        );
        h.key_press(egui::Key::Tab);
        h.run_steps(2);
        crossed.push(focused(&h));
    }
    let fields = |text: &str| crossed.iter().flatten().any(|(_, t)| t.as_str() == text);
    assert!(
        fields("0.0000") && fields(&shown),
        "the route crosses both value fields: {crossed:?}"
    );
    let roles = |role: Role| crossed.iter().flatten().filter(|(r, _)| *r == role).count();
    assert_eq!(roles(Role::GenericContainer), 0, "no cell: {crossed:?}");
    assert_eq!(roles(Role::Unknown), 3, "the resize handles: {crossed:?}");
    assert!(crossed.iter().all(Option::is_some), "{crossed:?}");
    unchanged(&h, ChartMetric::Entropy, "Tab to the switch");
    h.key_press(egui::Key::Space);
    h.run_steps(3);
    unchanged(&h, ChartMetric::Entropy, "Space on the metric shown");
    h.key_press(egui::Key::Tab);
    h.run_steps(2);
    h.key_press(egui::Key::Space);
    h.run_steps(3);
    unchanged(
        &h,
        ChartMetric::SimilarityWeighted,
        "Tab and Space on the other metric",
    );
    h.key_press_modifiers(egui::Modifiers::SHIFT, egui::Key::Tab);
    h.run_steps(2);
    assert_eq!(focused(&h), entropy);
    h.key_press(egui::Key::Space);
    h.run_steps(3);
    unchanged(&h, ChartMetric::Entropy, "Shift+Tab and Space back");
}

/// The metric switch from the keyboard and the command palette: Space selects the radio that
/// has the focus (given here directly; Tab reaches it in
/// `the_metric_switch_by_tab_and_space_keeps_the_value_range`); while a radio button has the
/// focus the arrow keys move between controls, not along the table, until Esc gives them back;
/// the palette offers the metric not shown. The keyboard help lists these keys.
#[test]
fn the_metric_from_the_keyboard_and_the_palette() {
    use crate::charts::metric::ChartMetric;
    let mut h = harness();
    load(&mut h, &fixtures::mers_swe());
    let metric = |h: &H| h.state().state.session.as_ref().unwrap().metric;
    let selected = |h: &H| h.state().state.session.as_ref().unwrap().selection.position;
    h.get_by_role_and_label(Role::RadioButton, "Similarity-weighted entropy")
        .focus();
    h.run_steps(2);
    h.key_press(egui::Key::Space);
    h.run_steps(3);
    assert_eq!(metric(&h), ChartMetric::SimilarityWeighted);
    let before = selected(&h);
    h.key_press(egui::Key::ArrowDown);
    h.run_steps(2);
    assert_eq!(selected(&h), before, "the focus took the arrow key");
    h.key_press(egui::Key::Escape);
    h.run_steps(2);
    h.key_press(egui::Key::ArrowDown);
    h.run_steps(2);
    assert_ne!(
        selected(&h),
        before,
        "Esc gave the arrow keys back to the table"
    );
    // The palette: the metric the chart does not show.
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::K);
    h.run_steps(2);
    assert!(has_label(&h, "Show entropy"));
    assert!(!has_label(&h, "Show similarity-weighted entropy"));
    h.get_by_label("Show entropy").click();
    h.run_steps(3);
    assert_eq!(metric(&h), ChartMetric::Entropy);
    // The export menu names the figures by the metric shown.
    h.state_mut()
        .dispatch(Msg::ChartMetric(ChartMetric::SimilarityWeighted), None);
    h.run_steps(2);
    h.get_by_label("Export").click();
    h.run_steps(2);
    for label in [
        "Similarity-weighted entropy chart (PNG)\u{2026}",
        "Similarity-weighted entropy chart (SVG)\u{2026}",
    ] {
        assert!(has_label(&h, label), "{label:?}");
    }
    assert!(!has_label(&h, "Entropy chart (PNG)\u{2026}"));
    h.key_press(egui::Key::Escape);
    h.run_steps(2);
    // Help ▸ Keyboard shortcuts.
    h.state_mut().state.windows.shortcuts = true;
    h.run_steps(2);
    for line in [
        "Tab / Shift+Tab",
        "Move between controls",
        "Space / Enter",
        "Press or select",
        "In a filter field: apply the value typed (Tab or a click elsewhere does too)",
        "Copy row, or a variant's k-mer",
        "Close the palette; in a filter field, leave it unchanged; also gives the arrow keys \
         back to the table",
    ] {
        assert!(has_label(&h, line), "{line:?}");
    }
}

/// Puts the keyboard focus in the filter bar's field `name` and selects its text, as a click
/// on it does.
fn type_into(h: &mut H, name: &str, text: &str) {
    h.get_by_role_and_label(Role::SpinButton, name).focus();
    h.run_steps(2);
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
    h.run_steps(1);
    h.event(egui::Event::Text(text.into()));
    h.run_steps(3);
}

/// A value typed into a value field is applied when it is entered (Enter, Tab or a click
/// elsewhere), not at every keystroke: typing 0.2 as the maximum used to apply 0 at the first
/// key, which hid the selected position and moved the selection away for good. Esc leaves the
/// field without applying. A typed value is kept to the four decimals the field shows, so
/// passing through the field later changes nothing: egui reads back the text a field shows
/// when it loses the focus, and a typed 0.111293 (shown as 0.1113) then became 0.1113, hid
/// position 1 (entropy 0.111294) and reset the zoom.
#[test]
fn a_typed_bound_applies_when_entered_as_the_field_shows_it() {
    const LOWEST: &str = "Lowest entropy shown, in bits";
    const HIGHEST: &str = "Highest entropy shown, in bits";
    fn session(h: &H) -> &crate::state::session::Session {
        h.state().state.session.as_ref().unwrap()
    }
    let mut h = harness();
    load(&mut h, &fixtures::mers());
    let (total, max) = (session(&h).shown.len(), session(&h).bounds.max_entropy);
    h.state_mut().dispatch(
        Msg::Select(4, crate::state::session::SelectSource::Palette),
        None,
    );
    h.run_steps(2);
    type_into(&mut h, HIGHEST, "0");
    let s = session(&h);
    assert_eq!(s.filter.value_range, (0.0, max), "nothing applied yet");
    assert_eq!((s.shown.len(), s.selection.position), (total, Some(4)));
    h.event(egui::Event::Text(".2".into()));
    h.run_steps(2);
    h.key_press(egui::Key::Enter);
    h.run_steps(4);
    let s = session(&h);
    assert_eq!(s.filter.value_range, (0.0, 0.2), "applied by Enter");
    assert_eq!(s.selection.position, Some(4), "position 4 is still shown");
    type_into(&mut h, HIGHEST, "1");
    h.key_press(egui::Key::Escape);
    h.run_steps(4);
    assert_eq!(
        session(&h).filter.value_range,
        (0.0, 0.2),
        "Esc applies nothing"
    );
    let e1 = session(&h).results.results[0].entropy.unwrap();
    assert!((0.111_293..0.1113).contains(&e1), "{e1}");
    type_into(&mut h, LOWEST, "0.111293");
    h.key_press(egui::Key::Tab);
    h.run_steps(4);
    let s = session(&h);
    assert_eq!(s.filter.value_range.0, 0.1113, "kept as the field shows it");
    assert!(
        !s.is_shown(1),
        "position 1 is below 0.1113, as the field reads"
    );
    h.state_mut().state.session.as_mut().unwrap().viewport = Some((100.5, 600.5));
    h.run_steps(2);
    h.get_by_role_and_label(Role::SpinButton, LOWEST).focus();
    h.run_steps(2);
    h.key_press(egui::Key::Tab);
    h.run_steps(4);
    let s = session(&h);
    assert_eq!(
        s.filter.value_range,
        (0.1113, 0.2),
        "passing through changes nothing"
    );
    assert_eq!(s.viewport, Some((100.5, 600.5)), "and keeps the zoom");
}

/// The filter bar keeps its controls in place while a position field is edited: focusing one
/// (which shows its number without "from"/"to") and a shorter position ("to 500") used to move
/// every control right of it by 6 to 8 px.
#[test]
fn editing_a_position_field_moves_nothing_in_the_filter_bar() {
    let mut h = harness();
    load(&mut h, &fixtures::mers());
    h.event(egui::Event::PointerGone);
    h.run_steps(2);
    // The filter bar's labels and controls but the field being edited, whose own width follows
    // its text.
    let bar = |h: &H, editing: &str| -> Vec<Place> {
        let top = h.get_by_label("Positions").rect().top() - 1.0;
        let reset = h.get_by_role_and_label(Role::Button, "Reset").rect();
        places_above(h, reset.bottom())
            .into_iter()
            .filter(|(_, text, r)| r.top() >= top && text != editing)
            .collect()
    };
    const FROM: &str = "First position shown";
    const TO: &str = "Last position shown";
    let before = bar(&h, FROM);
    assert!(before.len() > 10, "{before:?}");
    h.get_by_role_and_label(Role::SpinButton, FROM).focus();
    h.run_steps(3);
    assert_in_place(&before, &bar(&h, FROM), "'from' focused");
    h.key_press(egui::Key::Escape);
    h.run_steps(2);
    let before = bar(&h, TO);
    type_into(&mut h, TO, "500");
    h.key_press(egui::Key::Enter);
    h.run_steps(4);
    assert_eq!(
        h.state()
            .state
            .session
            .as_ref()
            .unwrap()
            .filter
            .position_range,
        (1, 500)
    );
    // The count reads "500 of 1345 positions" at the same right edge; nothing else changed.
    assert_in_place(&before, &bar(&h, TO), "'to 500'");
}

/// A metric switch in the frame of a filter edit not yet applied (a value field left by a click
/// on the switch) keeps the zoom: the edit is applied with the switch, not again on the next
/// frame (which reset the zoom).
#[test]
fn the_metric_switch_keeps_the_zoom_after_a_typed_bound() {
    use crate::charts::metric::ChartMetric;
    let mut h = harness();
    load(&mut h, &fixtures::mers_swe());
    type_into(&mut h, "Highest entropy shown, in bits", "1.2");
    h.state_mut().state.session.as_mut().unwrap().viewport = Some((100.5, 600.5));
    h.get_by_role_and_label(Role::RadioButton, "Similarity-weighted entropy")
        .click();
    h.run_steps(4);
    let s = h.state().state.session.as_ref().unwrap();
    assert_eq!(s.metric, ChartMetric::SimilarityWeighted);
    assert_eq!(
        s.filter.value_range,
        (0.0, s.bounds.max_similarity_weighted),
        "the switch resets the value range"
    );
    assert_eq!(s.viewport, Some((100.5, 600.5)), "and keeps the zoom");
    assert!(!s.filter_dirty);
}

/// Each table row is chosen with a click (also on its text, for the variants) or from the
/// keyboard, never by a stop on each of its cells, which showed no focus: the position table
/// moves with the arrow keys, and each variant the table shows whole has one stop, its
/// sequence, where Space selects it (for its metadata). No cell takes the focus on a whole
/// round of Tab.
#[test]
fn table_rows_are_chosen_by_click_or_keys_never_by_cell_stops() {
    use egui_kittest::kittest::NodeT;
    let mut h = harness();
    load(&mut h, &fixtures::hiv_pro_swe());
    let rows = h
        .state_mut()
        .state
        .session
        .as_mut()
        .unwrap()
        .visible_variants();
    let s = h.state().state.session.as_ref().unwrap();
    let variants = &s.selected().unwrap().diversity_motifs;
    let (second, third) = (
        variants[rows[1]].sequence.clone(),
        variants[rows[2]].sequence.clone(),
    );
    assert_eq!(s.selection.variant, Some(rows[0]));
    let focused = |h: &H| {
        h.root()
            .children_recursive()
            .find(|n| n.is_focused())
            .map(|n| {
                let a = n.accesskit_node();
                (
                    a.role(),
                    a.label().or_else(|| a.value()).unwrap_or_default(),
                )
            })
    };
    let mut cells = 0;
    for _ in 0..60 {
        h.key_press(egui::Key::Tab);
        h.run_steps(2);
        let now = focused(&h);
        cells += usize::from(
            now.as_ref()
                .is_some_and(|(r, _)| *r == Role::GenericContainer),
        );
        if now == Some((Role::Label, second.clone())) {
            break;
        }
    }
    assert_eq!(
        focused(&h),
        Some((Role::Label, second.clone())),
        "Tab reaches it"
    );
    h.key_press(egui::Key::Space);
    h.run_steps(3);
    let variant = |h: &H| h.state().state.session.as_ref().unwrap().selection.variant;
    assert_eq!(variant(&h), Some(rows[1]), "Space selects it");
    h.get_by_label(&third).click();
    h.run_steps(3);
    assert_eq!(
        variant(&h),
        Some(rows[2]),
        "a click on the text selects it too"
    );
    // The rest of the round, back to the top bar.
    for _ in 0..80 {
        h.key_press(egui::Key::Tab);
        h.run_steps(2);
        let now = focused(&h);
        cells += usize::from(
            now.as_ref()
                .is_some_and(|(r, _)| *r == Role::GenericContainer),
        );
        if now == Some((Role::Button, "Setup".to_owned())) {
            break;
        }
    }
    assert_eq!(focused(&h), Some((Role::Button, "Setup".to_owned())));
    assert_eq!(cells, 0, "no table cell takes the focus");
}

/// A click on the text of a table cell selects its row: the text is not selectable, which took
/// the click in 1.0.0 (a click on the "5" of position 5 left position 1 selected). Copying goes
/// through a row's menu instead (a right click): "Copy row" in both tables gives the row's cells
/// as shown, separated by tabs, and "Copy k-mer" in the variant table gives the variant's k-mer.
#[test]
fn a_click_on_a_cells_text_selects_its_row_and_its_menu_copies_it() {
    use egui_kittest::kittest::NodeT;
    let mut h = harness();
    let results = fixtures::mers_swe();
    load(&mut h, &results);
    let position = |h: &H| h.state().state.session.as_ref().unwrap().selection.position;
    assert_eq!(position(&h), Some(1));
    let labels_showing = |h: &H, text: &str| {
        h.root()
            .children_recursive()
            .filter(|n| {
                let a = n.accesskit_node();
                a.role() == Role::Label && a.value().as_deref() == Some(text)
            })
            .count()
    };
    // The top-variant cell of a position in view (its k-mer and share, which no other label
    // shows).
    let (target, top) = (2..=8u64)
        .map(|n| {
            let p = &results.results[usize::try_from(n).unwrap() - 1];
            let v = &p.diversity_motifs[0];
            let share = dima_core::human::percent_bounded(v.incidence, 1);
            (n, format!("{}  {share}", v.sequence))
        })
        .find(|(_, text)| labels_showing(&h, text) == 1)
        .expect("a position in view whose top variant no other label shows");
    let cell = |h: &H| {
        h.root()
            .children_recursive()
            .find(|n| {
                let a = n.accesskit_node();
                a.role() == Role::Label && a.value().as_deref() == Some(top.as_str())
            })
            .expect("the cell")
            .rect()
            .center()
    };
    let at = cell(&h);
    h.hover_at(at);
    h.event(egui::Event::PointerButton {
        pos: at,
        button: egui::PointerButton::Primary,
        pressed: true,
        modifiers: egui::Modifiers::NONE,
    });
    h.event(egui::Event::PointerButton {
        pos: at,
        button: egui::PointerButton::Primary,
        pressed: false,
        modifiers: egui::Modifiers::NONE,
    });
    h.run_steps(3);
    assert_eq!(
        position(&h),
        Some(target),
        "a click on the text selects the row"
    );
    // The position table's menu.
    let at = cell(&h);
    h.hover_at(at);
    for pressed in [true, false] {
        h.event(egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Secondary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        });
    }
    h.run_steps(2);
    h.get_by_label("Copy row").click();
    h.run_steps(1);
    let row = copied(&h);
    assert_eq!(row.len(), 1, "{row:?}");
    let fields: Vec<&str> = row[0].split('\t').collect();
    assert_eq!(fields.len(), 7, "{fields:?}");
    assert_eq!(
        [fields[0], fields[6]],
        [target.to_string().as_str(), top.as_str()]
    );
    h.run_steps(2);
    assert!(h.query_by_label("Copy row").is_none(), "the menu closed");
    // The variant table's menu, on a variant's sequence.
    let session = h.state().state.session.as_ref().unwrap();
    let variant = session.selected().unwrap().diversity_motifs[0].clone();
    h.get_by_label(&variant.sequence).click_secondary();
    h.run_steps(2);
    h.get_by_label("Copy k-mer").click();
    h.run_steps(1);
    assert_eq!(copied(&h), std::slice::from_ref(&variant.sequence));
    h.run_steps(2);
    h.get_by_label(&variant.sequence).click_secondary();
    h.run_steps(2);
    h.get_by_label("Copy row").click();
    h.run_steps(1);
    let row = copied(&h);
    let fields: Vec<&str> = row[0].split('\t').collect();
    assert_eq!(
        fields,
        [
            variant.count.to_string().as_str(),
            dima_core::human::percent_bounded(variant.incidence, 2).as_str(),
            variant.motif.long(),
            variant.sequence.as_str()
        ]
    );
}

/// Screen readers meet the chart's heading and legend before its zoom buttons, as the row
/// reads (1.0.0's order), although the buttons are laid out first at its top right.
#[test]
fn the_chart_heading_is_read_before_the_zoom_buttons() {
    use egui_kittest::kittest::NodeT;
    for size in [MIN_WINDOW, egui::vec2(1500.0, 1100.0)] {
        let mut h = harness_sized(size);
        load(&mut h, &fixtures::hiv_pro_swe());
        // Labels and buttons in reading order, with their top edges.
        let nodes: Vec<(Role, String, f64)> = h
            .root()
            .children_recursive()
            .filter_map(|n| {
                let a = n.accesskit_node();
                let text = a.label().or_else(|| a.value()).unwrap_or_default();
                let top = a.bounding_box()?.y0;
                matches!(a.role(), Role::Label | Role::Button).then_some((a.role(), text, top))
            })
            .collect();
        let at = |role: Role, text: &str| {
            nodes
                .iter()
                .position(|(r, t, _)| *r == role && t == text)
                .unwrap()
        };
        let fit = at(Role::Button, "Fit");
        // The heading is the "Entropy" label on the row of the buttons.
        let heading = nodes
            .iter()
            .position(|(r, t, top)| {
                *r == Role::Label && t == "Entropy" && (top - nodes[fit].2).abs() < 4.0
            })
            .unwrap();
        let hint = at(Role::Label, "drag: pan \u{2022} Shift-drag: zoom to region");
        let mean = at(Role::Label, "eligible mean");
        assert!(
            heading < mean && mean < hint && hint < fit,
            "{size:?}: {nodes:?}"
        );
        assert!(
            fit < at(Role::Button, "+") && at(Role::Button, "+") < at(Role::Button, "\u{2212}")
        );
        // Laid out as before: the buttons at the top right of the chart's column.
        let (fit, chart) = (
            h.get_by_label("Fit").rect(),
            h.get_by_label_contains("chart: ").rect(),
        );
        assert!(
            (fit.right() - chart.right()).abs() < 1.0,
            "{size:?}: {fit:?}"
        );
    }
}

/// Notifications stay in the window: a warning of three lines used to grow downwards past the
/// bottom edge, with its close button half off it. The stack ends 16 px above the bottom, the
/// newest nearest the corner, and a toast's message comes before its buttons, which are at its
/// top right.
#[test]
fn notifications_stay_in_the_window() {
    use crate::state::toasts::Severity;
    use egui_kittest::kittest::NodeT;
    const WARNING: &str = "Nothing was saved: the chart was switched to similarity-weighted \
                           entropy while the dialog was open. Export again to save the chart \
                           now shown.";
    for size in [MIN_WINDOW, egui::vec2(1360.0, 860.0)] {
        let mut h = harness_sized(size);
        load(&mut h, &fixtures::mers());
        let now = h.state().state.now;
        let toasts = &mut h.state_mut().state.toasts;
        toasts.push(Severity::Warning, WARNING.to_owned(), None, now);
        toasts.push(Severity::Info, "Opened results.json".to_owned(), None, now);
        h.run_steps(4);
        let window = h.ctx.content_rect();
        let warning = h.get_by_label(&format!("Warning: {WARNING}")).rect();
        let note = h.get_by_label("Note: Opened results.json").rect();
        let dismiss = h.get_by_role_and_label(Role::Button, "Dismiss").rect();
        for r in [warning, note, dismiss] {
            assert!(window.contains_rect(r), "{size:?}: {r:?} in {window:?}");
        }
        assert!(warning.height() > 40.0, "the warning wraps: {warning:?}");
        assert!(
            warning.bottom() < note.top(),
            "the newest is nearest the corner"
        );
        // The note's frame (8 px margin, 1 px stroke) ends 16 px above the bottom.
        assert!(
            (window.bottom() - 16.0 - 9.0 - note.bottom()).abs() < 1.0,
            "{note:?}"
        );
        assert!(dismiss.left() >= warning.right() && dismiss.top() <= warning.top() + 2.0);
        let order: Vec<String> = h
            .root()
            .children_recursive()
            .filter_map(|n| {
                n.accesskit_node()
                    .label()
                    .or_else(|| n.accesskit_node().value())
            })
            .collect();
        let at = |t: &str| order.iter().position(|o| o == t).unwrap();
        assert!(
            at(&format!("Warning: {WARNING}")) < at("Dismiss"),
            "read first"
        );
    }
}

/// A notification is as wide as its message and buttons, as in 1.0.0, so a short one covers
/// no more of the window than it needs; a long one wraps at its widest (380 px of contents).
#[test]
fn a_notification_is_as_wide_as_its_text() {
    use crate::state::toasts::Severity;
    // The left edge of a toast's message, and the window's right edge less the 16 px gap and
    // the frame's margin and stroke: the right edge of every toast's contents.
    let left = |text: &str, severity: Severity| {
        let mut h = harness();
        assert!(h.state().state.toasts.is_empty());
        let now = h.state().state.now;
        h.state_mut()
            .state
            .toasts
            .push(severity, text.to_owned(), None, now);
        h.run_steps(4);
        let right = h.ctx.content_rect().right() - 16.0 - 9.0;
        (h.get_by_label_contains(text).rect().left(), right)
    };
    for (text, severity) in [
        ("Opened results.json", Severity::Info),
        ("Short warning", Severity::Warning),
    ] {
        let (left, right) = left(text, severity);
        assert!(right - left < 200.0, "{text}: {left} to {right}");
    }
    let (left, right) = left(
        &"A long warning that has to wrap. ".repeat(6),
        Severity::Warning,
    );
    assert!((right - left - 380.0).abs() < 1.0, "{left} to {right}");
}

/// Every field, menu, slider and check box of the Setup screen has a name for screen readers:
/// the option it sets (its label beside it), or what it is for where it has no label.
#[test]
fn every_setup_control_has_a_name() {
    use egui_kittest::kittest::NodeT;
    let mut h = harness();
    open(&mut h, &fixtures::mers_path());
    h.get_by_role_and_label(Role::Button, "Advanced").click();
    h.run_steps(2);
    h.get_by_label("Stratify variants by FASTA header fields")
        .click();
    h.run_steps(3);
    let unnamed: Vec<String> = h
        .root()
        .children_recursive()
        .map(|n| n.accesskit_node())
        .filter(|a| {
            matches!(
                a.role(),
                Role::SpinButton | Role::TextInput | Role::ComboBox | Role::Slider | Role::CheckBox
            ) && a.label().is_none_or(|l| l.trim().is_empty())
        })
        .map(|a| format!("{:?} {:?}", a.role(), a.value()))
        .collect();
    assert!(unnamed.is_empty(), "unnamed: {unnamed:?}");
}

/// Every option of the setup screen gives screen readers what its tooltip shows (its AccessKit
/// description); the HCS threshold's slider and exact field, which set one value, have names
/// of their own, as does the worker-threads slider (not "of N CPUs", its visible text).
#[test]
fn every_option_reads_its_tooltip() {
    use crate::panels::analysis_options::{HCS_FIELD, HCS_SLIDER};
    use crate::panels::help_text as help;
    use egui_kittest::kittest::NodeT;
    let mut h = harness();
    open(&mut h, &fixtures::mers_path());
    h.get_by_role_and_label(Role::Button, "Advanced").click();
    h.run_steps(2);
    h.get_by_label("Stratify variants by FASTA header fields")
        .click();
    h.run_steps(3);
    let described = |h: &H, role: Role, name: &str| {
        h.get_by_role_and_label(role, name)
            .accesskit_node()
            .description()
    };
    for (role, name, tip) in [
        (Role::ComboBox, "Alphabet", help::alphabet()),
        (Role::SpinButton, "k-mer length", help::kmer_length()),
        (
            Role::SpinButton,
            "Support threshold T",
            help::support_threshold(),
        ),
        (Role::TextInput, "Query name", help::query_name()),
        (Role::Slider, HCS_SLIDER, help::hcs_threshold()),
        (
            Role::CheckBox,
            "Eligible positions only",
            help::hcs_eligible_only(),
        ),
        (
            Role::CheckBox,
            "Stratify variants by FASTA header fields",
            help::header_format(),
        ),
        (Role::ComboBox, "Entropy estimator", help::estimator()),
        (Role::ComboBox, "Validation", help::validation()),
        (Role::TextInput, "Fill value", help::fill()),
        (Role::CheckBox, "flag below", help::min_coverage()),
        (
            Role::CheckBox,
            "Compute similarity-weighted entropy (Wu 2025)",
            help::similarity_weighted_entropy(),
        ),
        (Role::Slider, "Worker threads", help::threads()),
    ] {
        assert_eq!(described(&h, role, name), Some(tip), "{name}");
    }
    assert!(
        described(&h, Role::TextInput, HCS_FIELD).is_some_and(|d| d.starts_with("Exact value")),
        "the exact field has its own name and tooltip"
    );
    assert_ne!(HCS_SLIDER, HCS_FIELD);
    assert!(
        described(&h, Role::CheckBox, "Stratify field 1").is_some(),
        "the header editor's boxes too"
    );
    assert!(
        h.query_by_role_and_label(Role::Slider, "Index incidence")
            .is_none()
    );
    // The theme switch is named in words, never by its glyph.
    assert!(
        has_label(&h, "Switch to light mode") || has_label(&h, "Switch to dark mode"),
        "theme switch"
    );
    for glyph in ["\u{2600}", "\u{1F319}"] {
        assert!(
            h.query_by_role_and_label(Role::Button, glyph).is_none(),
            "{glyph}"
        );
    }
    // The figure size fields are named apart.
    h.get_by_role_and_label(Role::Button, "Preferences").click();
    h.run_steps(3);
    for name in [
        crate::panels::preferences::FIGURE_WIDTH_NAME,
        crate::panels::preferences::FIGURE_HEIGHT_NAME,
    ] {
        assert!(described(&h, Role::SpinButton, name).is_some(), "{name}");
    }
}

/// A default k longer than a short alignment is shown as it is and named under Analyze, which
/// it keeps disabled; it is never silently replaced by the alignment length.
#[test]
fn a_default_k_longer_than_the_alignment_is_named_not_replaced() {
    use egui_kittest::kittest::NodeT;
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("short.fasta");
    std::fs::write(&p, ">a\nMKVLAE\n>b\nMKVLAE\n>c\nMKVLAQ\n").unwrap();
    let mut h = harness();
    open(&mut h, &p);
    h.run_steps(3);
    assert_eq!(
        h.state().state.setup.options.kmer_length,
        None,
        "not clamped"
    );
    let reason = "the default k for proteins (9) exceeds the alignment length (6 columns): \
                  choose a k-mer length of at most 6";
    assert!(has_label(&h, reason), "shown under Analyze");
    assert_eq!(h.state().state.not_ready().as_deref(), Some(reason));
    assert!(
        h.get_by_label_contains("Analyze")
            .accesskit_node()
            .is_disabled()
    );
}

/// An alphabet Auto cannot detect is explained with the library's reason while Auto is chosen;
/// choosing one removes the warning, and Analyze names the reason until then.
#[test]
fn an_undetected_alphabet_is_explained() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("tiny.fasta");
    std::fs::write(&p, ">a\nACD\n").unwrap();
    let mut h = harness();
    open(&mut h, &p);
    let why = "only 3 residue letters in the first record; that is too few to tell protein from \
               nucleotide sequences";
    assert!(has_label(
        &h,
        &format!("Alphabet not detected: {why}. Choose it under Alphabet below.")
    ));
    assert!(has_label(&h, &format!("Choose the alphabet: {why}")));
    h.state_mut().state.setup.options.alphabet = crate::state::options::AlphabetChoice::Protein;
    h.run_steps(3);
    assert!(!has_label(&h, "Alphabet not detected"));
    assert!(has_label(&h, "undetermined (Protein chosen below)"));
}

/// Ticking an option that is the widest row of its grid adds its dot without moving any
/// control (the dot's place is kept), so a second click at the same place reaches the same
/// check box; the main grid keeps its controls when T changes.
#[test]
fn the_change_dot_moves_no_option() {
    use egui_kittest::kittest::NodeT;
    const OPTION: &str = "Compute similarity-weighted entropy (Wu 2025)";
    let mut h = harness();
    open(&mut h, &fixtures::mers_path());
    h.get_by_role_and_label(Role::Button, "Advanced").click();
    h.run_steps(2);
    let controls = |h: &H| -> Vec<egui::Rect> {
        h.root()
            .children_recursive()
            .filter(|n| {
                matches!(
                    n.accesskit_node().role(),
                    Role::CheckBox | Role::ComboBox | Role::SpinButton | Role::TextInput
                )
            })
            .map(|n| n.rect())
            .collect()
    };
    let before = controls(&h);
    let option = h.get_by_role_and_label(Role::CheckBox, OPTION).rect();
    h.get_by_role_and_label(Role::CheckBox, OPTION).click();
    h.run_steps(3);
    assert!(has_label(&h, "Similarity-weighted changed; default: off"));
    assert_eq!(
        h.get_by_role_and_label(Role::CheckBox, OPTION).rect(),
        option
    );
    assert_eq!(controls(&h), before, "ticked");
    h.state_mut().state.setup.options.support_threshold = 50;
    h.run_steps(3);
    assert_eq!(controls(&h), before, "T changed");
}

/// The selected position's facts end before the panel's scroll bar, which floats over the
/// panel's right edge while the pointer is over the panel: the method with its fallback
/// ("observed entropy (plug_in; fallback: support at or below threshold)", the longest) used
/// to run under it.
#[test]
fn the_position_facts_end_before_the_scroll_bar() {
    use egui_kittest::kittest::NodeT;
    for size in [MIN_WINDOW, egui::vec2(1360.0, 860.0)] {
        let mut h = harness_sized(size);
        load(&mut h, &fixtures::hiv_pro_swe());
        h.state_mut().dispatch(
            Msg::Select(3, crate::state::session::SelectSource::Palette),
            None,
        );
        h.run_steps(4);
        let method = h
            .get_by_label("observed entropy (plug_in; fallback: support at or below threshold)")
            .rect();
        let bar = h
            .root()
            .children_recursive()
            .filter(|n| n.accesskit_node().role() == Role::ScrollBar)
            .map(|n| n.rect())
            .filter(|r| r.y_range().contains(method.center().y))
            .max_by(|a, b| a.right().total_cmp(&b.right()))
            .expect("the panel's scroll bar");
        assert!(
            method.right() <= bar.left(),
            "{size:?}: {method:?} under {bar:?}"
        );
    }
}

/// The results header's chips give screen readers what their tooltips show, as the facts do.
#[test]
fn the_chips_tooltips_are_their_descriptions() {
    use egui_kittest::kittest::NodeT;
    let mut h = harness();
    load(&mut h, &fixtures::mers_swe());
    let s = h.state().state.session.as_ref().unwrap();
    let chips = crate::panels::summary::chips(&s.results, s);
    assert!(
        chips
            .iter()
            .any(|(text, _)| text.starts_with("similarity-weighted mean"))
    );
    for (text, tip) in chips {
        let node = h.get_by_label(&text);
        assert_eq!(node.accesskit_node().description(), Some(tip), "{text:?}");
    }
}

/// The entropy chart's legend entries give screen readers what their tooltips show: the
/// eligible-mean entry its definition, for the metric shown.
#[test]
fn the_legend_tooltips_are_their_descriptions() {
    use crate::charts::metric::ChartMetric;
    use egui_kittest::kittest::NodeT;
    let mut h = harness();
    load(&mut h, &fixtures::mers_swe());
    let description = |h: &H| {
        h.get_by_label("eligible mean")
            .accesskit_node()
            .description()
    };
    assert_eq!(
        description(&h),
        Some(crate::panels::help_text::eligible_mean(
            ChartMetric::Entropy,
            100,
            1345
        ))
    );
    h.get_by_role_and_label(Role::RadioButton, ChartMetric::SimilarityWeighted.name())
        .click();
    h.run_steps(1);
    h.event(egui::Event::PointerGone);
    h.run_steps(3);
    assert_eq!(
        description(&h),
        Some(crate::panels::help_text::eligible_mean(
            ChartMetric::SimilarityWeighted,
            100,
            1345
        ))
    );
}
