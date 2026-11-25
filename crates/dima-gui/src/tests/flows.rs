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
    let mut app = DimaApp::new(ConfigStore::in_memory(), false);
    app.state.native_dialogs = false;
    // The harness runs a first pass while it is built; the app's fonts and style are installed
    // in that pass (as `DimaApp::from_creation_context` does before the first frame) and the
    // app draws from the next one, when the fonts are bound.
    let mut installed = false;
    let mut h = Harness::builder()
        .with_size(size)
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

/// The rectangles of every label and control above `bottom`, with their names.
fn controls_above(h: &H, bottom: f32) -> Vec<(String, egui::Rect)> {
    use egui_kittest::kittest::NodeT;
    let roles = [
        Role::Label,
        Role::Button,
        Role::CheckBox,
        Role::RadioButton,
        Role::SpinButton,
        Role::ComboBox,
        Role::TextInput,
    ];
    h.root()
        .children_recursive()
        .filter(|n| roles.contains(&n.accesskit_node().role()))
        .map(|n| {
            let a = n.accesskit_node();
            let name = a.label().or_else(|| a.value()).unwrap_or_default();
            (name, n.rect())
        })
        .filter(|(_, r)| r.top() < bottom)
        .collect()
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
    assert!(format!("{chart:?}").contains("1 spans with no usable support"));
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
        s.series.uncorrected_count(),
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
        assert!(has_label(&h, k.label()), "{}", k.label());
    }
    h.key_press(egui::Key::Escape);
    h.run_steps(2);
    let save = |h: &mut H, kind: ExportKind, name: &str| -> PathBuf {
        h.state_mut().dispatch(Msg::Export(kind), None);
        h.run_steps(2);
        let session = h.state().state.session.as_ref().unwrap().id;
        assert_eq!(
            h.state().state.dialog.pending,
            Some(Purpose::Export { kind, session })
        );
        let path = dir.path().join(name);
        h.state_mut().dispatch(
            Msg::DialogClosed {
                purpose: Purpose::Export { kind, session },
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
    for results in [fixtures::mers(), fixtures::mers_nt()] {
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
        assert_no_overlap(&controls_above(&h, chart.top()));
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
        // The table is below the window until the column scrolls to it.
        h.get_by_role_and_label(Role::Button, "Position")
            .scroll_to_me();
        h.run_steps(4);
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
