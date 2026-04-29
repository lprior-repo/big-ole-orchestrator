use crate::data::FleetError;
use std::process::Stdio;
use tokio::process::Command;
use tracing::{info, warn};

#[derive(Debug, Clone, Copy)]
struct RigRepo {
    name: &'static str,
    src_dir: &'static str,
}

const RIG_REPOS: [RigRepo; 6] = [
    RigRepo {
        name: "veloxide",
        src_dir: "/home/lewis/src/veloxide",
    },
    RigRepo {
        name: "hardline",
        src_dir: "/home/lewis/src/hardline",
    },
    RigRepo {
        name: "twerk",
        src_dir: "/home/lewis/src/twerk",
    },
    RigRepo {
        name: "seshat",
        src_dir: "/home/lewis/src/Seshat",
    },
    RigRepo {
        name: "cdocs",
        src_dir: "/home/lewis/src/centralized-docs",
    },
    RigRepo {
        name: "clarity",
        src_dir: "/home/lewis/src/clarity",
    },
];

#[derive(Debug, Default)]
pub struct BranchLandingSummary {
    pub repos_scanned: u32,
    pub branches_seen: u32,
    pub branches_landed: u32,
    pub branches_failed: u32,
    pub repos_skipped: u32,
}

impl BranchLandingSummary {
    const fn add(&mut self, other: &Self) {
        self.repos_scanned += other.repos_scanned;
        self.branches_seen += other.branches_seen;
        self.branches_landed += other.branches_landed;
        self.branches_failed += other.branches_failed;
        self.repos_skipped += other.repos_skipped;
    }
}

pub fn candidate_remote_branches(stdout: &str) -> Vec<String> {
    stdout
        .lines()
        .map(str::trim)
        .filter(|branch| branch.starts_with("origin/"))
        .filter(|branch| *branch != "origin/main" && *branch != "origin/HEAD")
        .filter(|branch| !branch.contains(" -> "))
        .map(ToString::to_string)
        .collect()
}

pub async fn land_remote_branches_for_all_rigs() -> BranchLandingSummary {
    let mut summary = BranchLandingSummary::default();

    for rig in RIG_REPOS {
        match land_remote_branches_for_rig(rig).await {
            Ok(rig_summary) => summary.add(&rig_summary),
            Err(error) => {
                warn!("{}: branch landing failed: {}", rig.name, error);
                summary.repos_scanned += 1;
                summary.branches_failed += 1;
            }
        }
    }

    info!(
        "branch landing done: repos={} skipped={} seen={} landed={} failed={}",
        summary.repos_scanned,
        summary.repos_skipped,
        summary.branches_seen,
        summary.branches_landed,
        summary.branches_failed
    );

    summary
}

async fn land_remote_branches_for_rig(rig: RigRepo) -> Result<BranchLandingSummary, FleetError> {
    let mut summary = BranchLandingSummary {
        repos_scanned: 1,
        ..BranchLandingSummary::default()
    };

    if repo_has_local_changes(rig).await? {
        warn!(
            "{}: skipping branch landing because source repo is dirty",
            rig.name
        );
        summary.repos_skipped = 1;
        return Ok(summary);
    }

    git_status(rig, &["fetch", "origin", "--prune"]).await?;
    git_status(rig, &["checkout", "main"]).await?;
    git_status(rig, &["pull", "--ff-only", "origin", "main"]).await?;

    let branches_output = git_output(
        rig,
        &[
            "for-each-ref",
            "--format=%(refname:short)",
            "refs/remotes/origin",
        ],
    )
    .await?;
    let stdout = String::from_utf8(branches_output.stdout)?;
    let branches = candidate_remote_branches(&stdout);
    summary.branches_seen = u32::try_from(branches.len()).map_or(u32::MAX, |count| count);

    let mut landed_branches: Vec<String> = Vec::new();

    for branch in branches {
        if remote_branch_is_merged(rig, &branch).await? {
            // Already merged — just delete the remote branch
            let remote_ref = branch.strip_prefix("origin/").unwrap_or(&branch);
            let _ = git_status(rig, &["push", "origin", "--delete", remote_ref]).await;
            continue;
        }

        match git_status(rig, &["merge", "--no-edit", &branch]).await {
            Ok(()) => {
                info!("{}: landed {} into main", rig.name, branch);
                landed_branches.push(branch);
                summary.branches_landed += 1;
            }
            Err(error) => {
                warn!("{}: could not land {}: {}", rig.name, branch, error);
                abort_merge(rig).await;
                summary.branches_failed += 1;
            }
        }
    }

    if !landed_branches.is_empty() {
        git_status(rig, &["push", "origin", "main"]).await?;
        // Delete landed remote branches to keep remote clean
        for branch in &landed_branches {
            let remote_ref = branch.strip_prefix("origin/").unwrap_or(branch);
            let _ = git_status(rig, &["push", "origin", "--delete", remote_ref]).await;
        }
        info!(
            "{}: cleaned up {} remote branches",
            rig.name,
            landed_branches.len()
        );
    }

    Ok(summary)
}

async fn repo_has_local_changes(rig: RigRepo) -> Result<bool, FleetError> {
    let output = git_output(rig, &["status", "--porcelain"]).await?;
    Ok(!output.stdout.is_empty())
}

async fn remote_branch_is_merged(rig: RigRepo, branch: &str) -> Result<bool, FleetError> {
    let status = Command::new("git")
        .args(["merge-base", "--is-ancestor", branch, "main"])
        .current_dir(rig.src_dir)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await
        .map_err(|error| FleetError::Git(error.to_string()))?;

    Ok(status.success())
}

async fn abort_merge(rig: RigRepo) {
    let _ = Command::new("git")
        .args(["merge", "--abort"])
        .current_dir(rig.src_dir)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await;
}

async fn git_status(rig: RigRepo, args: &[&str]) -> Result<(), FleetError> {
    let output = git_output(rig, args).await?;

    if output.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    Err(FleetError::Git(format!(
        "{}: git {:?} failed: {}",
        rig.name,
        args,
        stderr.trim()
    )))
}

async fn git_output(rig: RigRepo, args: &[&str]) -> Result<std::process::Output, FleetError> {
    Command::new("git")
        .args(args)
        .current_dir(rig.src_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .map_err(|error| FleetError::Git(error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::candidate_remote_branches;

    #[test]
    fn candidate_remote_branches_excludes_main_and_head() {
        let branches = candidate_remote_branches(
            "origin/HEAD\norigin/main\norigin/polecat/fury\norigin/feature/x\n",
        );

        assert_eq!(branches, vec!["origin/polecat/fury", "origin/feature/x"]);
    }

    #[test]
    fn candidate_remote_branches_ignores_non_origin_and_symbolic_refs() {
        let branches = candidate_remote_branches(
            "upstream/main\norigin/HEAD -> origin/main\norigin/ghoul-fix\n",
        );

        assert_eq!(branches, vec!["origin/ghoul-fix"]);
    }
}
