//! The CPUs, run as the phone's own system runs them once it has started.
//! A recovery's init leaves them as the kernel starts them: every core at
//! its top frequency, and on Qualcomm phones none powered down while idle.

use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

/// Where the kernel keeps the CPUs' frequency policies, one per cluster.
const CPUFREQ: &str = "/sys/devices/system/cpu/cpufreq";

/// The governors that set a cluster's frequency by its load, the one to
/// take first: Qualcomm's, which the phones' own boot scripts set, then the
/// kernel's own.
const GOVERNORS: [&str; 2] = ["walt", "schedutil"];

/// Qualcomm's CPU idle driver keeps idle cores from powering down until it
/// is told to let them, as the phones' own boot scripts tell it.
const SLEEP_DISABLED: &str = "/sys/devices/system/cpu/qcom_lpm/parameters/sleep_disabled";

/// Have each cluster's frequency follow its load, and let idle cores power
/// down where Qualcomm's idle driver keeps them up: what was done, for the
/// log.
pub fn run_as_system() -> Result<Vec<String>> {
    let policies = policies()?;
    let mut done = Vec::new();
    for policy in &policies {
        let name = policy.file_name().expect("a policy has a name").to_string_lossy().into_owned();
        let available = fs::read_to_string(policy.join("scaling_available_governors")).with_context(|| format!("read {name}'s governors"))?;
        match governor(&available) {
            Some(g) => {
                fs::write(policy.join("scaling_governor"), g).with_context(|| format!("set {name}'s governor to {g}"))?;
                done.push(format!("{name} {g}"));
            }
            None => done.push(format!("{name} offers none of {GOVERNORS:?}, only {}", available.trim())),
        }
    }
    if Path::new(SLEEP_DISABLED).exists() {
        fs::write(SLEEP_DISABLED, "N").with_context(|| format!("write {SLEEP_DISABLED}"))?;
        done.push("idle cores power down".into());
    }
    Ok(done)
}

/// The governor to run a cluster with, of the ones `available` lists.
fn governor(available: &str) -> Option<&'static str> {
    GOVERNORS.into_iter().find(|g| available.split_whitespace().any(|a| a == *g))
}

/// The clusters' frequency policies, in order.
fn policies() -> Result<Vec<PathBuf>> {
    let mut policies: Vec<_> = fs::read_dir(CPUFREQ)
        .with_context(|| format!("list {CPUFREQ}"))?
        .map(|e| e.map(|e| e.path()))
        .collect::<std::io::Result<_>>()
        .with_context(|| format!("list {CPUFREQ}"))?;
    policies.retain(|p| p.file_name().is_some_and(|n| n.to_string_lossy().starts_with("policy")));
    policies.sort();
    Ok(policies)
}

/// The CPUs held at their top frequency for as long as it lives, and put
/// back as they were when it goes: what a package that spends minutes
/// taking its payload apart needs, since a load-following governor leaves
/// the one core doing that work well below the frequency the cluster can
/// hold, and the writing takes as long again (2026-11-20).
pub struct Demand {
    /// Each cluster, the governor it ran before, and whether its least
    /// frequency was raised instead of its governor changed.
    previous: Vec<(PathBuf, String, bool)>,
}

/// Hold every cluster at its top frequency until the returned [`Demand`]
/// goes.
pub fn demand() -> Result<Demand> {
    let mut previous = Vec::new();
    for policy in policies()? {
        let was = match fs::read_to_string(policy.join("scaling_governor")) {
            Ok(was) => was.trim().to_string(),
            Err(_) => continue,
        };
        // `performance` pins the frequency at the top of what the cluster
        // offers; where a kernel does not have it, the least frequency is
        // raised to the most instead.
        if fs::write(policy.join("scaling_governor"), "performance").is_ok() {
            previous.push((policy, was, false));
        } else if hold_at_top(&policy).is_ok() {
            previous.push((policy, was, true));
        }
    }
    Ok(Demand { previous })
}

impl Demand {
    /// How many clusters were held, for the log.
    pub fn held(&self) -> usize {
        self.previous.len()
    }
}

impl Drop for Demand {
    fn drop(&mut self) {
        for (policy, was, raised) in &self.previous {
            if *raised
                && let Ok(least) = fs::read_to_string(policy.join("cpuinfo_min_freq"))
            {
                let _ = fs::write(policy.join("scaling_min_freq"), least.trim());
            }
            let _ = fs::write(policy.join("scaling_governor"), was);
        }
    }
}

/// Raise a cluster's least frequency to its most, for a kernel without
/// the `performance` governor.
fn hold_at_top(policy: &Path) -> Result<()> {
    let top = fs::read_to_string(policy.join("cpuinfo_max_freq")).with_context(|| format!("read {policy:?}'s top frequency"))?;
    fs::write(policy.join("scaling_min_freq"), top.trim()).with_context(|| format!("hold {policy:?} at {top}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_cluster_runs_qualcomm_s_governor_where_it_has_it_and_the_kernel_s_otherwise() {
        assert_eq!(governor("walt conservative powersave performance schedutil \n"), Some("walt"));
        assert_eq!(governor("conservative ondemand userspace powersave performance schedutil"), Some("schedutil"));
        assert_eq!(governor("performance"), None);
        assert_eq!(governor("walty schedutilx"), None, "a name is matched whole");
    }
}
