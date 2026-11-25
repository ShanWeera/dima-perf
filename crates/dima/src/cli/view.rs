//! `dima view`: convert results between formats.

use std::io::IsTerminal;

use dima_core::Result;
use dima_core::hcs::{self, HcsParams};
use dima_core::output;

use super::Context;
use super::args::ViewArgs;
use super::outputs;
use super::ui;

/// Runs `dima view`.
///
/// # Errors
/// Errors reading the results or writing the outputs.
pub fn run(a: &ViewArgs, ctx: &Context) -> Result<()> {
    let plan_for = |base: &str| -> Result<outputs::Plan> {
        let plan = outputs::plan(&a.output, None, None, base, std::io::stdout().is_terminal())?;
        outputs::check_collisions(&plan, &[a.input.as_path()])?;
        Ok(plan)
    };
    // Usage errors first, and the destinations opened, before a possibly large file is read.
    let first = plan_for("dima")?;
    outputs::warn_unused_pretty(a.output.pretty, &first, ctx.verbosity);
    let early = if a.output.tsv_dir.is_none() {
        Some(outputs::Opened::open(&first, &ctx.signals.cancel)?)
    } else {
        None
    };
    let results = output::read_results(&a.input)?;
    let named;
    let (plan, opened) = match early {
        Some(opened) => (&first, opened),
        None => {
            // The tables are named after the query, as `dima analyze --tsv-dir` names them.
            named = plan_for(&results.query_name)?;
            (&named, outputs::Opened::open(&named, &ctx.signals.cancel)?)
        }
    };
    report_schema(&results, plan, ctx);
    let hcs = if plan.needs_hcs() {
        Some(hcs::compute(&results, &HcsParams::default())?)
    } else {
        None
    };
    opened.write(Some(&results), hcs.as_ref(), &|| ctx.signals_check())?;
    if plan
        .outputs
        .iter()
        .any(|p| p.content == outputs::Content::Jsonl)
    {
        ui::note(
            "JSONL holds one position per line; provenance and summaries are in the JSON output",
            ctx.verbosity,
        );
    }
    Ok(())
}

/// Tells how a file of another schema version is written: an older 1.x file gets a note where
/// the outputs hold values its schema did not record (unless the file holds them anyway, as an
/// edited one can); a newer (or unrecognised) one a warning, since this version keeps only the
/// fields it knows.
fn report_schema(results: &dima_core::model::Results, plan: &outputs::Plan, ctx: &Context) {
    let version = results.schema_version.as_str();
    let current = dima_core::model::SCHEMA_VERSION;
    if version == current {
        return;
    }
    let older = matches!(
        (
            output::parse_schema_version(version),
            output::parse_schema_version(current),
        ),
        (Some(file), Some(this)) if file < this
    );
    if older {
        if plan.writes_position_values()
            && !results.run.parameters.similarity_weighted_entropy.value
        {
            ui::note(
                &format!(
                    "results file written with schema {version}; values that schema did not \
                     record are written as null (NA in the positions table), and options it \
                     did not have as off"
                ),
                ctx.verbosity,
            );
        }
    } else {
        ui::warning(
            "newer_schema",
            &format!(
                "the results were written with schema {version}; this version writes {current} \
                 and keeps only the fields it knows"
            ),
            Some("use the DiMA version that wrote the file to keep every field"),
            ctx.verbosity,
        );
    }
}
