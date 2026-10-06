//! The run report of an analysis made in the app (D103), built with the library's types so it
//! validates against `schemas/run-report.schema.json` like the CLI's.
//!
//! The command is the equivalent `dima analyze` command: the input's file name, the parameters
//! (`AnalysisParams::to_args`) and the thread count. CPU time and peak memory are not recorded
//! (`null`): the app's process also draws the interface, so its totals would not describe the
//! analysis. No paths, user or host names are recorded; error texts are redacted.

use std::path::Path;

use dima_core::DimaError;
use dima_core::config::AnalysisParams;
use dima_core::model::Results;
use dima_core::run_report::{
    self as rr, FileRecord, PhaseTime, Platform, REPORT_SCHEMA_VERSION, Resources, RunReport,
    Status, Threads, Tool, Work,
};

use crate::workers::analysis::RunFacts;

/// The equivalent command: `dima analyze --input <name> <options…> --threads N`.
#[must_use]
pub fn command(input: &Path, params: &AnalysisParams, threads: usize) -> Vec<String> {
    let mut c = vec![
        "dima".to_owned(),
        "analyze".to_owned(),
        "--input".to_owned(),
        rr::file_name(input),
    ];
    c.extend(params.to_args());
    c.push("--threads".to_owned());
    c.push(threads.to_string());
    c
}

/// The report of a run.
///
/// `results` is present for completed runs (the input's identity comes from them); `error`
/// for failed runs; `outputs` are the data files exported from the results so far.
#[must_use]
pub fn build(
    input: &Path,
    params: &AnalysisParams,
    facts: &RunFacts,
    status: Status,
    error: Option<&DimaError>,
    results: Option<&Results>,
    outputs: &[FileRecord],
) -> RunReport {
    let exit_code = match (status, error) {
        (Status::Completed, _) => 0,
        (Status::Cancelled, _) => dima_core::error::ExitClass::Cancelled.code(),
        (Status::Failed, Some(e)) => e.class().code(),
        (Status::Failed, None) => dima_core::error::ExitClass::Internal.code(),
    };
    RunReport {
        schema_version: REPORT_SCHEMA_VERSION,
        tool: Tool {
            name: "dima-gui",
            version: env!("CARGO_PKG_VERSION"),
            target: env!("DIMA_GUI_TARGET").to_owned(),
        },
        command: command(input, params, facts.threads),
        status,
        exit_code,
        error: error
            .filter(|_| status == Status::Failed)
            .map(|e| rr::error_text(e, &[input])),
        start_time: rr::rfc3339(facts.start_time),
        end_time: rr::rfc3339(facts.end_time),
        threads: Some(Threads {
            count: facts.threads,
            started: facts.threads_started,
            source: "settings",
        }),
        platform: Platform::current(),
        resources: Resources {
            wall_seconds: facts.wall.as_secs_f64(),
            cpu_user_seconds: None,
            cpu_system_seconds: None,
            peak_rss_bytes: None,
            waited_for_input_seconds: facts.waited_for_input.map(|d| d.as_secs_f64()),
        },
        phases: facts
            .phases
            .iter()
            .map(|(p, d)| PhaseTime::new(*p, *d))
            .collect(),
        work: Work {
            records: facts.records,
            positions: facts.positions,
        },
        inputs: results
            .map(|r| FileRecord {
                role: "alignment",
                name: r.run.input.name.clone(),
                size_bytes: r.run.input.size_bytes,
                sha256: r.run.input.checksum.value.clone(),
            })
            .into_iter()
            .collect(),
        outputs: outputs.to_vec(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::num::NonZeroUsize;
    use std::sync::Arc;
    use std::sync::atomic::AtomicBool;

    use dima_core::control::Progress;

    use crate::workers::analysis::{self, Outcome, Request};

    fn validator() -> jsonschema::Validator {
        let path =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../schemas/run-report.schema.json");
        let schema: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        jsonschema::meta::validate(&schema).unwrap();
        jsonschema::options()
            .with_draft(jsonschema::Draft::Draft202012)
            .build(&schema)
            .unwrap()
    }

    fn check(v: &jsonschema::Validator, report: &RunReport) -> serde_json::Value {
        let value = serde_json::to_value(report).unwrap();
        let errors: Vec<String> = v
            .iter_errors(&value)
            .map(|e| format!("{e} at {}", e.instance_path()))
            .collect();
        assert!(errors.is_empty(), "{errors:#?}\n{value:#}");
        let text = value.to_string();
        let repo = Path::new(env!("CARGO_MANIFEST_DIR"))
            .canonicalize()
            .unwrap();
        assert!(
            !text.contains(&*repo.to_string_lossy()),
            "no directories: {text}"
        );
        assert!(
            !text.contains("/home/") && !text.contains("/tmp/"),
            "{text}"
        );
        value
    }

    #[test]
    fn completed_failed_and_cancelled_reports_validate_against_the_schema() {
        let v = validator();
        let path = crate::tests::fixtures::mers_path();
        let params = AnalysisParams {
            support_threshold: Some(30),
            ..AnalysisParams::default()
        };
        let request = Request {
            path: path.clone(),
            params: params.clone(),
            threads: NonZeroUsize::new(2).unwrap(),
            expected: None,
            expected_probe: None,
        };
        let Outcome::Completed(a, facts) = analysis::run(
            &request,
            &Arc::new(Progress::new()),
            Arc::new(AtomicBool::new(false)),
        ) else {
            panic!("completed");
        };
        let out = FileRecord {
            role: "results",
            name: "mers.json".into(),
            size_bytes: 10,
            sha256: "a".repeat(64),
        };
        let r = build(
            &path,
            &params,
            &facts,
            Status::Completed,
            None,
            Some(&a.results),
            &[out],
        );
        let value = check(&v, &r);
        assert_eq!(value["tool"]["name"], "dima-gui");
        assert_eq!(
            value["threads"],
            serde_json::json!({"count": 2, "started": 2, "source": "settings"})
        );
        assert_eq!(
            value["resources"]["peak_rss_bytes"],
            serde_json::Value::Null
        );
        assert_eq!(
            r.command,
            [
                "dima",
                "analyze",
                "--input",
                "mers_spike_aa.fasta",
                "--support-threshold",
                "30",
                "--threads",
                "2"
            ]
        );
        assert_eq!(r.inputs[0].sha256, a.results.run.input.checksum.value);
        assert_eq!(r.work.positions, Some(1345));
        assert_eq!(value["phases"].as_array().unwrap().len(), 3);

        // A failure: the error is reported without directories.
        let dir = tempfile::tempdir().unwrap();
        let bad = dir.path().join("bad input.fa");
        std::fs::write(&bad, ">a\nACDEFGHIK\n>b\nACDEF1HIK\n").unwrap();
        let params = AnalysisParams {
            alphabet: Some(dima_core::config::Alphabet::Protein),
            ..AnalysisParams::default()
        };
        let request = Request {
            path: bad.clone(),
            params: params.clone(),
            threads: NonZeroUsize::new(1).unwrap(),
            expected: None,
            expected_probe: None,
        };
        let Outcome::Failed(e, facts) = analysis::run(
            &request,
            &Arc::new(Progress::new()),
            Arc::new(AtomicBool::new(false)),
        ) else {
            panic!("failed");
        };
        let r = build(&bad, &params, &facts, Status::Failed, Some(&e), None, &[]);
        let value = check(&v, &r);
        assert_eq!(value["status"], "failed");
        assert_eq!(value["exit_code"], 4);
        let message = value["error"].as_str().unwrap();
        assert!(
            message.contains("bad input.fa") && message.contains("record 2"),
            "{message}"
        );
        assert!(r.inputs.is_empty());

        let r = build(&bad, &params, &facts, Status::Cancelled, None, None, &[]);
        let value = check(&v, &r);
        assert_eq!(value["exit_code"], 130);
        assert_eq!(value["error"], serde_json::Value::Null);
    }
}
