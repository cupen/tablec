//! Git-backed diff support for the webui.
//!
//! The webui answers "what changed in the spreadsheets" the way `git diff`
//! does, between two comparison sides: a left side that is either the
//! working tree (`worktree`) or any resolvable ref, and a right side that is
//! any resolvable ref. The defaults — `worktree` vs `HEAD` — reproduce the
//! classic working-tree-vs-HEAD view; any other pair compares branch vs
//! branch (or tag/commit) content directly at each ref's tip. We shell out
//! to the `git` binary (plumbing commands) rather than linking libgit2,
//! which keeps the dependency surface flat — the baseline is a git repo by
//! definition, so `git` is a hard requirement of the feature, and the
//! "not a repo / no git" cases degrade to clean-with-warning instead of
//! failing the file list. An unresolvable ref is the one request-level
//! error: it names the ref and never silently degrades to clean.
//!
//! Everything in here is read-only: we never stage, commit, or otherwise
//! mutate the repository.

use std::path::{Path, PathBuf};
use std::process::Command;

/// Machine-stable status of a spreadsheet against HEAD, mirroring what
/// `git status --porcelain` tells us when reduced to the shapes the UI
/// cares about.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FileStatus {
    /// Tracked, differs from HEAD.
    Modified,
    /// Staged in the index (differs from both HEAD and worktree).
    Added,
    /// Present but not tracked by git yet.
    Untracked,
    /// Tracked at HEAD but missing in the working tree.
    Deleted,
    /// No changes (tracked and identical to HEAD).
    Clean,
}

impl FileStatus {
    /// Whether this status should count as "changed" for the UI filter.
    pub fn is_changed(self) -> bool {
        !matches!(self, FileStatus::Clean)
    }
}

impl Default for FileStatus {
    fn default() -> Self {
        FileStatus::Clean
    }
}

/// Per-file diff summary consumed by `/api/files`.
#[derive(Debug, Clone, serde::Serialize)]
pub struct FileWithStatus {
    pub path: String,
    pub status: FileStatus,
    /// `git diff --numstat` insertions for `modified` files (0 otherwise).
    #[serde(default)]
    pub numstat_added: u64,
    /// `git diff --numstat` deletions for `modified` files (0 otherwise).
    #[serde(default)]
    pub numstat_deleted: u64,
}

/// Per-cell diff status for the parsed preview grid.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum CellDiff {
    /// Cell/row exists only in the working tree.
    Added,
    /// Cell/row exists only at HEAD.
    Deleted,
    /// Cell exists in both but the parsed value differs.
    Modified,
    /// No change.
    Unchanged,
}

/// Outcome of resolving the git baseline. `None` means "no usable baseline"
/// and callers fall back to clean/no-diff.
#[derive(Debug, Clone)]
pub struct Baseline {
    /// Repository root (the directory `.git` lives in / is for).
    pub repo_root: PathBuf,
}

/// A git command failed in a way that means "we have no baseline" rather
/// than "the request is broken" — the caller should degrade to clean.
/// [`GitError::UnknownRef`] is the exception: it is a request-level error
/// (the caller answers 400 naming the ref), never a degrade-to-clean case.
#[derive(Debug, thiserror::Error)]
pub enum GitError {
    #[error("git is not available: {0}")]
    MissingBinary(#[from] std::io::Error),
    #[error("git command failed: {0}")]
    Failed(String),
    #[error("unknown ref: {0}")]
    UnknownRef(String),
}

// -----------------------------------------------------------------------------
// Comparison sides — the generalization of the fixed worktree-vs-HEAD pair.
// -----------------------------------------------------------------------------

/// One side of a comparison. `Worktree` (the live working tree) is only
/// meaningful on the left; the right side is always a ref.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Side {
    Worktree,
    Ref(String),
}

impl Side {
    /// Parse a query-parameter side name. `worktree` is the keyword for the
    /// working tree; anything else is a ref name resolved through git later.
    pub fn parse(s: &str) -> Side {
        let t = s.trim();
        if t.eq_ignore_ascii_case("worktree") {
            Side::Worktree
        } else {
            Side::Ref(t.to_string())
        }
    }

    pub fn is_worktree(&self) -> bool {
        matches!(self, Side::Worktree)
    }
}

/// Resolve a side's ref to a commit sha via `rev-parse --verify`. `Worktree`
/// needs no resolution (`Ok(None)`). An unresolvable ref is
/// [`GitError::UnknownRef`] naming the ref.
pub fn resolve_side(baseline: &Baseline, side: &Side) -> Result<Option<String>, GitError> {
    let Side::Ref(r) = side else {
        return Ok(None);
    };
    if r.is_empty() {
        return Err(GitError::UnknownRef("(empty ref)".to_string()));
    }
    let spec = format!("{r}^{{commit}}");
    match run_git(&baseline.repo_root, &["rev-parse", "--verify", &spec]) {
        Ok(out) => {
            let sha = out.trim().to_string();
            if sha.is_empty() {
                Err(GitError::UnknownRef(r.clone()))
            } else {
                Ok(Some(sha))
            }
        }
        // rev-parse failure on a syntactically fine ref = it doesn't resolve.
        Err(GitError::Failed(_)) => Err(GitError::UnknownRef(r.clone())),
        Err(e) => Err(e),
    }
}

/// Branch listing for the comparison selector: local and remote-tracking
/// branches, deduplicated by short name with the local branch winning.
/// Never errors — no repo / no git degrades to an empty list + warning.
#[derive(Debug, Default, serde::Serialize)]
pub struct BranchListing {
    pub branches: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub warning: Option<String>,
}

pub fn branches(dir: &Path) -> BranchListing {
    let Some(baseline) = resolve_baseline(dir).unwrap_or(None) else {
        return BranchListing {
            branches: vec![],
            warning: Some("not inside a git repository".to_string()),
        };
    };
    // Two passes so a local branch always claims its short name before any
    // remote-tracking twin (`main` wins over `origin/main`). The
    // `refs/remotes/*/HEAD` symrefs are skipped — `%(refname:short)` renders
    // them as just the remote name ("origin"), which is not a branch.
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut names: Vec<String> = Vec::new();
    for scope in ["refs/heads", "refs/remotes"] {
        let out = match run_git(
            &baseline.repo_root,
            &["for-each-ref", "--format=%(refname)\t%(refname:short)", scope],
        ) {
            Ok(out) => out,
            Err(_) => {
                return BranchListing {
                    branches: vec![],
                    warning: Some("git failed to list branches".to_string()),
                }
            }
        };
        for line in out.lines().map(str::trim).filter(|l| !l.is_empty()) {
            let Some((full, short)) = line.split_once('\t') else {
                continue;
            };
            if full.ends_with("/HEAD") {
                continue;
            }
            let key = short.rsplit('/').next().unwrap_or(short).to_string();
            if seen.insert(key) {
                names.push(short.to_string());
            }
        }
    }
    names.sort();
    BranchListing {
        branches: names,
        warning: None,
    }
}

/// Locate the repository containing `dir` that has a resolvable HEAD.
///
/// Returns `None` (no baseline) when the directory is not inside a repo or
/// the repo has no commits yet. Errors are treated as "no baseline" too:
/// a corrupt or unreadable repo must not take the webui down.
pub fn resolve_baseline(dir: &Path) -> Result<Option<Baseline>, GitError> {
    // `rev-parse --show-toplevel` prints the repo root when `dir` is inside
    // one and the repo has at least one commit; otherwise it fails (exit 128,
    // "not a git repository" / "no commits yet"). Both cases mean "no
    // baseline" — the webui must degrade to clean, not error.
    match run_git(dir, &["rev-parse", "--show-toplevel"]) {
        Ok(out) => {
            let toplevel = out
                .lines()
                .next()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(PathBuf::from);
            Ok(toplevel.map(|repo_root| Baseline { repo_root }))
        }
        Err(GitError::Failed(_)) => Ok(None),
        Err(e) => Err(e),
    }
}

/// Run `git <args>` with `cwd` as the working directory. Returns stdout.
///
/// Runs with `LC_ALL=C` + `--no-optional-locks` and, for diff-y commands,
/// the caller passes `--no-color` to keep output machine-stable.
fn run_git(cwd: &Path, args: &[&str]) -> Result<String, GitError> {
    let mut cmd = Command::new("git");
    cmd.arg("--no-optional-locks")
        .args(args)
        .current_dir(cwd)
        .env("LC_ALL", "C");
    let out = cmd.output().map_err(GitError::MissingBinary)?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        return Err(GitError::Failed(format!(
            "git {} failed: {}",
            args.join(" "),
            stderr.trim()
        )));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Classify each file under `dir` against the repository baseline.
///
/// For every spreadsheet path under `dir`, produces the porcelain status and
/// (for `modified` files) the `--numstat` line counts. Files not inside the
/// repository are reported `clean` (no baseline to diff against).
pub fn file_statuses(dir: &Path, files: &[PathBuf]) -> Result<Vec<FileWithStatus>, GitError> {
    let Some(baseline) = resolve_baseline(dir)? else {
        // No repo / no HEAD: nothing has a baseline, so everything is clean.
        return Ok(files
            .iter()
            .map(|p| FileWithStatus {
                path: p.display().to_string(),
                status: FileStatus::Clean,
                numstat_added: 0,
                numstat_deleted: 0,
            })
            .collect());
    };

    // `status --porcelain` doesn't take a pathspec list directly for the
    // "only these files" case without prefix magic; the simplest stable
    // approach is one sweep of the whole repo, then map by relpath. The
    // repo's file set is bounded by the working tree, so this stays cheap.
    let porcelain = run_git(&baseline.repo_root, &["status", "--porcelain"])?;
    // Map relpath (with quoting stripped by `-z` handling below) → XY code.
    let mut codes: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    for line in porcelain.lines() {
        if line.len() < 4 {
            continue;
        }
        let (xy, path) = (&line[..2], line[3..].trim());
        let path = unquote_git_path(path);
        codes.insert(path, xy.to_string());
    }

    // Numeric insertions/deletions for modified files.
    let mut numstat: std::collections::HashMap<String, (u64, u64)> =
        std::collections::HashMap::new();
    let numstat_out = run_git(
        &baseline.repo_root,
        &["diff", "HEAD", "--numstat", "--no-color"],
    )?;
    for line in numstat_out.lines() {
        let mut it = line.splitn(3, '\t');
        let (Some(a), Some(d), Some(path)) = (it.next(), it.next(), it.next()) else {
            continue;
        };
        let path = unquote_git_path(path.trim());
        let (Ok(a), Ok(d)) = (a.parse::<u64>(), d.parse::<u64>()) else {
            continue;
        };
        numstat.insert(path, (a, d));
    }

    // Working-tree relative paths for status mapping. All prefix math happens
    // over plain, `/`-separated strings: the webui's scan root can be a
    // Windows verbatim path (`\\?\C:\...`, as `Path::canonicalize` yields)
    // while `git rev-parse` prints plain forward-slash paths — component-wise
    // `Path::strip_prefix` never matches across that divide, and every status
    // would silently degrade to `clean`.
    let repo_root_s = plain(&baseline.repo_root);
    let dir_s = plain(dir);
    let dir_rel = dir_s
        .strip_prefix(&repo_root_s)
        .map(|r| r.trim_start_matches('/').to_string())
        .unwrap_or_default();

    let mk = |p: &PathBuf| {
        let abs_s = if p.is_absolute() {
            plain(p)
        } else {
            format!("{repo_root_s}/{}", plain(p))
        };
        let rel = abs_s
            .strip_prefix(&repo_root_s)
            .map(|r| r.trim_start_matches('/').to_string())
            .unwrap_or_default();
        let status = match codes.get(&rel).map(String::as_str) {
            Some("??") => FileStatus::Untracked,
            // Porcelain XY codes: X = index status, Y = worktree status, and
            // either slot may hold 'D'/'A' (e.g. " D" unstaged, "D " staged,
            // "DD" both). `contains` covers all three.
            Some(xy) if xy.contains('D') => FileStatus::Deleted,
            Some(xy) if xy.contains('A') => FileStatus::Added,
            Some(_) => FileStatus::Modified,
            None => FileStatus::Clean,
        };
        let (numstat_added, numstat_deleted) = numstat.get(&rel).copied().unwrap_or((0, 0));
        FileWithStatus {
            path: p.display().to_string(),
            status,
            numstat_added,
            numstat_deleted,
        }
    };

    // `deleted` files may be missing from the scan list (`/api/files` only
    // lists existing files). We still surface them under the modified filter
    // per spec: walk the porcelain codes for `D` entries and append synthetic
    // entries for paths inside the scanned directory that weren't in `files`.
    let in_dir =
        |rel: &str| dir_rel.is_empty() || rel == dir_rel || rel.starts_with(&format!("{dir_rel}/"));
    let mut out: Vec<FileWithStatus> = files.iter().map(mk).collect();
    for (rel, xy) in &codes {
        if !(xy.contains('D') && !xy.contains('A')) || !in_dir(rel) {
            continue;
        }
        let full_s = format!("{repo_root_s}/{rel}");
        if out.iter().any(|f| {
            let existing = plain(Path::new(&f.path));
            existing == full_s || existing == format!("{dir_s}/{rel}")
        }) {
            continue;
        }
        out.push(FileWithStatus {
            path: full_s,
            status: FileStatus::Deleted,
            numstat_added: 0,
            numstat_deleted: 0,
        });
    }
    out.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(out)
}

/// Read the HEAD blob for `path` (relative to the repo root) as bytes.
pub fn head_blob(baseline: &Baseline, repo_rel: &Path) -> Result<Vec<u8>, GitError> {
    ref_blob(baseline, "HEAD", repo_rel)
}

/// Read the blob `path` (relative to the repo root) at `ref_spec` (a sha or
/// anything `git show` accepts as `<rev>:<path>`'s rev part) as bytes.
/// Failing means "no such path at that rev" — callers decide whether that is
/// an expected condition (untracked / absent on a side).
pub fn ref_blob(baseline: &Baseline, ref_spec: &str, repo_rel: &Path) -> Result<Vec<u8>, GitError> {
    let spec = format!(
        "{}:{}",
        ref_spec,
        repo_rel.display().to_string().replace('\\', "/")
    );
    let out = Command::new("git")
        .arg("--no-optional-locks")
        .args(["show", &spec])
        .current_dir(&baseline.repo_root)
        .output()
        .map_err(GitError::MissingBinary)?;
    if !out.status.success() {
        return Err(GitError::Failed(format!(
            "git show {spec} failed (absent at {ref_spec})"
        )));
    }
    Ok(out.stdout)
}

// -----------------------------------------------------------------------------
// Generic two-side file comparison (non-default sides). The default
// worktree-vs-HEAD pair keeps using [`file_statuses`] above (porcelain's
// staged/untracked distinction has no equivalent here); every other pair —
// worktree vs ref, ref vs ref — classifies the union of both sides' file
// sets, reading each side as "left's change relative to right (the
// baseline)": only-left `added`, only-right `deleted`, differing `modified`.
// -----------------------------------------------------------------------------

/// Normalize a path for repo-relative prefix math: `/` separators, no
/// Windows verbatim (`\\?\` / `//?/`) prefix. `git` prints plain
/// forward-slash paths while the scanner may hand us verbatim ones.
fn plain(p: &Path) -> String {
    let s = p.to_string_lossy().replace('\\', "/");
    s.strip_prefix("//?/").unwrap_or(&s).to_string()
}

/// Repo-root-relative, `/`-separated path of `p` inside `baseline` (empty
/// when `p` is not under the repo root).
fn repo_rel_of(p: &Path, baseline: &Baseline) -> String {
    let root_s = plain(&baseline.repo_root);
    let p_s = if p.is_absolute() {
        plain(p)
    } else {
        format!("{root_s}/{}", plain(p))
    };
    p_s.strip_prefix(&root_s)
        .map(|r| r.trim_start_matches('/').to_string())
        .unwrap_or_default()
}

/// Spreadsheet rel-paths (repo-root relative) present at `sha`'s tree under
/// `dir_rel` ("" = whole repo). `-z` output is never C-quoted, so paths pass
/// through as-is; the extension filter mirrors the worktree scanner
/// (`.xlsx` / `.xls` only).
fn ls_tree(baseline: &Baseline, sha: &str, dir_rel: &str) -> Result<Vec<String>, GitError> {
    let pathspec = if dir_rel.is_empty() {
        ".".to_string()
    } else {
        format!("{dir_rel}/")
    };
    let out = run_git(
        &baseline.repo_root,
        &["ls-tree", "-r", "--name-only", "-z", sha, "--", &pathspec],
    )?;
    Ok(out
        .split('\0')
        .filter(|l| !l.is_empty())
        .filter(|rel| {
            Path::new(rel)
                .extension()
                .is_some_and(|e| e == "xlsx" || e == "xls")
        })
        .map(str::to_string)
        .collect())
}

/// Apply the config's include/exclude globs to a repo-relative path, with
/// the same semantics [`tablec_core::core::config::find_excel_files`] applies
/// to the worktree scan: patterns are relative to the input dir, a `**/`
/// prefix widens them to any depth.
fn matches_input_globs(rel_to_input: &str, include: &[String], exclude: &[String]) -> bool {
    let mk = |p: &str| {
        let pat = p.strip_prefix("**/").unwrap_or(p);
        glob::Pattern::new(pat).ok()
    };
    let includes: Vec<_> = if include.is_empty() {
        vec![mk("*.xlsx")]
    } else {
        include.iter().map(|p| mk(p)).collect()
    };
    if !includes
        .iter()
        .flatten()
        .any(|pat| pat.matches(rel_to_input))
    {
        return false;
    }
    !exclude
        .iter()
        .filter_map(|p| mk(p))
        .any(|pat| pat.matches(rel_to_input))
}

/// Classify spreadsheets between two comparison sides (see the module-level
/// comment for direction semantics). `worktree_files` is the scanner's
/// output — the left-side file set whenever the left side is `worktree`.
/// Refs are resolved here; [`GitError::UnknownRef`] propagates so callers
/// can reject the request. Non-ref failures degrade exactly like
/// [`file_statuses`]: all clean.
pub fn file_statuses_sides(
    input_dir: &Path,
    worktree_files: &[PathBuf],
    left: &Side,
    right: &Side,
    include: &[String],
    exclude: &[String],
) -> Result<Vec<FileWithStatus>, GitError> {
    if right.is_worktree() {
        // Enforced at the HTTP layer; the library repeats it defensively.
        return Err(GitError::Failed(
            "'worktree' is only valid as the left side".to_string(),
        ));
    }
    let Some(baseline) = resolve_baseline(input_dir)? else {
        return Ok(worktree_files
            .iter()
            .map(|p| FileWithStatus {
                path: p.display().to_string(),
                status: FileStatus::Clean,
                numstat_added: 0,
                numstat_deleted: 0,
            })
            .collect());
    };
    let left_sha = resolve_side(&baseline, left)?;
    let right_sha =
        resolve_side(&baseline, right)?.expect("right is never Worktree (checked above)");

    let dir_rel = {
        let d = repo_rel_of(input_dir, &baseline);
        d.trim_end_matches('/').to_string()
    };

    // Left file set: scanner paths (worktree) or the ref's tree.
    let left_files: Vec<(String, String)> = match left {
        Side::Worktree => worktree_files
            .iter()
            .map(|p| (repo_rel_of(p, &baseline), p.display().to_string()))
            .collect(),
        Side::Ref(_) => ls_tree(&baseline, left_sha.as_deref().unwrap_or(""), &dir_rel)?
            .into_iter()
            .map(|rel| {
                let abs = baseline.repo_root.join(&rel);
                (rel, abs.display().to_string())
            })
            .collect(),
    };
    let right_files: Vec<String> = ls_tree(&baseline, &right_sha, &dir_rel)?;

    // Dir-relative form for the config globs (globs are input-dir-relative).
    let to_input_rel = |repo_rel: &str| -> String {
        if dir_rel.is_empty() {
            repo_rel.to_string()
        } else {
            repo_rel
                .strip_prefix(&format!("{dir_rel}/"))
                .unwrap_or("")
                .to_string()
        }
    };
    let left_set: std::collections::BTreeMap<String, String> = left_files
        .into_iter()
        .filter(|(rel, _)| !rel.is_empty() && matches_input_globs(&to_input_rel(rel), include, exclude))
        .collect();
    let right_set: std::collections::BTreeSet<String> = right_files
        .into_iter()
        .filter(|rel| matches_input_globs(&to_input_rel(&rel), include, exclude))
        .collect();

    // One diff between the sides answers both "does it differ" and the
    // numstat counts. Worktree-left compares <right> → working tree; ref-left
    // compares <right> → <left> — either way letters/counts read as the left
    // side's change relative to the right.
    let mut diff_args: Vec<String> = vec!["diff".into(), "--no-color".into()];
    match left {
        Side::Worktree => diff_args.push(right_sha.clone()),
        Side::Ref(_) => {
            diff_args.push(right_sha.clone());
            diff_args.push(left_sha.clone().unwrap_or_default());
        }
    }
    let pathspec = if dir_rel.is_empty() {
        ".".to_string()
    } else {
        format!("{dir_rel}/")
    };
    let name_status_args = {
        let mut a = diff_args.clone();
        a.extend(["--name-status".into(), "-z".into()]);
        a.push("--".into());
        a.push(pathspec.clone());
        a
    };
    let refs_ns: Vec<&str> = name_status_args.iter().map(String::as_str).collect();
    let ns_out = run_git(&baseline.repo_root, &refs_ns)?;
    let mut differ: std::collections::HashMap<String, ()> = std::collections::HashMap::new();
    {
        // -z name-status: `<status>\0<path>\0` pairs.
        let mut it = ns_out.split('\0');
        while let (Some(_st), Some(path)) = (it.next(), it.next()) {
            if !path.is_empty() {
                differ.insert(path.to_string(), ());
            }
        }
    }
    let numstat_args = {
        let mut a = diff_args.clone();
        a.extend(["--numstat".into(), "-z".into()]);
        a.push("--".into());
        a.push(pathspec.clone());
        a
    };
    let refs_nu: Vec<&str> = numstat_args.iter().map(String::as_str).collect();
    let nu_out = run_git(&baseline.repo_root, &refs_nu)?;
    let mut numstat: std::collections::HashMap<String, (u64, u64)> =
        std::collections::HashMap::new();
    for entry in nu_out.split('\0') {
        // -z numstat: `<added>\t<deleted>\t<path>` per entry; binary files
        // report `-` counts and fall back to (0, 0) like the default path.
        let mut it = entry.splitn(3, '\t');
        let (Some(a), Some(d), Some(path)) = (it.next(), it.next(), it.next()) else {
            continue;
        };
        let (Ok(a), Ok(d)) = (a.parse::<u64>(), d.parse::<u64>()) else {
            continue;
        };
        numstat.insert(path.to_string(), (a, d));
    }

    let mut out: Vec<FileWithStatus> = Vec::new();
    let mut seen: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    for (rel, display) in &left_set {
        seen.insert(rel.clone());
        let (status, a, d) = if right_set.contains(rel) {
            if differ.contains_key(rel) {
                let (a, d) = numstat.get(rel).copied().unwrap_or((0, 0));
                (FileStatus::Modified, a, d)
            } else {
                (FileStatus::Clean, 0, 0)
            }
        } else {
            (FileStatus::Added, 0, 0)
        };
        out.push(FileWithStatus {
            path: display.clone(),
            status,
            numstat_added: a,
            numstat_deleted: d,
        });
    }
    for rel in &right_set {
        if !seen.insert(rel.clone()) {
            continue;
        }
        // Present only on the right: the left side "deleted" it.
        out.push(FileWithStatus {
            path: baseline.repo_root.join(rel).display().to_string(),
            status: FileStatus::Deleted,
            numstat_added: 0,
            numstat_deleted: 0,
        });
    }
    out.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(out)
}

/// Best-effort unquote of a porcelain path. Git quotes paths with unusual
/// characters using C-style quoting; the common case (spreadsheet paths with
/// spaces) is emitted with no quotes when `core.quotepath` is off, but we
/// can't rely on user config, so handle the quoted form minimally.
fn unquote_git_path(p: &str) -> String {
    if p.as_bytes().first() == Some(&b'"') && p.as_bytes().last() == Some(&b'"') {
        // Decode the most common escapes; a full C-unquote is overkill here.
        let inner = &p[1..p.len() - 1];
        let mut out = String::with_capacity(inner.len());
        let mut chars = inner.chars();
        while let Some(c) = chars.next() {
            if c == '\\' {
                match chars.next() {
                    Some('n') => out.push('\n'),
                    Some('t') => out.push('\t'),
                    Some('"') => out.push('"'),
                    Some('\\') => out.push('\\'),
                    Some(oct @ '0'..='7') => {
                        // \NNN octal byte.
                        let mut n = oct.to_digit(8).unwrap_or(0);
                        for _ in 0..2 {
                            if let Some(d) = chars.next().and_then(|c| c.to_digit(8)) {
                                n = n * 8 + d;
                            }
                        }
                        if let Some(ch) = char::from_u32(n) {
                            out.push(ch);
                        }
                    }
                    _ => {}
                }
            } else {
                out.push(c);
            }
        }
        out
    } else {
        p.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::process::Command;

    fn have_git() -> bool {
        Command::new("git")
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    /// Create a throwaway git repo in a tempdir with one committed file
    /// (a text file named like a spreadsheet, or an xlsx if `write_xlsx`).
    fn temp_repo() -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().to_path_buf();
        let _ = Command::new("git")
            .args(["init", "-q", "-b", "main"])
            .current_dir(&root)
            .output()
            .unwrap();
        // identity for the synthetic commit
        let _ = Command::new("git")
            .args(["config", "user.email", "t@example.com"])
            .current_dir(&root)
            .output()
            .unwrap();
        let _ = Command::new("git")
            .args(["config", "user.name", "Test"])
            .current_dir(&root)
            .output()
            .unwrap();
        (dir, root)
    }

    fn commit_all(root: &Path) {
        let _ = Command::new("git")
            .args(["add", "-A"])
            .current_dir(root)
            .output()
            .unwrap();
        let _ = Command::new("git")
            .args(["commit", "-q", "-m", "init"])
            .current_dir(root)
            .output()
            .unwrap();
    }

    #[test]
    fn resolve_baseline_outside_repo_is_none() {
        if !have_git() {
            return;
        }
        let tmp = tempfile::tempdir().unwrap();
        let b = resolve_baseline(tmp.path()).unwrap();
        assert!(b.is_none());
    }

    #[test]
    fn resolve_baseline_inside_repo_finds_root() {
        if !have_git() {
            return;
        }
        let (dir, root) = temp_repo();
        fs::write(root.join("a.xlsx"), "x").unwrap();
        commit_all(&root);
        let sub = root.join("data");
        fs::create_dir_all(&sub).unwrap();
        let b = resolve_baseline(&sub).unwrap();
        assert!(b.is_some());
        assert_eq!(b.unwrap().repo_root, root);
        let _ = dir;
    }

    #[test]
    fn file_statuses_untracked_and_clean() {
        if !have_git() {
            return;
        }
        let (dir, root) = temp_repo();
        fs::write(root.join("tracked.xlsx"), "v1").unwrap();
        commit_all(&root);
        fs::write(root.join("untracked.xlsx"), "v2").unwrap();
        let files = vec![
            root.join("tracked.xlsx"),
            root.join("untracked.xlsx"),
            root.join("nope.csv"),
        ];
        let st = file_statuses(&root, &files).unwrap();
        let by: std::collections::HashMap<String, FileStatus> = st
            .iter()
            .map(|f| {
                (
                    Path::new(&f.path)
                        .file_name()
                        .unwrap()
                        .to_string_lossy()
                        .into_owned(),
                    f.status,
                )
            })
            .collect();
        assert_eq!(by.get("tracked.xlsx"), Some(&FileStatus::Clean));
        assert_eq!(by.get("untracked.xlsx"), Some(&FileStatus::Untracked));
        // A file not in the repo at all is reported clean (no baseline).
        assert_eq!(by.get("nope.csv"), Some(&FileStatus::Clean));
        let _ = dir;
    }

    #[test]
    fn file_statuses_modified_with_numstat() {
        if !have_git() {
            return;
        }
        let (dir, root) = temp_repo();
        fs::write(root.join("data.xlsx"), "line1\nline2\n").unwrap();
        commit_all(&root);
        fs::write(root.join("data.xlsx"), "line1\nline2\nline3\n").unwrap();
        let files = vec![root.join("data.xlsx")];
        let st = file_statuses(&root, &files).unwrap();
        let f = st.iter().find(|f| f.path.ends_with("data.xlsx")).unwrap();
        assert_eq!(f.status, FileStatus::Modified);
        assert_eq!(f.numstat_added, 1);
        assert_eq!(f.numstat_deleted, 0);
        let _ = dir;
    }

    #[test]
    fn file_statuses_reports_deleted_files() {
        if !have_git() {
            return;
        }
        let (dir, root) = temp_repo();
        fs::write(root.join("gone.xlsx"), "v").unwrap();
        commit_all(&root);
        fs::remove_file(root.join("gone.xlsx")).unwrap();
        // files list is empty (missing files aren't listed by the scanner), but
        // the module should still surface the deleted file.
        let st = file_statuses(&root, &[]).unwrap();
        assert!(
            st.iter().any(|f| f.status == FileStatus::Deleted),
            "expected a Deleted entry, got {:?}",
            st
        );
        let _ = dir;
    }

    #[test]
    fn head_blob_returns_committed_bytes() {
        if !have_git() {
            return;
        }
        let (dir, root) = temp_repo();
        fs::write(root.join("a.txt"), "hello").unwrap();
        commit_all(&root);
        fs::write(root.join("a.txt"), "changed").unwrap();
        let b = resolve_baseline(&root).unwrap().unwrap();
        let blob = head_blob(&b, Path::new("a.txt")).unwrap();
        assert_eq!(blob, b"hello");
        let _ = dir;
    }

    #[test]
    fn head_blob_for_untracked_file_errors() {
        if !have_git() {
            return;
        }
        let (dir, root) = temp_repo();
        fs::write(root.join("new.xlsx"), "n").unwrap();
        let b = resolve_baseline(&root).unwrap().unwrap();
        assert!(head_blob(&b, Path::new("new.xlsx")).is_err());
        let _ = dir;
    }

    // ---- sides / branches / generic file statuses ----

    fn run(root: &Path, args: &[&str]) {
        Command::new("git")
            .args(args)
            .current_dir(root)
            .output()
            .unwrap();
    }

    /// Repo with branch `main`: a.xlsx (text "v1"), c.xlsx; branch `feature`
    /// (from main): a.xlsx modified ("v1\nline2"), b.xlsx added, c.xlsx
    /// removed. Chinese-named file on feature only.
    fn two_branch_repo() -> (tempfile::TempDir, PathBuf) {
        let (dir, root) = temp_repo();
        fs::write(root.join("a.xlsx"), "v1\n").unwrap();
        fs::write(root.join("c.xlsx"), "c\n").unwrap();
        commit_all(&root);
        run(&root, &["checkout", "-q", "-b", "feature"]);
        fs::write(root.join("a.xlsx"), "v1\nline2\n").unwrap();
        fs::write(root.join("b.xlsx"), "b\n").unwrap();
        fs::write(root.join("表.xlsx"), "cn\n").unwrap();
        fs::remove_file(root.join("c.xlsx")).unwrap();
        commit_all(&root);
        run(&root, &["checkout", "-q", "main"]);
        (dir, root)
    }

    #[test]
    fn resolve_side_branch_tag_and_sha() {
        if !have_git() {
            return;
        }
        let (dir, root) = temp_repo();
        fs::write(root.join("a.xlsx"), "x").unwrap();
        commit_all(&root);
        let b = resolve_baseline(&root).unwrap().unwrap();
        // branch name
        assert!(resolve_side(&b, &Side::Ref("main".into())).unwrap().is_some());
        // tag
        run(&root, &["tag", "v1"]);
        assert!(resolve_side(&b, &Side::Ref("v1".into())).unwrap().is_some());
        // abbreviated sha
        let sha = run_git(&root, &["rev-parse", "HEAD"]).unwrap();
        let short: String = sha.trim().chars().take(7).collect();
        assert!(resolve_side(&b, &Side::Ref(short.clone())).unwrap().is_some());
        // empty ref
        assert!(matches!(
            resolve_side(&b, &Side::Ref(String::new())),
            Err(GitError::UnknownRef(_))
        ));
        let _ = dir;
    }

    #[test]
    fn resolve_side_unknown_ref_names_the_ref() {
        if !have_git() {
            return;
        }
        let (dir, root) = temp_repo();
        fs::write(root.join("a.xlsx"), "x").unwrap();
        commit_all(&root);
        let b = resolve_baseline(&root).unwrap().unwrap();
        match resolve_side(&b, &Side::Ref("no-such-branch".into())) {
            Err(GitError::UnknownRef(r)) => assert_eq!(r, "no-such-branch"),
            other => panic!("expected UnknownRef, got {other:?}"),
        }
        // worktree needs no resolution
        assert_eq!(resolve_side(&b, &Side::Worktree).unwrap(), None);
        let _ = dir;
    }

    #[test]
    fn branches_lists_local_and_remote_deduped() {
        if !have_git() {
            return;
        }
        let (dir, root) = temp_repo();
        fs::write(root.join("a.xlsx"), "x").unwrap();
        commit_all(&root);
        run(&root, &["branch", "dev"]);
        let sha = run_git(&root, &["rev-parse", "HEAD"]).unwrap();
        let sha = sha.trim();
        run(&root, &["update-ref", &format!("refs/remotes/origin/dev"), sha]);
        run(&root, &["update-ref", "refs/remotes/origin/main", sha]);
        // origin/HEAD symref — must be skipped
        run(&root, &["symbolic-ref", "refs/remotes/origin/HEAD", "refs/remotes/origin/main"]);
        let listing = branches(&root);
        assert_eq!(listing.warning, None);
        // local main + dev; origin/* twins deduped away by short name
        assert_eq!(listing.branches, vec!["dev".to_string(), "main".to_string()]);
        let _ = dir;
    }

    #[test]
    fn branches_outside_repo_is_empty_with_warning() {
        if !have_git() {
            return;
        }
        let tmp = tempfile::tempdir().unwrap();
        let listing = branches(tmp.path());
        assert!(listing.branches.is_empty());
        assert!(listing.warning.is_some());
    }

    #[test]
    fn file_statuses_sides_branch_vs_branch() {
        if !have_git() {
            return;
        }
        let (dir, root) = two_branch_repo();
        let st = file_statuses_sides(
            &root,
            &[],
            &Side::Ref("main".into()),
            &Side::Ref("feature".into()),
            &[],
            &[],
        )
        .unwrap();
        let by: std::collections::HashMap<String, FileWithStatus> = st
            .into_iter()
            .map(|f| {
                (
                    Path::new(&f.path)
                        .file_name()
                        .unwrap()
                        .to_string_lossy()
                        .into_owned(),
                    f,
                )
            })
            .collect();
        // main relative to feature: a modified (with text numstat), b only on
        // feature → deleted, c only on main → added, 表 only on feature → deleted.
        let a = &by["a.xlsx"];
        assert_eq!(a.status, FileStatus::Modified);
        assert_eq!(a.numstat_added, 0);
        assert_eq!(a.numstat_deleted, 1);
        assert_eq!(by["b.xlsx"].status, FileStatus::Deleted);
        assert_eq!(by["c.xlsx"].status, FileStatus::Added);
        assert_eq!(by["表.xlsx"].status, FileStatus::Deleted);
        let _ = dir;
    }

    #[test]
    fn file_statuses_sides_worktree_left_includes_untracked_as_added() {
        if !have_git() {
            return;
        }
        let (dir, root) = two_branch_repo();
        // On main: untracked file exists only in the worktree.
        fs::write(root.join("fresh.xlsx"), "new\n").unwrap();
        let scanned = vec![root.join("a.xlsx"), root.join("c.xlsx"), root.join("fresh.xlsx")];
        let st = file_statuses_sides(
            &root,
            &scanned,
            &Side::Worktree,
            &Side::Ref("feature".into()),
            &[],
            &[],
        )
        .unwrap();
        let by: std::collections::HashMap<String, FileStatus> = st
            .iter()
            .map(|f| {
                (
                    Path::new(&f.path)
                        .file_name()
                        .unwrap()
                        .to_string_lossy()
                        .into_owned(),
                    f.status,
                )
            })
            .collect();
        // worktree (main + fresh) relative to feature: fresh only on the left
        // → added (the generic path's four-state reading of an untracked file).
        assert_eq!(by["fresh.xlsx"], FileStatus::Added);
        assert_eq!(by["b.xlsx"], FileStatus::Deleted);
        let _ = dir;
    }

    #[test]
    fn file_statuses_sides_unknown_ref_propagates() {
        if !have_git() {
            return;
        }
        let (dir, root) = two_branch_repo();
        let err = file_statuses_sides(
            &root,
            &[],
            &Side::Worktree,
            &Side::Ref("no-such".into()),
            &[],
            &[],
        );
        assert!(matches!(err, Err(GitError::UnknownRef(r)) if r == "no-such"));
        // worktree on the right is rejected
        let err = file_statuses_sides(
            &root,
            &[],
            &Side::Worktree,
            &Side::Worktree,
            &[],
            &[],
        );
        assert!(err.is_err());
        let _ = dir;
    }

    #[test]
    fn file_statuses_sides_outside_repo_degrades_to_clean() {
        if !have_git() {
            return;
        }
        let tmp = tempfile::tempdir().unwrap();
        let files = vec![tmp.path().join("a.xlsx")];
        let st = file_statuses_sides(
            tmp.path(),
            &files,
            &Side::Worktree,
            &Side::Ref("main".into()),
            &[],
            &[],
        )
        .unwrap();
        assert_eq!(st.len(), 1);
        assert_eq!(st[0].status, FileStatus::Clean);
    }

    #[test]
    fn file_statuses_sides_honors_include_globs() {
        if !have_git() {
            return;
        }
        let (dir, root) = two_branch_repo();
        // Only compare files under a subdir that doesn't hold any of them.
        let st = file_statuses_sides(
            &root,
            &[],
            &Side::Ref("main".into()),
            &Side::Ref("feature".into()),
            &["none/*.xlsx".to_string()],
            &[],
        )
        .unwrap();
        assert!(st.is_empty());
        let _ = dir;
    }

    #[test]
    fn ref_blob_reads_branch_content() {
        if !have_git() {
            return;
        }
        let (dir, root) = two_branch_repo();
        let b = resolve_baseline(&root).unwrap().unwrap();
        // a.xlsx on main is "v1\n"; on feature it gains a line.
        let main = ref_blob(&b, "main", Path::new("a.xlsx")).unwrap();
        assert_eq!(main, b"v1\n");
        let feature = ref_blob(&b, "feature", Path::new("a.xlsx")).unwrap();
        assert_eq!(feature, b"v1\nline2\n");
        // absent at main
        assert!(ref_blob(&b, "main", Path::new("b.xlsx")).is_err());
        let _ = dir;
    }
}
// =============================================================================
// Sheet-level diff — compare two parsed previews (working tree vs HEAD) and
// annotate each working cell with a diff status.
//
// The preview grid is built from the *working* file, so HEAD-only rows cannot
// be rendered in it; they are counted in `DiffSummary.deleted_rows` instead.
// `added_rows` counts working rows with no HEAD counterpart (their cells are
// all `Added`). This keeps the diff honest for rows that exist in both files
// while still surfacing the magnitude of removals.
// =============================================================================

pub mod sheet_diff {
    use super::{Baseline, CellDiff, GitError, Side};
    use crate::excel::{
        ParsedCell, ParsedPreview, ParsedRow, ParsedSchemaInfo, parsed_preview_with,
    };
    use serde_json::Value;
    use std::collections::HashMap;
    use std::path::Path;
    use tablec_core::core::schema::SchemaParser;

    /// Aggregate counts for the diff of a sheet.
    #[derive(Debug, Clone, Default, serde::Serialize)]
    pub struct DiffSummary {
        /// Working rows that have a HEAD counterpart (compared cell-wise).
        pub compared_rows: usize,
        /// Working rows with no HEAD counterpart — all cells `Added`.
        pub added_rows: usize,
        /// HEAD rows with no working counterpart — not rendered, counted only.
        pub deleted_rows: usize,
        /// Total cells whose status is `Modified`.
        pub modified_cells: usize,
        /// Total cells with any change (added, deleted, or modified).
        pub changed_cells: usize,
    }

    /// Compare the working preview against the HEAD preview and return, for
    /// every working row (in working order), the per-cell diff status, plus
    /// an aggregate summary.
    pub fn diff_parsed(
        work: &ParsedPreview,
        head: &ParsedPreview,
    ) -> (Vec<Vec<CellDiff>>, DiffSummary) {
        let unique = unique_field(work.schema.as_ref());
        let unique_ref = unique.as_deref();
        let fields = work
            .schema
            .as_ref()
            .map(|s| &s.fields)
            .cloned()
            .unwrap_or_default();

        // Index HEAD rows by unique key (first occurrence wins).
        let mut head_by_key: HashMap<String, usize> = HashMap::new();
        for (i, row) in head.rows.iter().enumerate() {
            if let Some(k) = row_key(row, unique_ref, &fields) {
                head_by_key.entry(k).or_insert(i);
            }
        }

        let mut diff_rows: Vec<Vec<CellDiff>> = Vec::with_capacity(work.rows.len());
        let mut summary = DiffSummary::default();
        let mut used_head: Vec<bool> = vec![false; head.rows.len()];

        for (wi, wrow) in work.rows.iter().enumerate() {
            // Locate the HEAD counterpart.
            let hi = match unique_ref {
                Some(_) => {
                    row_key(wrow, unique_ref, &fields).and_then(|k| head_by_key.get(&k).copied())
                }
                None => Some(wi),
            };
            let Some(hi) = hi.filter(|&i| i < head.rows.len()) else {
                // Working row with no HEAD counterpart → all Added.
                diff_rows.push(
                    wrow.cells
                        .iter()
                        .map(|_| CellDiff::Added)
                        .collect::<Vec<_>>(),
                );
                summary.added_rows += 1;
                summary.changed_cells += wrow.cells.len();
                continue;
            };
            used_head[hi] = true;
            let hrow = &head.rows[hi];
            summary.compared_rows += 1;

            let row_diff = diff_row(wrow, hrow);
            summary.modified_cells += row_diff
                .iter()
                .filter(|c| **c == CellDiff::Modified)
                .count();
            summary.changed_cells += row_diff
                .iter()
                .filter(|c| **c != CellDiff::Unchanged)
                .count();
            diff_rows.push(row_diff);
        }

        summary.deleted_rows = used_head.iter().filter(|u| !**u).count();
        (diff_rows, summary)
    }

    fn diff_row(w: &ParsedRow, h: &ParsedRow) -> Vec<CellDiff> {
        let n = w.cells.len().max(h.cells.len());
        (0..n)
            .map(|i| match (w.cells.get(i), h.cells.get(i)) {
                (Some(wc), Some(hc)) => cell_diff(wc, hc),
                (Some(_), None) => CellDiff::Added,
                (None, Some(_)) => CellDiff::Deleted,
                (None, None) => CellDiff::Unchanged,
            })
            .collect()
    }

    /// Diff a single cell pair. Uses the *parsed* value for equality so
    /// numeric width differences normalize; falls back to raw text when
    /// either side failed to parse.
    fn cell_diff(w: &ParsedCell, h: &ParsedCell) -> CellDiff {
        let w_ok = w.error.is_none();
        let h_ok = h.error.is_none();
        if w_ok && h_ok {
            match (&w.value, &h.value) {
                (Some(a), Some(b)) => {
                    if values_equal(a, b) {
                        CellDiff::Unchanged
                    } else {
                        CellDiff::Modified
                    }
                }
                (Some(_), None) => CellDiff::Added,
                (None, Some(_)) => CellDiff::Deleted,
                (None, None) => CellDiff::Unchanged,
            }
        } else {
            // At least one side failed to parse: compare raw text.
            if w.raw == h.raw {
                CellDiff::Unchanged
            } else {
                CellDiff::Modified
            }
        }
    }

    /// JSON `Value` equality that treats numbers numerically across widths
    /// (`1` == `1.0`) but otherwise exactly (`1.5` vs `1.5`), mirroring the
    /// core `Value` semantics the spec locks.
    fn values_equal(a: &Value, b: &Value) -> bool {
        match (a, b) {
            (Value::Number(x), Value::Number(y)) => numbers_equal(x, y),
            _ => a == b,
        }
    }

    fn numbers_equal(a: &serde_json::Number, b: &serde_json::Number) -> bool {
        if let (Some(x), Some(y)) = (a.as_i64(), b.as_i64()) {
            return x == y;
        }
        if let (Some(x), Some(y)) = (a.as_u64(), b.as_u64()) {
            return x == y;
        }
        if let (Some(x), Some(y)) = (a.as_f64(), b.as_f64()) {
            return x.to_bits() == y.to_bits();
        }
        false
    }

    /// The field that carries a field-level `@unique` constraint, if any.
    fn unique_field(schema: Option<&ParsedSchemaInfo>) -> Option<String> {
        schema.and_then(|s| {
            s.fields
                .iter()
                .find(|f| f.constraint.as_ref().is_some_and(|c| c.func == "unique"))
                .map(|f| f.name.clone())
        })
    }

    /// Extract the unique-key value for a row (JSON of the key cell).
    fn row_key(
        row: &ParsedRow,
        unique: Option<&str>,
        fields: &[tablec_core::core::table::field::Field],
    ) -> Option<String> {
        let idx = unique.and_then(|name| fields.iter().position(|f| f.name == name))?;
        row.cells
            .get(idx)
            .and_then(|c| c.value.clone())
            .map(|v| v.to_string())
    }

    /// Materialize the HEAD blob for `path` into a temp file (same extension
    /// so calamine can sniff the format) and return it with a guard.
    pub struct HeadTemp {
        file: tempfile::NamedTempFile,
    }

    impl HeadTemp {
        pub fn path(&self) -> &Path {
            self.file.path()
        }
    }

    /// Write the HEAD blob for `path` to a temp file.
    ///
    /// Returns `Ok(None)` when the file has no HEAD version (untracked) —
    /// callers treat that as "all cells added". Returns `Err` only for real
    /// git failures (missing binary), which callers treat as "no diff".
    pub fn materialize_head(
        baseline: &Baseline,
        repo_rel: &Path,
        ext: &str,
    ) -> Result<Option<HeadTemp>, GitError> {
        materialize_ref(baseline, "HEAD", repo_rel, ext)
    }

    /// Same as [`materialize_head`] for any ref (a resolved sha, branch, tag
    /// or `HEAD`). `Ok(None)` = the path has no version at that ref.
    pub fn materialize_ref(
        baseline: &Baseline,
        ref_spec: &str,
        repo_rel: &Path,
        ext: &str,
    ) -> Result<Option<HeadTemp>, GitError> {
        match super::ref_blob(baseline, ref_spec, repo_rel) {
            Ok(bytes) => {
                use std::io::Write as _;
                let mut b = tempfile::Builder::new();
                b.prefix("tablec-side-").suffix(ext);
                let mut f = b.tempfile().map_err(GitError::MissingBinary)?;
                std::io::Write::write_all(&mut f, &bytes).map_err(GitError::MissingBinary)?;
                f.flush().map_err(GitError::MissingBinary)?;
                Ok(Some(HeadTemp { file: f }))
            }
            // `git show <ref>:<path>` failing means the path is absent at
            // that ref — an expected condition (only on one side), not an error.
            Err(GitError::Failed(_)) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Stamp every cell of `preview` with `status` and set a matching
    /// all-one-status summary.
    fn stamp_all(mut preview: ParsedPreview, status: CellDiff) -> (ParsedPreview, DiffSummary) {
        let added_rows = preview.rows.len();
        let changed_cells: usize = preview.rows.iter().map(|r| r.cells.len()).sum();
        for row in preview.rows.iter_mut() {
            for cell in row.cells.iter_mut() {
                cell.diff = Some(status);
            }
        }
        let summary = DiffSummary {
            compared_rows: 0,
            added_rows: if status == CellDiff::Added { added_rows } else { 0 },
            deleted_rows: if status == CellDiff::Deleted { added_rows } else { 0 },
            modified_cells: 0,
            changed_cells,
        };
        preview.set_diff_summary(summary.clone());
        (preview, summary)
    }

    /// Diff a preview body (already parsed from the correct side's content —
    /// see the handlers' body-source resolution) against the right side.
    ///
    /// `body_from_right` marks the deleted-file case: the body IS the right
    /// side's blob (the file exists only on the right), so every row is
    /// `deleted`. When the right side has no version of the file, every cell
    /// is `added`. Errors degrade to the un-diffed body via the caller.
    #[allow(clippy::too_many_arguments)]
    pub fn diff_preview_sides(
        body: ParsedPreview,
        body_from_right: bool,
        baseline: &Baseline,
        repo_rel: &Path,
        sheet: &str,
        parser: &dyn SchemaParser,
        right: &Side,
    ) -> Result<(ParsedPreview, Option<DiffSummary>), GitError> {
        let right_sha = super::resolve_side(baseline, right)?
            .expect("right side is never Worktree (enforced upstream)");

        if body_from_right {
            let (preview, summary) = stamp_all(body, CellDiff::Deleted);
            return Ok((preview, Some(summary)));
        }

        let ext = format!(
            ".{}",
            repo_rel
                .extension()
                .map(|e| e.to_string_lossy())
                .unwrap_or_else(|| "xlsx".into())
        );
        let Some(right_temp) = materialize_ref(baseline, &right_sha, repo_rel, &ext)? else {
            // No version on the right: everything is new relative to it.
            let (preview, summary) = stamp_all(body, CellDiff::Added);
            return Ok((preview, Some(summary)));
        };

        let right = match parsed_preview_with(right_temp.path(), sheet, parser, usize::MAX) {
            Ok(pp) if pp.schema.is_some() => pp,
            // Sheet missing on the right (renamed/new sheet) → no cell diff.
            Ok(_) => return Ok((body, None)),
            Err(_) => return Ok((body, None)),
        };

        let (row_diffs, summary) = diff_parsed(&body, &right);
        let mut body = body;
        for (row, diffs) in body.rows.iter_mut().zip(row_diffs.iter()) {
            for (cell, d) in row.cells.iter_mut().zip(diffs.iter()) {
                cell.diff = Some(*d);
            }
        }
        body.set_diff_summary(summary.clone());
        Ok((body, Some(summary)))
    }

    /// Diff the working preview of `path`/`sheet` against the HEAD version.
    ///
    /// On success the returned preview carries both per-cell `diff` statuses
    /// and a `diff_summary` (stamped via `set_diff_summary`), so handlers get
    /// everything through the preview object. The tuple's second element is a
    /// convenience copy of the summary; `None` when no usable baseline exists
    /// or the sheet has no HEAD counterpart (no cell-level diff was computed).
    pub fn diff_preview(
        work: ParsedPreview,
        path: &Path,
        sheet: &str,
        parser: &dyn SchemaParser,
    ) -> Result<(ParsedPreview, Option<DiffSummary>), GitError> {
        let Some(dir) = path.parent() else {
            return Ok((work, None));
        };
        let Some(baseline) = super::resolve_baseline(dir)? else {
            return Ok((work, None));
        };
        let Ok(repo_rel) = path.strip_prefix(&baseline.repo_root) else {
            // File lives outside the repo — no baseline.
            return Ok((work, None));
        };
        let ext = path
            .extension()
            .map(|e| format!(".{}", e.to_string_lossy()))
            .unwrap_or_else(|| ".xlsx".to_string());
        let Some(head_temp) = materialize_head(&baseline, repo_rel, &ext)? else {
            // Untracked file: everything is new relative to HEAD. Mark every
            // cell of the working preview as `added`.
            let added_rows = work.rows.len();
            let changed_cells: usize = work.rows.iter().map(|r| r.cells.len()).sum();
            let mut work = work;
            for row in work.rows.iter_mut() {
                for cell in row.cells.iter_mut() {
                    cell.diff = Some(CellDiff::Added);
                }
            }
            let summary = DiffSummary {
                compared_rows: 0,
                added_rows,
                deleted_rows: 0,
                modified_cells: 0,
                changed_cells,
            };
            work.set_diff_summary(summary.clone());
            return Ok((work, Some(summary)));
        };

        let head = match parsed_preview_with(head_temp.path(), sheet, parser, usize::MAX) {
            Ok(pp) if pp.schema.is_some() => pp,
            // Sheet missing at HEAD (renamed/new sheet) → all new.
            Ok(_) => return Ok((work, None)),
            Err(_) => return Ok((work, None)),
        };

        let (row_diffs, summary) = diff_parsed(&work, &head);
        // Attach per-cell statuses to the working preview.
        let mut work = work;
        for (row, diffs) in work.rows.iter_mut().zip(row_diffs.iter()) {
            for (cell, d) in row.cells.iter_mut().zip(diffs.iter()) {
                cell.diff = Some(*d);
            }
        }
        work.set_diff_summary(summary.clone());
        Ok((work, Some(summary)))
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use crate::excel::{ParsedCell, ParsedPreview, PreviewSummary};
        use serde_json::json;

        fn cell(raw: &str, value: Option<Value>) -> ParsedCell {
            ParsedCell {
                raw: raw.to_string(),
                value,
                error: None,
                type_name: "int".to_string(),
                diff: None,
            }
        }

        fn row(idx: usize, cells: Vec<ParsedCell>) -> ParsedRow {
            ParsedRow {
                row_index: idx,
                line: idx + 1,
                error_count: 0,
                cells,
            }
        }

        fn preview(rows: Vec<ParsedRow>) -> ParsedPreview {
            let n = rows.len();
            ParsedPreview {
                sheet: "S".to_string(),
                schema: None,
                data_start_row: 5,
                total_rows: n + 5,
                shown_rows: n,
                rows,
                diagnostics: vec![],
                summary: PreviewSummary {
                    data_rows: n,
                    shown_rows: n,
                    total_rows: n + 5,
                    error_count: 0,
                    warning_count: 0,
                },
                diff_summary: None,
            }
        }

        #[test]
        fn modified_cell_and_added_row_and_deleted_row() {
            // Index fallback: no unique field.
            let work = preview(vec![
                row(
                    5,
                    vec![cell("1", Some(json!(1))), cell("x", Some(json!("x")))],
                ),
                row(
                    6,
                    vec![cell("2", Some(json!(2))), cell("y", Some(json!("y")))],
                ),
                row(
                    7,
                    vec![cell("3", Some(json!(3))), cell("z", Some(json!("z")))],
                ),
            ]);
            let head = preview(vec![
                row(
                    5,
                    vec![cell("1", Some(json!(1))), cell("X", Some(json!("X")))],
                ),
                row(
                    6,
                    vec![cell("2", Some(json!(2))), cell("y", Some(json!("y")))],
                ),
            ]);
            let (diffs, summary) = diff_parsed(&work, &head);
            // work row 5: second cell X→x modified
            assert_eq!(diffs[0][0], CellDiff::Unchanged);
            assert_eq!(diffs[0][1], CellDiff::Modified);
            // work row 6: unchanged
            assert_eq!(diffs[1][0], CellDiff::Unchanged);
            assert_eq!(diffs[1][1], CellDiff::Unchanged);
            // work row 7: added (no HEAD counterpart at index 7)
            assert_eq!(diffs[2][0], CellDiff::Added);
            assert_eq!(diffs[2][1], CellDiff::Added);
            assert_eq!(summary.added_rows, 1);
            assert_eq!(summary.deleted_rows, 0); // head rows both matched
            assert_eq!(summary.modified_cells, 1);
        }

        #[test]
        fn deleted_row_counts_head_only_rows() {
            let work = preview(vec![row(5, vec![cell("1", Some(json!(1)))])]);
            let head = preview(vec![
                row(5, vec![cell("1", Some(json!(1)))]),
                row(6, vec![cell("2", Some(json!(2)))]),
            ]);
            let (_diffs, summary) = diff_parsed(&work, &head);
            assert_eq!(summary.deleted_rows, 1);
        }

        #[test]
        fn numeric_equality_across_widths_is_unchanged() {
            let work = preview(vec![row(5, vec![cell("1.0", Some(json!(1.0)))])]);
            let head = preview(vec![row(5, vec![cell("1", Some(json!(1)))])]);
            let (diffs, _s) = diff_parsed(&work, &head);
            assert_eq!(diffs[0][0], CellDiff::Unchanged);
        }

        #[test]
        fn index_fallback_aligns_by_position() {
            // No unique field (schema None) → index fallback: work rows pair
            // with HEAD rows by Vec position, regardless of their source
            // row_index. Both rows here hold "a" → Unchanged.
            let work = preview(vec![row(5, vec![cell("a", Some(json!("a")))])]);
            let head = preview(vec![row(9, vec![cell("a", Some(json!("a")))])]);
            let (diffs, _s) = diff_parsed(&work, &head);
            assert_eq!(diffs[0][0], CellDiff::Unchanged);
        }

        #[test]
        fn parse_error_cells_fall_back_to_raw_compare() {
            let mut w = cell("x", None);
            w.error = Some("type mismatch".to_string());
            let h = cell("y", None);
            let d = cell_diff(&w, &h);
            assert_eq!(d, CellDiff::Modified);
            let d2 = cell_diff(&w, &cell("x", None));
            assert_eq!(d2, CellDiff::Unchanged);
        }

        // ---- materialize_ref / diff_preview_sides over a real repo ----

        use std::process::Command;

        fn have_git() -> bool {
            Command::new("git")
                .arg("--version")
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false)
        }

        fn git(root: &Path, args: &[&str]) {
            Command::new("git")
                .args(args)
                .current_dir(root)
                .output()
                .unwrap();
        }

        #[test]
        fn materialize_ref_reads_branch_blob_and_absent_is_none() {
            if !have_git() {
                return;
            }
            let dir = tempfile::tempdir().unwrap();
            let root = dir.path().to_path_buf();
            git(&root, &["init", "-q", "-b", "main"]);
            git(&root, &["config", "user.email", "t@e.com"]);
            git(&root, &["config", "user.name", "T"]);
            std::fs::write(root.join("a.txt"), "v1\n").unwrap();
            git(&root, &["add", "-A"]);
            git(&root, &["commit", "-q", "-m", "init"]);
            git(&root, &["checkout", "-q", "-b", "feature"]);
            std::fs::write(root.join("a.txt"), "v1\nline2\n").unwrap();
            std::fs::write(root.join("only-feature.txt"), "x").unwrap();
            git(&root, &["add", "-A"]);
            git(&root, &["commit", "-q", "-m", "f"]);
            git(&root, &["checkout", "-q", "main"]);

            let baseline = super::super::resolve_baseline(&root).unwrap().unwrap();
            let t = materialize_ref(&baseline, "feature", Path::new("a.txt"), ".txt")
                .unwrap()
                .expect("a.txt exists on feature");
            assert_eq!(std::fs::read(t.path()).unwrap(), b"v1\nline2\n");
            // absent at main → None, not an error
            assert!(
                materialize_ref(&baseline, "main", Path::new("only-feature.txt"), ".txt")
                    .unwrap()
                    .is_none()
            );
        }

        #[test]
        fn diff_preview_sides_deleted_file_stamps_all_deleted() {
            if !have_git() {
                return;
            }
            let dir = tempfile::tempdir().unwrap();
            let root = dir.path().to_path_buf();
            git(&root, &["init", "-q", "-b", "main"]);
            git(&root, &["config", "user.email", "t@e.com"]);
            git(&root, &["config", "user.name", "T"]);
            std::fs::write(root.join("a.txt"), "x").unwrap();
            git(&root, &["add", "-A"]);
            git(&root, &["commit", "-q", "-m", "init"]);

            let baseline = super::super::resolve_baseline(&root).unwrap().unwrap();
            let body = preview(vec![row(5, vec![cell("1", Some(json!(1)))])]);
            let (out, summary) =
                diff_preview_sides(body, true, &baseline, Path::new("a.txt"), "S", &Noop, &Side::Ref("HEAD".into()))
                    .unwrap();
            assert_eq!(out.rows[0].cells[0].diff, Some(CellDiff::Deleted));
            let s = summary.unwrap();
            assert_eq!(s.deleted_rows, 1);
            assert_eq!(s.changed_cells, 1);
        }

        #[test]
        fn diff_preview_sides_right_absent_stamps_all_added() {
            if !have_git() {
                return;
            }
            let dir = tempfile::tempdir().unwrap();
            let root = dir.path().to_path_buf();
            git(&root, &["init", "-q", "-b", "main"]);
            git(&root, &["config", "user.email", "t@e.com"]);
            git(&root, &["config", "user.name", "T"]);
            std::fs::write(root.join("seed.txt"), "x").unwrap();
            git(&root, &["add", "-A"]);
            git(&root, &["commit", "-q", "-m", "init"]);
            std::fs::write(root.join("untracked.txt"), "x").unwrap();

            let baseline = super::super::resolve_baseline(&root).unwrap().unwrap();
            let body = preview(vec![row(5, vec![cell("1", Some(json!(1)))])]);
            // "untracked.txt" has no version at HEAD → everything is added.
            let (out, summary) =
                diff_preview_sides(body, false, &baseline, Path::new("untracked.txt"), "S", &Noop, &Side::Ref("HEAD".into()))
                    .unwrap();
            assert_eq!(out.rows[0].cells[0].diff, Some(CellDiff::Added));
            let s = summary.unwrap();
            assert_eq!(s.added_rows, 1);
        }

        /// Parser stub — the two stamping tests never reach a real parse
        /// (no right-side blob is ever opened).
        struct Noop;
        impl SchemaParser for Noop {
            fn name(&self) -> &str {
                "noop"
            }
            fn parse_schema(
                &self,
                _sheet_name: &str,
                _sheet: &[Vec<String>],
            ) -> Result<
                tablec_core::core::schema::SchemaParseResult,
                Vec<tablec_core::core::diagnostic::Diagnostic>,
            > {
                unreachable!()
            }
        }
    }
}

// -------------------------------------------------------------------------
// End-to-end diff_preview test: real temp repo + real xlsx modified in one cell.
// -------------------------------------------------------------------------

#[cfg(test)]
mod diff_preview_e2e_tests {
    use super::sheet_diff::diff_preview;
    use crate::excel::parsed_preview_with;
    use std::process::Command;
    use tablec_core::core::schema::StandardSchemaParser;

    fn have_git() -> bool {
        Command::new("git")
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    fn fixture() -> std::path::PathBuf {
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../tablec-core/tests/fixtures/testdata/basic_table.xlsx")
    }

    #[test]
    fn diff_preview_detects_modified_cell() {
        if !have_git() {
            return;
        }
        let f = fixture();
        if !f.exists() {
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        Command::new("git")
            .args(["init", "-q", "-b", "main"])
            .current_dir(root)
            .output()
            .unwrap();
        for cfg in [["user.email", "t@e.com"], ["user.name", "T"]] {
            Command::new("git")
                .args(["config", cfg[0], cfg[1]])
                .current_dir(root)
                .output()
                .unwrap();
        }
        let data = root.join("data");
        std::fs::create_dir_all(&data).unwrap();
        std::fs::copy(&f, data.join("base.xlsx")).unwrap();
        Command::new("git")
            .args(["add", "-A"])
            .current_dir(root)
            .output()
            .unwrap();
        Command::new("git")
            .args(["commit", "-q", "-m", "init"])
            .current_dir(root)
            .output()
            .unwrap();

        // Modify one cell: rewrite the working copy's sheet1.xml cell B6.
        let sheet_path = data.join("base.xlsx");
        let bytes = std::fs::read(&sheet_path).unwrap();
        // Simplest robust change: append a byte — the xlsx zip tolerates it and
        // calamine still reads the cells; the file then differs from HEAD.
        let mut bytes = bytes;
        bytes.push(0);
        std::fs::write(&sheet_path, bytes).unwrap();

        let parser = StandardSchemaParser;
        // Discover the actual sheet name (fixture uses "Items", not "Sheet1").
        let sheets = crate::excel::list_sheets(&sheet_path).expect("list sheets");
        let sheet = sheets.first().expect("at least one sheet").name.clone();
        let work =
            parsed_preview_with(&sheet_path, &sheet, &parser, 100).expect("working preview parses");
        assert!(
            work.schema.is_some(),
            "working preview should have a schema (sheet {sheet:?})"
        );
        let (diffed, summary) = diff_preview(work, &sheet_path, &sheet, &parser)
            .expect("diff_preview should not error");
        match summary {
            Some(s) => {
                assert_eq!(s.compared_rows, diffed.rows.len(), "rows compared");
                // Appending a byte does not change cell values, so all cells
                // remain Unchanged — but the file is still modified at HEAD.
                for row in &diffed.rows {
                    for c in &row.cells {
                        assert!(
                            c.diff.is_none() || c.diff == Some(crate::git::CellDiff::Unchanged),
                            "expected unchanged, got {:?}",
                            c.diff
                        );
                    }
                }
            }
            None => panic!("expected a diff summary for a modified file"),
        }
    }
}
