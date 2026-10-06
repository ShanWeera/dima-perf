//! Worker-thread count (D101): `--threads`, else `RAYON_NUM_THREADS`, else the CPUs this
//! process may use limited to the CPUs a batch scheduler granted on this host.
//!
//! Scheduler variables are read only where they describe *this host's* allocation (multi-node
//! jobs list every host); malformed values are ignored. `SLURM_JOB_CPUS_PER_NODE`,
//! `SLURM_NTASKS` and `OMP_NUM_THREADS` are deliberately not used.

use std::fmt;
use std::num::NonZeroUsize;

/// Where the thread count came from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Source {
    /// `--threads`.
    Flag,
    /// `RAYON_NUM_THREADS`.
    RayonEnv,
    /// A scheduler variable limited the CPUs.
    Scheduler {
        /// The variable's name (all the run report records; D103).
        variable: &'static str,
        /// What it said, for the `-v` line (`NAME=value`; may name this host).
        detail: String,
    },
    /// The CPUs available to the process.
    Os,
}

/// The resolved thread count.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Threads {
    /// Worker threads.
    pub count: NonZeroUsize,
    /// Its origin.
    pub source: Source,
    /// CPUs available to the process.
    pub available: usize,
}

impl fmt::Display for Threads {
    /// `8 (SLURM_CPUS_ON_NODE=8; OS 64)`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let src = match &self.source {
            Source::Flag => "--threads".to_owned(),
            Source::RayonEnv => "RAYON_NUM_THREADS".to_owned(),
            Source::Scheduler { detail, .. } => detail.clone(),
            Source::Os => "available CPUs".to_owned(),
        };
        write!(f, "{} ({src}; OS {})", self.count, self.available)
    }
}

/// The warning when the system started only `started` of the `requested` worker threads (D110),
/// naming what asked for that many (never a variable's value, which may name this host).
pub fn refused_message(started: usize, requested: usize, source: &Source) -> String {
    let by = match source {
        Source::Flag => "--threads",
        Source::RayonEnv => "RAYON_NUM_THREADS",
        Source::Scheduler { variable, .. } => variable,
        Source::Os => "one per CPU",
    };
    format!(
        "using {started} of the {requested} worker threads requested ({by}); the system refused \
         to start more"
    )
}

/// Resolves the thread count from the flag and the environment (`env` returns a variable).
///
/// # Errors
/// A message when `RAYON_NUM_THREADS` is set but not an integer ≥ 1.
pub fn resolve(
    flag: Option<NonZeroUsize>,
    env: &dyn Fn(&str) -> Option<String>,
    available: usize,
    hostname: Option<&str>,
) -> Result<Threads, String> {
    let available = available.max(1);
    if let Some(count) = flag {
        return Ok(Threads {
            count,
            source: Source::Flag,
            available,
        });
    }
    if let Some(v) = env("RAYON_NUM_THREADS") {
        let count = v
            .trim()
            .parse::<NonZeroUsize>()
            // The value is not repeated: messages end up in run reports, which never hold
            // environment values (D103).
            .map_err(|_| "RAYON_NUM_THREADS is not a whole number of at least 1".to_owned())?;
        return Ok(Threads {
            count,
            source: Source::RayonEnv,
            available,
        });
    }
    if let Some((grant, variable, detail)) = scheduler_grant(env, hostname) {
        if grant < available {
            return Ok(Threads {
                count: NonZeroUsize::new(grant).unwrap_or(NonZeroUsize::MIN),
                source: Source::Scheduler { variable, detail },
                available,
            });
        }
    }
    Ok(Threads {
        count: NonZeroUsize::new(available).unwrap_or(NonZeroUsize::MIN),
        source: Source::Os,
        available,
    })
}

fn positive(v: &str) -> Option<usize> {
    v.trim().parse::<usize>().ok().filter(|&n| n > 0)
}

/// Expands Slurm's compact per-node lists: `"4,2(x3)"` → `[4, 2, 2, 2]`.
fn expand_slurm_list(v: &str) -> Option<Vec<usize>> {
    let mut out = Vec::new();
    for part in v.split(',') {
        if let Some((n, rep)) = part.split_once("(x") {
            let reps: usize = rep.strip_suffix(')')?.parse().ok()?;
            let n: usize = n.parse().ok()?;
            out.extend(std::iter::repeat_n(n, reps));
        } else {
            out.push(part.parse().ok()?);
        }
    }
    Some(out)
}

fn short_host(h: &str) -> &str {
    h.split('.').next().unwrap_or(h)
}

/// CPUs granted on this host by Slurm, PBS, SGE or LSF, with the variable that said so and
/// what it said.
fn scheduler_grant(
    env: &dyn Fn(&str) -> Option<String>,
    hostname: Option<&str>,
) -> Option<(usize, &'static str, String)> {
    let var = |name: &'static str| {
        env(name).and_then(|v| positive(&v).map(|n| (n, name, format!("{name}={v}"))))
    };
    if env("SLURM_JOB_ID").is_some() {
        let in_step = env("SLURM_STEP_ID").is_some();
        if in_step {
            if let Some(g) = var("SLURM_CPUS_PER_TASK") {
                return Some(g);
            }
            let on_node = env("SLURM_CPUS_ON_NODE").and_then(|v| positive(&v))?;
            let tasks = env("SLURM_STEP_TASKS_PER_NODE").and_then(|v| expand_slurm_list(&v))?;
            let node: usize = env("SLURM_NODEID")
                .and_then(|v| v.parse().ok())
                .unwrap_or(0);
            let here = *tasks.get(node)?;
            return (here > 0 && on_node / here > 0).then(|| {
                (
                    on_node / here,
                    "SLURM_STEP_TASKS_PER_NODE",
                    format!(
                        "SLURM_CPUS_ON_NODE={on_node}/SLURM_STEP_TASKS_PER_NODE[{node}]={here}"
                    ),
                )
            });
        }
        return var("SLURM_CPUS_ON_NODE");
    }
    if env("PBS_JOBID").is_some() {
        return var("PBS_NUM_PPN").or_else(|| var("NCPUS"));
    }
    if let Some(file) = env("PE_HOSTFILE") {
        let text = std::fs::read_to_string(&file).ok()?;
        let lines: Vec<(&str, usize)> = text
            .lines()
            .filter_map(|l| {
                let mut f = l.split_whitespace();
                Some((f.next()?, f.next()?.parse().ok()?))
            })
            .collect();
        if lines.len() == 1 {
            // A count of 0 grants nothing: it is ignored like any malformed value.
            return var("NSLOTS").or_else(|| {
                (lines[0].1 > 0).then(|| (lines[0].1, "PE_HOSTFILE", "PE_HOSTFILE".to_owned()))
            });
        }
        let host = short_host(hostname?);
        let slots: usize = lines
            .iter()
            .filter(|(h, _)| short_host(h) == host)
            .map(|(_, n)| n)
            .sum();
        return (slots > 0).then(|| (slots, "PE_HOSTFILE", format!("PE_HOSTFILE[{host}]={slots}")));
    }
    if let Some(hosts) = env("LSB_MCPU_HOSTS") {
        let pairs: Vec<(&str, usize)> = hosts
            .split_whitespace()
            .collect::<Vec<_>>()
            .chunks(2)
            .filter_map(|c| Some((*c.first()?, c.get(1)?.parse().ok()?)))
            .collect();
        if pairs.len() == 1 {
            return var("LSB_DJOB_NUMPROC").or_else(|| {
                (pairs[0].1 > 0).then(|| {
                    (
                        pairs[0].1,
                        "LSB_MCPU_HOSTS",
                        format!("LSB_MCPU_HOSTS={hosts}"),
                    )
                })
            });
        }
        let host = short_host(hostname?);
        let n: usize = pairs
            .iter()
            .filter(|(h, _)| short_host(h) == host)
            .map(|(_, n)| n)
            .sum();
        return (n > 0).then(|| (n, "LSB_MCPU_HOSTS", format!("LSB_MCPU_HOSTS[{host}]={n}")));
    }
    None
}

/// This host's name (for multi-host SGE/LSF allocations).
pub fn hostname() -> Option<String> {
    std::env::var("HOSTNAME")
        .ok()
        .filter(|h| !h.is_empty())
        .or_else(platform_hostname)
}

#[cfg(unix)]
fn platform_hostname() -> Option<String> {
    let mut buf = [0u8; 256];
    // SAFETY: the buffer is valid for its length; gethostname NUL-terminates on success
    // (POSIX allows truncation without NUL, which the search below handles).
    let rc = unsafe { libc::gethostname(buf.as_mut_ptr().cast(), buf.len()) };
    if rc != 0 {
        return None;
    }
    let end = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
    std::str::from_utf8(&buf[..end]).ok().map(str::to_owned)
}

#[cfg(not(unix))]
fn platform_hostname() -> Option<String> {
    std::env::var("COMPUTERNAME").ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn with(vars: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
        let m: HashMap<String, String> = vars
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect();
        move |k: &str| m.get(k).cloned()
    }

    fn run(vars: &[(&str, &str)], available: usize) -> Result<Threads, String> {
        resolve(None, &with(vars), available, Some("node2.cluster"))
    }

    #[test]
    fn precedence() {
        let t = resolve(
            NonZeroUsize::new(3),
            &with(&[("RAYON_NUM_THREADS", "5")]),
            64,
            None,
        )
        .unwrap();
        assert_eq!((t.count.get(), t.source), (3, Source::Flag));
        let t = run(
            &[
                ("RAYON_NUM_THREADS", "5"),
                ("SLURM_JOB_ID", "1"),
                ("SLURM_CPUS_ON_NODE", "2"),
            ],
            64,
        )
        .unwrap();
        assert_eq!((t.count.get(), t.source), (5, Source::RayonEnv));
        assert!(run(&[("RAYON_NUM_THREADS", "0")], 4).is_err());
        assert!(run(&[("RAYON_NUM_THREADS", "x")], 4).is_err());
        let t = run(&[], 6).unwrap();
        assert_eq!(t.to_string(), "6 (available CPUs; OS 6)");
        assert_eq!((t.count.get(), t.source), (6, Source::Os));
    }

    #[test]
    fn slurm() {
        let t = run(&[("SLURM_JOB_ID", "9"), ("SLURM_CPUS_ON_NODE", "8")], 64).unwrap();
        assert_eq!(t.count.get(), 8);
        assert_eq!(t.to_string(), "8 (SLURM_CPUS_ON_NODE=8; OS 64)");
        let t = run(
            &[
                ("SLURM_JOB_ID", "9"),
                ("SLURM_STEP_ID", "0"),
                ("SLURM_CPUS_PER_TASK", "4"),
                ("SLURM_CPUS_ON_NODE", "16"),
            ],
            64,
        )
        .unwrap();
        assert_eq!(t.count.get(), 4);
        let t = run(
            &[
                ("SLURM_JOB_ID", "9"),
                ("SLURM_STEP_ID", "0"),
                ("SLURM_CPUS_ON_NODE", "16"),
                ("SLURM_STEP_TASKS_PER_NODE", "4,2(x3)"),
                ("SLURM_NODEID", "2"),
            ],
            64,
        )
        .unwrap();
        assert_eq!(t.count.get(), 8);
        // Never above the CPUs actually available.
        let t = run(&[("SLURM_JOB_ID", "9"), ("SLURM_CPUS_ON_NODE", "128")], 16).unwrap();
        assert_eq!((t.count.get(), t.source), (16, Source::Os));
        // Malformed values are ignored.
        let t = run(&[("SLURM_JOB_ID", "9"), ("SLURM_CPUS_ON_NODE", "lots")], 16).unwrap();
        assert_eq!(t.source, Source::Os);
        assert_eq!(expand_slurm_list("3(x2),1"), Some(vec![3, 3, 1]));
        assert_eq!(expand_slurm_list("x"), None);
    }

    #[test]
    fn pbs_sge_lsf() {
        assert_eq!(
            run(&[("PBS_JOBID", "1"), ("PBS_NUM_PPN", "6")], 64)
                .unwrap()
                .count
                .get(),
            6
        );
        assert_eq!(
            run(&[("PBS_JOBID", "1"), ("NCPUS", "3")], 64)
                .unwrap()
                .count
                .get(),
            3
        );
        let dir = tempfile::tempdir().unwrap();
        let hf = dir.path().join("pe");
        std::fs::write(
            &hf,
            "node1.cluster 4 all.q UNDEFINED\nnode2.cluster 6 all.q UNDEFINED\n",
        )
        .unwrap();
        let t = run(
            &[("PE_HOSTFILE", hf.to_str().unwrap()), ("NSLOTS", "10")],
            64,
        )
        .unwrap();
        assert_eq!(t.count.get(), 6, "this host's slots, not NSLOTS");
        std::fs::write(&hf, "node2 5 all.q UNDEFINED\n").unwrap();
        assert_eq!(
            run(
                &[("PE_HOSTFILE", hf.to_str().unwrap()), ("NSLOTS", "5")],
                64
            )
            .unwrap()
            .count
            .get(),
            5
        );
        assert_eq!(
            run(&[("LSB_MCPU_HOSTS", "node1 4 node2 2")], 64)
                .unwrap()
                .count
                .get(),
            2
        );
        assert_eq!(
            run(
                &[("LSB_MCPU_HOSTS", "node2 8"), ("LSB_DJOB_NUMPROC", "8")],
                64
            )
            .unwrap()
            .count
            .get(),
            8
        );
        // A slot count of 0 grants nothing and is ignored, like any malformed value.
        std::fs::write(&hf, "node2 0 all.q UNDEFINED\n").unwrap();
        let t = run(&[("PE_HOSTFILE", hf.to_str().unwrap())], 64).unwrap();
        assert_eq!((t.count.get(), t.source), (64, Source::Os));
        let t = run(&[("LSB_MCPU_HOSTS", "node2 0")], 64).unwrap();
        assert_eq!((t.count.get(), t.source), (64, Source::Os));
    }

    #[test]
    fn refusals_name_what_asked_for_the_threads() {
        assert_eq!(
            refused_message(16, 64, &Source::Flag),
            "using 16 of the 64 worker threads requested (--threads); the system refused to start more"
        );
        let slurm = Source::Scheduler {
            variable: "SLURM_CPUS_ON_NODE",
            detail: "SLURM_CPUS_ON_NODE=64".into(),
        };
        assert!(refused_message(1, 64, &slurm).contains("(SLURM_CPUS_ON_NODE)"));
        assert!(refused_message(3, 8, &Source::Os).contains("(one per CPU)"));
        assert!(refused_message(3, 8, &Source::RayonEnv).contains("(RAYON_NUM_THREADS)"));
    }
}
