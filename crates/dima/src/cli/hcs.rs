//! `dima hcs`: conserved regions from results.

use std::io::IsTerminal;

use dima_core::Result;
use dima_core::config::{DEFAULT_HCS_THRESHOLD, Percent};
use dima_core::hcs::{self, HcsParams};
use dima_core::human;
use dima_core::output;

use super::Context;
use super::args::HcsArgs;
use super::destinations::{self, Destinations, Input};
use super::outputs::{self, Folders};
use super::ui;

/// Runs `dima hcs`.
///
/// # Errors
/// Errors reading the results or writing the regions.
pub fn run(a: &HcsArgs, ctx: &Context) -> Result<()> {
    let threshold = match &a.hcs.hcs_min_incidence {
        Some(t) => Percent::hcs_threshold(t)?,
        None => DEFAULT_HCS_THRESHOLD,
    };
    let destination = Destinations::of_hcs(a.output.as_deref());
    let plan = outputs::plan_hcs(
        &destination,
        a.output_format,
        a.pretty,
        std::io::stdout().is_terminal(),
    )?;
    destinations::check(&destination, Input::of(Some(&a.input)))?;
    outputs::warn_unused_pretty(a.pretty, &plan, ctx.verbosity);
    outputs::warn_assumed_format(&plan, true, ctx.verbosity);
    let mut folders = Folders::default();
    let opened = outputs::Opened::open(&plan, &mut folders, &ctx.signals.cancel)?;
    let results = output::read_results(&a.input)?;
    let h = hcs::compute(
        &results,
        &HcsParams {
            threshold,
            eligible_only: a.hcs.hcs_eligible_only,
        },
    )?;
    // A reader of standard output that went away (`| head`) ends that output only: the
    // diagnostics and the summary line are still printed.
    outputs::unless_reader_gone(opened.write(
        Some(&results),
        Some(&h),
        &|| ctx.signals_check(),
        &ctx.ledger,
    ))?;
    for d in &h.diagnostics {
        ui::diagnostic(d, ctx.verbosity);
    }
    let flagged = if h.flagged_region_count > 0 {
        format!(" ({} flagged for low support)", h.flagged_region_count)
    } else {
        String::new()
    };
    let rule = if a.hcs.hcs_eligible_only {
        ", eligible positions only"
    } else {
        ""
    };
    ui::line(
        &format!(
            "{}{flagged} with an Index incidence of at least {threshold}%{rule}",
            human::plural(h.region_count, "conserved region", "conserved regions")
        ),
        ctx.verbosity,
    );
    Ok(())
}
