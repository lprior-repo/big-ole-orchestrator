use crate::data::{FleetError, Rig};
use std::path::Path;
use std::process::Stdio;
use tokio::process::Command;
use tracing::{info, warn};

#[derive(Debug, Default)]
pub struct BranchLandingSummary {
    pub repos_scanned: u32,
    pub branches_seen: u32,
    pub branches_landed: u32,
    pub branches_auto_resolved: u32,
    pub branches_escalated: u32,
    pub branches_failed: u32,
    pub repos_skipped: u32,
}

impl BranchLandingSummary {
    pub const fn add(&mut self, other: &Self) {
        self.repos_scanned += other.repos_scanned;
        self.branches_seen += other.branches_seen;
        self.branches_landed += other.branches_landed;
        self.branches_auto_resolved += other.branches_auto_resolved;
        self.branches_escalated += other.branches_escalated;
        self.branches_failed += other.branches_failed;
        self.repos_skipped += other.repos_skipped;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ConflictOutcome {
    AutoResolved,
    Escalated,
    Aborted,
}

pub async fn land_branches_for_all_rigs() -> BranchLandingSummary {
    let mut summary = BranchLandingSummary::default();

    for rig in Rig::all() {
        match land_branches_for_rig(rig).await {
            Ok(rig_summary) => summary.add(&rig_summary),
            Err(error) => {
                warn!("{}: branch landing failed: {}", rig.name, error);
                summary.repos_scanned += 1;
                summary.branches_failed += 1;
            }
        }
    }

    info!(
        "branch landing done: repos={} skipped={} seen={} landed={} auto_resolved={} escalated={} failed={}",
        summary.repos_scanned,
        summary.repos_skipped,
        summary.branches_seen,
        summary.branches_landed,
        summary.branches_auto_resolved,
        summary.branches_escalated,
        summary.branches_failed,
    );

    summary
}

async fn land_branches_for_rig(rig: &'static Rig) -> Result<BranchLandingSummary, FleetError> {
    let mut summary = BranchLandingSummary {
        repos_scanned: 1,
        ..BranchLandingSummary::default()
    };
    let repo_path = Path::new(rig.src_dir);

    if !repo_path.exists() {
        warn!("{}: source dir does not exist, skipping", rig.name);
        summary.repos_skipped = 1;
        return Ok(summary);
    }

    if repo_is_dirty(repo_path).await? {
        warn!(
            "{}: skipping branch landing because source repo is dirty",
            rig.name
        );
        summary.repos_skipped = 1;
        return Ok(summary);
    }

    git_run(repo_path, &["fetch", "origin", "--prune"]).await?;
    git_run(repo_path, &["checkout", "-f", "main"]).await?;

    // ff-only pull is best-effort — main may have local-only commits
    if git_run(repo_path, &["pull", "--ff-only", "origin", "main"])
        .await
        .is_err()
    {
        warn!(
            "{}: main diverged from origin/main, resetting",
            rig.name
        );
        git_run(repo_path, &["reset", "--hard", "origin/main"]).await?;
    }

    let branches_output = git_output(
        repo_path,
        &[
            "for-each-ref",
            "--format=%(refname:short)",
            "refs/remotes/origin",
        ],
    )
    .await?;
    let stdout = String::from_utf8(branches_output.stdout)?;
    let candidate_branches = filter_candidate_branches(&stdout);
    summary.branches_seen = u32::try_from(candidate_branches.len()).unwrap_or(u32::MAX);

    let mut landed: Vec<String> = Vec::new();

    for branch in &candidate_branches {
        let remote_ref = format!("origin/{branch}");

        // Already merged — just delete the remote ref
        if branch_is_ancestor_of_main(repo_path, &remote_ref).await {
            let _ = git_run(repo_path, &["push", "origin", "--delete", branch]).await;
            continue;
        }

        // Step 1: Try normal merge
        match git_run(repo_path, &["merge", "--no-edit", &remote_ref]).await {
            Ok(()) => {
                info!("{}: landed {} into main", rig.name, branch);
                landed.push(branch.clone());
                summary.branches_landed += 1;
            }
            Err(_) => {
                // Step 2: Conflict — try resolution
                let outcome =
                    resolve_or_escalate(rig, repo_path, &remote_ref, branch).await;
                match outcome {
                    ConflictOutcome::AutoResolved => {
                        info!("{}: auto-resolved conflicts for {}", rig.name, branch);
                        landed.push(branch.clone());
                        summary.branches_auto_resolved += 1;
                    }
                    ConflictOutcome::Escalated => {
                        warn!("{}: escalated conflicts for {}", rig.name, branch);
                        summary.branches_escalated += 1;
                    }
                    ConflictOutcome::Aborted => {
                        warn!(
                            "{}: could not land {} — conflicts need manual resolution",
                            rig.name, branch
                        );
                        summary.branches_failed += 1;
                    }
                }
            }
        }
    }

    if !landed.is_empty() {
        git_run(repo_path, &["push", "origin", "main"]).await?;
        for branch in &landed {
            let _ = git_run(repo_path, &["push", "origin", "--delete", branch]).await;
        }
        info!(
            "{}: pushed main and cleaned up {} remote branches",
            rig.name,
            landed.len()
        );
    }

    Ok(summary)
}

/// Multi-step conflict resolution: try -X theirs, then per-file --theirs,
/// then escalate to AI via bd create.
async fn resolve_or_escalate(
    rig: &'static Rig,
    repo_path: &Path,
    remote_ref: &str,
    branch: &str,
) -> ConflictOutcome {
    // Capture conflicted files before aborting
    let files = unmerged_files(repo_path).await.unwrap_or_default();
    merge_abort(repo_path).await;

    // Strategy 1: merge -X theirs (auto-resolve all text conflicts with branch version)
    if git_run(
        repo_path,
        &["merge", "--no-edit", "-X", "theirs", remote_ref],
    )
    .await
    .is_ok()
    {
        return ConflictOutcome::AutoResolved;
    }

    // Strategy 2: per-file checkout --theirs for remaining conflicts
    let remaining = unmerged_files(repo_path).await.unwrap_or_default();
    if remaining.is_empty() {
        // Merge resolved after all
        return ConflictOutcome::AutoResolved;
    }

    let mut all_resolved = true;
    for file in &remaining {
        if checkout_file_theirs(repo_path, file).await.is_err() {
            all_resolved = false;
            break;
        }
    }

    if all_resolved
        && git_run(repo_path, &["commit", "--no-edit"])
            .await
            .is_ok()
    {
        info!(
            "{}: resolved {} conflicted files via --theirs for {}",
            rig.name,
            remaining.len(),
            branch
        );
        return ConflictOutcome::AutoResolved;
    }

    // Strategy 3: Could not auto-resolve — abort and escalate to AI
    merge_abort(repo_path).await;

    if let Err(e) = escalate_conflict(rig, branch, &files).await {
        warn!(
            "{}: failed to escalate conflict for {}: {}",
            rig.name, branch, e
        );
        return ConflictOutcome::Aborted;
    }

    ConflictOutcome::Escalated
}

async fn repo_is_dirty(repo_path: &Path) -> Result<bool, FleetError> {
    let output = git_output(repo_path, &["status", "--porcelain"]).await?;
    Ok(!output.stdout.is_empty())
}

async fn branch_is_ancestor_of_main(repo_path: &Path, branch: &str) -> bool {
    let status = Command::new("git")
        .args(["merge-base", "--is-ancestor", branch, "main"])
        .current_dir(repo_path)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await;

    status.is_ok_and(|s| s.success())
}

async fn unmerged_files(repo_path: &Path) -> Result<Vec<String>, FleetError> {
    let output = Command::new("git")
        .args(["diff", "--name-only", "--diff-filter=U"])
        .current_dir(repo_path)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .await
        .map_err(|e| FleetError::Git(e.to_string()))?;

    let stdout = String::from_utf8(output.stdout)?;
    Ok(stdout
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(String::from)
        .collect())
}

async fn checkout_file_theirs(repo_path: &Path, file: &str) -> Result<(), FleetError> {
    let checkout = Command::new("git")
        .args(["checkout", "--theirs", file])
        .current_dir(repo_path)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await
        .map_err(|e| FleetError::Git(e.to_string()))?;

    if !checkout.success() {
        return Err(FleetError::Git(format!(
            "git checkout --theirs {file} failed"
        )));
    }

    let add = Command::new("git")
        .args(["add", file])
        .current_dir(repo_path)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await
        .map_err(|e| FleetError::Git(e.to_string()))?;

    if !add.success() {
        return Err(FleetError::Git(format!(
            "git add {file} failed after --theirs checkout"
        )));
    }

    Ok(())
}

async fn merge_abort(repo_path: &Path) {
    let _ = Command::new("git")
        .args(["merge", "--abort"])
        .current_dir(repo_path)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await;
}

async fn escalate_conflict(
    rig: &'static Rig,
    branch: &str,
    conflicted_files: &[String],
) -> Result<(), FleetError> {
    let files_list = conflicted_files.join(", ");
    let title = format!("MERGE-CONFLICT: {branch} -> main in {}", rig.name);
    let description = format!(
        "Branch `{branch}` has merge conflicts with `main` in `{rig_name}` that could not be auto-resolved.\n\n\
         **Conflicted files:**\n{files_list}\n\n\
         **Resolution steps:**\n\
         1. Checkout the branch: `git checkout {branch}`\n\
         2. Rebase onto main: `git rebase main`\n\
         3. Resolve conflicts manually\n\
         4. Push: `git push origin {branch} --force-with-lease`\n\
         5. Fleet-feed will land it on the next cycle\n\n\
         **Source:** fleet-feed branch landing auto-escalation",
        branch = branch,
        rig_name = rig.name,
        files_list = files_list,
    );

    let output = Command::new("bd")
        .args([
            "create",
            &title,
            "--description",
            &description,
            "-t",
            "bug",
            "-p",
            "1",
        ])
        .current_dir(rig.src_dir)
        .env("BD_DOLT_AUTO_START", "false")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .map_err(|e| FleetError::Bd(e.to_string()))?;

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        info!(
            "{}: created escalation bead for merge conflict on {}: {}",
            rig.name, branch, stdout.trim()
        );
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(FleetError::Bd(format!(
            "bd create escalation failed: {}",
            stderr.trim()
        )))
    }
}

fn filter_candidate_branches(stdout: &str) -> Vec<String> {
    stdout
        .lines()
        .map(str::trim)
        .filter(|branch| branch.starts_with("origin/"))
        .filter(|branch| *branch != "origin/main" && *branch != "origin/HEAD")
        .filter(|branch| !branch.contains(" -> "))
        .map(ToString::to_string)
        .collect()
}

async fn git_run(repo_path: &Path, args: &[&str]) -> Result<(), FleetError> {
    let output = git_output(repo_path, args).await?;

    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(FleetError::Git(format!(
            "git {:?} failed: {}",
            args,
            stderr.trim()
        )))
    }
}

async fn git_output(
    repo_path: &Path,
    args: &[&str],
) -> Result<std::process::Output, FleetError> {
    Command::new("git")
        .args(args)
        .current_dir(repo_path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .map_err(|error| FleetError::Git(error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::filter_candidate_branches;

    #[test]
    fn filter_excludes_main_and_head() {
        let branches = filter_candidate_branches(
            "origin/HEAD\norigin/main\norigin/polecat/fury\norigin/feature/x\n",
        );

        assert_eq!(
            branches,
            vec!["origin/polecat/fury", "origin/feature/x"]
        );
    }

    #[test]
    fn filter_ignores_non_origin_and_symbolic_refs() {
        let branches = filter_candidate_branches(
            "upstream/main\norigin/HEAD -> origin/main\norigin/ghoul-fix\n",
        );

        assert_eq!(branches, vec!["origin/ghoul-fix"]);
    }
}
