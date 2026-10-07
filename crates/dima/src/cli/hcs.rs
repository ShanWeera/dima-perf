//! `dima hcs`: conserved regions from results.

use std::io::IsTerminal;

use dima_core::Result;
use dima_core::config::{DEFAULT_HCS_THRESHOLD, Percent};
use dima_core::hcs::{self, HcsParams};
use dima_core::human;
use dima_core::output;

use super::Context;
use super::args::HcsArgs;
use super::outputs::{self, Plan};
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
    let target = a.output.clone().unwrap_or_else(|| "-".into());
    let planned = outputs::hcs_planned(
        &target,
        a.output_format,
        a.pretty,
        std::io::stdout().is_terminal(),
    )?;
    let plan = Plan {
        outputs: vec![planned],
        ..Plan::default()
    };
    outputs::check_collisions(&plan, &[a.input.as_path()])?;
    outputs::warn_unused_pretty(a.pretty, &plan, ctx.verbosity);
    let opened = outputs::Opened::open(&plan, &ctx.signals.cancel)?;
    let results = output::read_results(&a.input)?;
    let h = hcs::compute(
        &results,
        &HcsParams {
            threshold,
            eligible_only: a.hcs.hcs_eligible_only,
        },
    )?;
    opened.write(Some(&results), Some(&h), &|| ctx.signals_check())?;
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
