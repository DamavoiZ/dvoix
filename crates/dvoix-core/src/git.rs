//! Git support for the vault: what changed, what is staged, the history and
//! the branches. Everything runs through the `git` binary of the system, so
//! nothing has to be shipped with the app.
//!
//! All paths that go in and out are relative to the vault root and use `/` as
//! separator, like everywhere else in dvoix. The vault may sit in a folder of
//! a bigger repository; the paths are translated accordingly and files outside
//! of the vault stay invisible.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde::Serialize;

use crate::{Error, Result, Vault};

/// Separator between the fields of one line of `git log`.
const FIELD: char = '\u{1f}';
/// Separator between two commits of `git log`.
const RECORD: char = '\u{1e}';

/// Files dvoix rewrites on its own while it runs: the search index (a cache)
/// and the open tabs, plus the temp files of an atomic write. They change
/// without the user changing anything, so "stage everything" leaves them out
/// and the panel does not list them. Patterns are vault-relative and use the
/// pathspec matching of git, where `*` also crosses `/`.
const MACHINE_STATE: &[&str] = &[
    ".dvoix/index.sqlite*",
    ".dvoix/workspace.json",
    "*.dvoix-tmp",
];

/// True when a vault-relative path is one of [`MACHINE_STATE`].
fn is_machine_state(rel: &str) -> bool {
    rel.starts_with(".dvoix/index.sqlite")
        || rel == ".dvoix/workspace.json"
        || rel.ends_with(".dvoix-tmp")
}

/// One file with changes, as shown in the source control panel.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangedFile {
    /// Path relative to the vault root, `/`-separated.
    pub path: String,
    /// Letter of the change: `M` (changed), `A` (added), `D` (deleted),
    /// `R` (renamed) or `?` (not tracked by git yet).
    pub status: String,
}

/// The repository the vault sits in.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepoInfo {
    /// Absolute path of the repository root.
    pub root: String,
    /// Branch that is checked out, or the short commit id when HEAD is detached.
    pub branch: String,
    /// Branch the current one is pushed to, e.g. `origin/main`.
    pub upstream: Option<String>,
    /// Name of the remote push and pull work with: the remote of the
    /// upstream branch, or the first remote of the repository when there is
    /// no upstream (see [`pick_remote`]).
    pub remote: Option<String>,
    /// Commits that are not pushed yet.
    pub ahead: u32,
    /// Commits that are not fetched yet.
    pub behind: u32,
    /// False when the vault is a folder inside a bigger repository.
    pub vault_is_root: bool,
}

/// Everything the source control panel shows.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitStatus {
    /// `None` when the vault is not inside a repository.
    pub repo: Option<RepoInfo>,
    /// Files whose changes are staged.
    pub staged: Vec<ChangedFile>,
    /// Files that are changed in the working tree.
    pub unstaged: Vec<ChangedFile>,
    /// Files that git does not know yet.
    pub untracked: Vec<ChangedFile>,
}

/// The two versions of a file that a diff compares.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileDiff {
    /// The version before the change.
    pub old: String,
    /// The version after the change.
    pub new: String,
}

/// One commit of the history.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Commit {
    /// Full commit id.
    pub id: String,
    /// Short commit id.
    pub short_id: String,
    /// Name of the author.
    pub author: String,
    /// Date of the commit, ISO 8601.
    pub date: String,
    /// First line of the commit message.
    pub message: String,
}

/// One branch, local or on a remote.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Branch {
    /// Name of the branch, e.g. `main` or `origin/main`.
    pub name: String,
    /// True for branches that live on a remote.
    pub remote: bool,
    /// True for the branch that is checked out.
    pub current: bool,
}

// ------------------------------------------------------------------ reading

/// Reads the state of the repository: what is staged, what is changed and
/// what is new. A vault without a repository is not an error, the status then
/// simply has no repository.
///
/// One call of git answers everything: `status --porcelain=v2 --branch`
/// reports the changes and the branch headers (branch, upstream, ahead and
/// behind) together. The panel asks for the status on every save, so fewer
/// started programs make every save cheaper.
pub fn status(vault: &Vault) -> Result<GitStatus> {
    let Some(repo) = Repo::find(vault)? else {
        return Ok(GitStatus {
            repo: None,
            staged: Vec::new(),
            unstaged: Vec::new(),
            untracked: Vec::new(),
        });
    };

    let out = git_raw(
        &repo.root,
        // `--untracked-files=all` on purpose, not `normal`: `normal` reports
        // a new folder as one entry, but the panel lists files, filters the
        // machine state per file and offers "stage this file". Listing the
        // files of such a folder correctly would need more calls of git
        // (gitignore rules), which is what this one call is meant to avoid.
        &[
            "status",
            "--porcelain=v2",
            "--branch",
            "-z",
            "--untracked-files=all",
        ],
    )?;
    if !out.status.success() {
        return Err(Error::Git(git_error(&out)));
    }

    let mut info = RepoInfo {
        root: repo.root.to_string_lossy().into_owned(),
        branch: String::new(),
        upstream: None,
        remote: None,
        ahead: 0,
        behind: 0,
        vault_is_root: repo.prefix.is_empty(),
    };
    let mut staged = Vec::new();
    let mut unstaged = Vec::new();
    let mut untracked = Vec::new();
    // With `-z` the entries are separated by a null byte and no path is quoted.
    let text = String::from_utf8_lossy(&out.stdout).to_string();
    let mut entries = text.split('\0');
    while let Some(entry) = entries.next() {
        if let Some(header) = entry.strip_prefix("# ") {
            branch_header(&mut info, header);
            continue;
        }
        let mut fields = entry.splitn(2, ' ');
        match fields.next() {
            Some("?") => {
                // Files outside of the vault belong to the repository, not to dvoix.
                let Some(path) = fields.next().and_then(|p| repo.to_vault(p)) else {
                    continue;
                };
                if is_machine_state(&path) {
                    continue;
                }
                untracked.push(ChangedFile {
                    path,
                    status: "?".into(),
                });
            }
            Some("1") => {
                // `1 <XY> <sub> <mH> <mI> <mW> <hH> <hI> <path>`
                let mut rest = fields.next().unwrap_or_default().splitn(8, ' ');
                let letters = rest.next().unwrap_or_default();
                let Some(path) = rest.nth(6).and_then(|p| repo.to_vault(p)) else {
                    continue;
                };
                if is_machine_state(&path) {
                    continue;
                }
                push_changes(&mut staged, &mut unstaged, letters, path);
            }
            Some("2") => {
                // `2 <XY> <sub> <mH> <mI> <mW> <hH> <hI> <X><score> <path>`;
                // a rename or a copy carries the old path in the next entry.
                let mut rest = fields.next().unwrap_or_default().splitn(9, ' ');
                let letters = rest.next().unwrap_or_default();
                let path = rest.nth(7).and_then(|p| repo.to_vault(p));
                let _ = entries.next();
                let Some(path) = path else {
                    continue;
                };
                if is_machine_state(&path) {
                    continue;
                }
                push_changes(&mut staged, &mut unstaged, letters, path);
            }
            // Unmerged entries (a conflict) keep the file visible in both
            // lists, as the v1 format showed them (`U` on both sides).
            Some("u") => {
                // `u <XY> <sub> <m1> <m2> <m3> <mW> <h1> <h2> <h3> <path>`
                let mut rest = fields.next().unwrap_or_default().splitn(10, ' ');
                let Some(path) = rest.nth(9).and_then(|p| repo.to_vault(p)) else {
                    continue;
                };
                if is_machine_state(&path) {
                    continue;
                }
                staged.push(ChangedFile {
                    path: path.clone(),
                    status: "U".into(),
                });
                unstaged.push(ChangedFile {
                    path,
                    status: "U".into(),
                });
            }
            // Anything unknown is not shown.
            _ => {}
        }
    }

    // The status does not name the remote, but push and pull have a clear
    // choice: the remote of the upstream branch when there is one, the first
    // remote of the repository otherwise. The status reports the same choice
    // (see `pick_remote`); only without an upstream does it cost one extra
    // call of git (`git remote`).
    info.remote = pick_remote(&repo.root, info.upstream.as_deref());

    Ok(GitStatus {
        repo: Some(info),
        staged,
        unstaged,
        untracked,
    })
}

/// The remote an upstream branch is fetched from: the first part of its
/// name (`origin/main` -> `origin`). `None` without an upstream.
fn remote_of(upstream: Option<&str>) -> Option<String> {
    let (remote, _) = upstream?.split_once('/')?;
    Some(remote.to_string())
}

/// The remote push and pull work with: the remote of the upstream branch
/// when there is one, otherwise the first remote of the repository. The
/// status and the push/pull defaults both go through this helper, so the
/// panel never names a different remote than the one a push would use.
fn pick_remote(root: &Path, upstream: Option<&str>) -> Option<String> {
    remote_of(upstream).or_else(|| first_remote(root))
}

/// The first entry of `git remote`, `None` when the repository has none.
fn first_remote(root: &Path) -> Option<String> {
    git_optional(root, &["remote"])?
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(str::to_string)
}

/// Reads one `# ` header of `status --porcelain=v2 --branch`: the branch (or
/// the commit when HEAD is detached), the upstream and how far ahead and
/// behind it is.
fn branch_header(info: &mut RepoInfo, header: &str) {
    let mut parts = header.split(' ');
    match parts.next() {
        Some("branch.oid") => {
            // "(initial)" before the first commit, the commit id when HEAD
            // is detached: both are shown like the short id of a commit.
            let oid = parts.next().unwrap_or_default();
            if oid != "(initial)" {
                info.branch = oid.chars().take(7).collect();
            }
        }
        Some("branch.head") => {
            let head = parts.next().unwrap_or_default();
            if head != "(detached)" {
                info.branch = head.to_string();
            }
        }
        Some("branch.upstream") => {
            let upstream = parts.next().unwrap_or_default();
            if !upstream.is_empty() {
                info.upstream = Some(upstream.to_string());
            }
        }
        Some("branch.ab") => {
            // "+<ahead> -<behind>"
            for part in parts {
                let number = part.get(1..).and_then(|n| n.parse().ok()).unwrap_or(0);
                match part.as_bytes().first() {
                    Some(b'+') => info.ahead = number,
                    Some(b'-') => info.behind = number,
                    _ => {}
                }
            }
        }
        _ => {}
    }
}

/// Puts the two letters of a v2 entry into the lists of the panel: the first
/// is the change in the index (staged), the second the one in the working
/// tree. A `.` means no change there.
fn push_changes(
    staged: &mut Vec<ChangedFile>,
    unstaged: &mut Vec<ChangedFile>,
    letters: &str,
    path: String,
) {
    let mut letters = letters.chars();
    let index = letters.next().unwrap_or('.');
    let worktree = letters.next().unwrap_or('.');
    if index != '.' && index != '?' {
        staged.push(ChangedFile {
            path: path.clone(),
            status: index.to_string(),
        });
    }
    if worktree != '.' && worktree != '?' {
        unstaged.push(ChangedFile {
            path,
            status: worktree.to_string(),
        });
    }
}

/// Reads the last commits, newest first.
pub fn log(vault: &Vault, limit: u32) -> Result<Vec<Commit>> {
    let repo = Repo::of(vault)?;
    let limit = limit.clamp(1, 500);
    let format = format!("--pretty=format:%H{FIELD}%h{FIELD}%an{FIELD}%aI{FIELD}%s{RECORD}");
    let out = git(
        &repo.root,
        &["log", "--no-color", &format, "-n", &limit.to_string()],
    )?;

    let mut commits = Vec::new();
    for record in out.split(RECORD) {
        let record = record.trim_matches(|c| c == '\n' || c == '\r');
        if record.is_empty() {
            continue;
        }
        let fields: Vec<&str> = record.split(FIELD).collect();
        if fields.len() < 5 {
            continue;
        }
        commits.push(Commit {
            id: fields[0].to_string(),
            short_id: fields[1].to_string(),
            author: fields[2].to_string(),
            date: fields[3].to_string(),
            message: fields[4].trim_end().to_string(),
        });
    }
    Ok(commits)
}

/// Lists the files a commit changed.
pub fn commit_files(vault: &Vault, id: &str) -> Result<Vec<String>> {
    let repo = Repo::of(vault)?;
    let out = git(
        &repo.root,
        &["show", "--no-color", "--name-only", "--format=", id],
    )?;
    Ok(out
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .filter_map(|l| repo.to_vault(l))
        .collect())
}

/// Lists the local branches and the ones on the remotes. One call of git
/// walks both places; the full ref name tells them apart.
pub fn branches(vault: &Vault) -> Result<Vec<Branch>> {
    let repo = Repo::of(vault)?;
    let format = format!("--format=%(refname){FIELD}%(refname:short){FIELD}%(HEAD){FIELD}");
    let text = git(
        &repo.root,
        &["for-each-ref", &format, "refs/heads", "refs/remotes"],
    )?;
    let mut out = Vec::new();
    for line in text.lines() {
        let mut fields = line.split(FIELD);
        let full = fields.next().unwrap_or("").trim();
        let name = fields.next().unwrap_or("").trim().to_string();
        let head = fields.next().unwrap_or("").trim();
        if name.is_empty() {
            continue;
        }
        // `origin/HEAD` points at the default branch, it is no branch of its own.
        if name.ends_with("/HEAD") {
            continue;
        }
        out.push(Branch {
            name,
            remote: full.starts_with("refs/remotes/"),
            current: head == "*",
        });
    }
    Ok(out)
}

/// Shows the changes of one file as its two versions, so the frontend can lay
/// them out side by side or inline: the working tree against the index, the
/// index against `HEAD`, or a commit against its parent. A side where the file
/// does not exist (added, deleted, first commit) is empty.
pub fn diff(vault: &Vault, path: &str, staged: bool, commit: Option<&str>) -> Result<FileDiff> {
    let repo = Repo::of(vault)?;
    let repo_path = repo.to_repo(path);
    let (old, new) = match commit {
        Some(id) => (
            blob(&repo, &format!("{id}^"), &repo_path)?,
            blob(&repo, id, &repo_path)?,
        ),
        None if staged => (
            blob(&repo, "HEAD", &repo_path)?,
            blob(&repo, "", &repo_path)?,
        ),
        None => {
            let file = vault.resolve(path)?;
            let new = match std::fs::read(&file) {
                Ok(bytes) => String::from_utf8_lossy(&bytes).to_string(),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
                Err(e) => return Err(e.into()),
            };
            (blob(&repo, "", &repo_path)?, new)
        }
    };
    Ok(FileDiff { old, new })
}

/// The content of a file at a revision, `""` for the index. Empty when the file
/// does not exist there, e.g. before it was added or above the first commit.
fn blob(repo: &Repo, rev: &str, path: &str) -> Result<String> {
    let out = git_raw(&repo.root, &["cat-file", "blob", &format!("{rev}:{path}")])?;
    if !out.status.success() {
        return Ok(String::new());
    }
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}

// ------------------------------------------------------------------ writing

/// Stages the given files.
pub fn stage(vault: &Vault, paths: &[String]) -> Result<()> {
    let repo = Repo::of(vault)?;
    for path in paths {
        git(&repo.root, &["add", "-A", "--", &repo.to_repo(path)])?;
    }
    Ok(())
}

/// Stages every change of the vault.
pub fn stage_all(vault: &Vault) -> Result<()> {
    let repo = Repo::of(vault)?;
    repo.add_all()?;
    Ok(())
}

/// Moves the given files back out of the index.
pub fn unstage(vault: &Vault, paths: &[String]) -> Result<()> {
    let repo = Repo::of(vault)?;
    let has_commit = git_optional(&repo.root, &["rev-parse", "--verify", "HEAD"]).is_some();
    for path in paths {
        let path = repo.to_repo(path);
        if has_commit {
            // `git restore` needs git 2.23, `git reset` works with every git.
            if git_optional(&repo.root, &["restore", "--staged", "--", &path]).is_none() {
                git(&repo.root, &["reset", "--quiet", "HEAD", "--", &path])?;
            }
        } else {
            // Without a commit there is nothing to restore from: the file leaves the index.
            git(&repo.root, &["update-index", "--force-remove", "--", &path])?;
        }
    }
    Ok(())
}

/// Throws the changes of one file away. Files that git does not know yet are
/// deleted, they have no earlier version to go back to.
pub fn discard(vault: &Vault, path: &str, untracked: bool) -> Result<()> {
    let repo = Repo::of(vault)?;
    let path = repo.to_repo(path);
    if untracked {
        git(&repo.root, &["clean", "-f", "--", &path])?;
        return Ok(());
    }
    // `git restore` needs git 2.23, `git checkout --` works with every git.
    if git_optional(&repo.root, &["restore", "--worktree", "--", &path]).is_none() {
        git(&repo.root, &["checkout", "--", &path])?;
    }
    Ok(())
}

/// Creates a commit. With `stage_all` every change of the vault goes into it.
pub fn commit(vault: &Vault, message: &str, stage_all: bool) -> Result<String> {
    let message = message.trim();
    if message.is_empty() {
        return Err(Error::Git("The commit message must not be empty.".into()));
    }
    let repo = Repo::of(vault)?;
    if stage_all {
        repo.add_all()?;
    }
    git(&repo.root, &["commit", "-m", message])?;
    Ok(git(&repo.root, &["rev-parse", "--short", "HEAD"])?
        .trim()
        .to_string())
}

/// Checks out another branch.
pub fn checkout(vault: &Vault, name: &str) -> Result<()> {
    let repo = Repo::of(vault)?;
    let name = name.trim();
    if name.is_empty() {
        return Err(Error::Git("The branch name must not be empty.".into()));
    }
    switch(&repo, &["switch", name])
}

/// Creates a branch and checks it out.
pub fn create_branch(vault: &Vault, name: &str) -> Result<()> {
    let repo = Repo::of(vault)?;
    let name = name.trim();
    if name.is_empty() {
        return Err(Error::Git("The branch name must not be empty.".into()));
    }
    switch(&repo, &["switch", "-c", name])
}

/// Pushes the current branch to its remote.
pub fn push(vault: &Vault) -> Result<String> {
    let repo = Repo::of(vault)?;
    let info = repo_info(&repo)?;
    let args: Vec<&str> = match (&info.upstream, &info.remote) {
        // With an upstream, git pushes to its remote by default, which is
        // exactly the remote `pick_remote` chose for the status.
        (Some(_), _) => vec!["push"],
        // Without one, the first remote gets the branch and becomes its
        // upstream, the same remote the status names.
        (None, Some(remote)) => vec!["push", "-u", remote, &info.branch],
        (None, None) => {
            return Err(Error::Git("There is no remote to push to.".into()));
        }
    };
    let out = git_raw(&repo.root, &args)?;
    if !out.status.success() {
        return Err(Error::Git(git_error(&out)));
    }
    Ok(push_result(&out))
}

/// Fetches the remote branch and merges it into the current one.
pub fn pull(vault: &Vault) -> Result<String> {
    let repo = Repo::of(vault)?;
    let info = repo_info(&repo)?;
    if info.upstream.is_none() {
        return Err(Error::Git(
            "The branch has no upstream. Please push first.".into(),
        ));
    }
    let out = git_raw(&repo.root, &["pull"])?;
    if !out.status.success() {
        return Err(Error::Git(git_error(&out)));
    }
    Ok(pull_result(&out))
}

/// Makes the vault folder a repository of its own.
pub fn init(vault: &Vault) -> Result<()> {
    git(vault.root(), &["init"])?;
    Ok(())
}

// ------------------------------------------------------------------ helpers

/// The repository the vault sits in, with the place of the vault inside it.
struct Repo {
    /// Absolute path of the repository root.
    root: PathBuf,
    /// The vault folder relative to the repository root, empty when both are the same.
    prefix: String,
}

impl Repo {
    /// Finds the repository of the vault, `None` when the vault is not in one.
    fn find(vault: &Vault) -> Result<Option<Self>> {
        let out = git_raw(vault.root(), &["rev-parse", "--show-toplevel"])?;
        if !out.status.success() {
            return Ok(None);
        }
        let root = PathBuf::from(String::from_utf8_lossy(&out.stdout).trim().to_string());
        let prefix = vault
            .root()
            .strip_prefix(&root)
            .map(|p| p.to_string_lossy().replace('\\', "/"))
            .unwrap_or_default();
        Ok(Some(Self { root, prefix }))
    }

    /// The repository of the vault, or an error when there is none.
    fn of(vault: &Vault) -> Result<Self> {
        Self::find(vault)?.ok_or_else(|| Error::Git("The vault is not a Git repository.".into()))
    }

    /// Turns a vault-relative path into one relative to the repository root.
    fn to_repo(&self, rel: &str) -> String {
        if self.prefix.is_empty() {
            rel.to_string()
        } else {
            format!("{}/{}", self.prefix, rel)
        }
    }

    /// Turns a path relative to the repository root into a vault-relative one,
    /// `None` when the file lies outside of the vault.
    fn to_vault(&self, rel: &str) -> Option<String> {
        let rel = rel.replace('\\', "/");
        if self.prefix.is_empty() {
            return Some(rel);
        }
        rel.strip_prefix(&format!("{}/", self.prefix))
            .map(|s| s.to_string())
    }

    /// Stages every change of the vault except the files dvoix rewrites on
    /// its own (see [`MACHINE_STATE`]).
    fn add_all(&self) -> Result<()> {
        let excludes: Vec<String> = MACHINE_STATE
            .iter()
            .map(|p| format!(":(exclude){}", self.to_repo(p)))
            .collect();
        let vault = self.vault_pathspec();
        let mut args = vec!["add", "-A", "--", vault.as_str()];
        args.extend(excludes.iter().map(String::as_str));
        git(&self.root, &args)?;
        Ok(())
    }

    /// The pathspec that covers the whole vault.
    fn vault_pathspec(&self) -> String {
        if self.prefix.is_empty() {
            ".".into()
        } else {
            self.prefix.clone()
        }
    }
}

/// Reads the branch, the remote and how far ahead or behind it is.
fn repo_info(repo: &Repo) -> Result<RepoInfo> {
    let branch = match git_optional(&repo.root, &["symbolic-ref", "--short", "HEAD"]) {
        Some(name) => name.trim().to_string(),
        // Detached HEAD: show the commit instead of a branch.
        None => git(&repo.root, &["rev-parse", "--short", "HEAD"])?
            .trim()
            .to_string(),
    };
    let upstream = git_optional(
        &repo.root,
        &["rev-parse", "--abbrev-ref", "--symbolic-full-name", "@{u}"],
    )
    .map(|s| s.trim().to_string())
    .filter(|s| !s.is_empty());
    // The same choice the status reports: the upstream's remote first, the
    // first remote of the repository as a fallback.
    let remote = pick_remote(&repo.root, upstream.as_deref());
    let (ahead, behind) = match upstream {
        Some(_) => {
            // "<commits only on the remote>\t<commits only here>"
            let out = git(
                &repo.root,
                &["rev-list", "--left-right", "--count", "@{u}...HEAD"],
            )?;
            let mut parts = out.split_whitespace();
            let behind = parts.next().and_then(|n| n.parse().ok()).unwrap_or(0);
            let ahead = parts.next().and_then(|n| n.parse().ok()).unwrap_or(0);
            (ahead, behind)
        }
        None => (0, 0),
    };
    Ok(RepoInfo {
        root: repo.root.to_string_lossy().into_owned(),
        branch,
        upstream,
        remote,
        ahead,
        behind,
        vault_is_root: repo.prefix.is_empty(),
    })
}

/// Switches the branch, with `git switch` and `git checkout` as a fallback
/// for git versions older than 2.23.
fn switch(repo: &Repo, args: &[&str]) -> Result<()> {
    let out = git_raw(&repo.root, args)?;
    if out.status.success() {
        return Ok(());
    }
    let err = String::from_utf8_lossy(&out.stderr);
    if err.contains("unknown option") || err.contains("is not a git command") {
        // The old name of `git switch` is `git checkout`; the trailing `--`
        // makes sure git reads the name as a branch and not as a file.
        let mut fallback: Vec<&str> = vec!["checkout"];
        fallback.extend_from_slice(&args[1..]);
        fallback.push("--");
        return git(&repo.root, &fallback).map(|_| ());
    }
    Err(Error::Git(git_error(&out)))
}

/// The message after a successful push.
fn push_result(out: &Output) -> String {
    let text = String::from_utf8_lossy(&out.stdout);
    let text = text.trim();
    if text.is_empty() {
        "Push successful.".into()
    } else {
        text.to_string()
    }
}

/// The message after a successful pull.
fn pull_result(out: &Output) -> String {
    let text = String::from_utf8_lossy(&out.stdout);
    let text = text.trim();
    if text.is_empty() {
        "Pull successful.".into()
    } else {
        text.to_string()
    }
}

/// Runs `git` and returns its output, also when git reports an error.
fn git_raw(dir: &Path, args: &[&str]) -> Result<Output> {
    Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                Error::Git("git is not installed. Please install git.".into())
            } else {
                Error::Git(format!("git could not be started: {e}"))
            }
        })
}

/// Runs `git` and fails when it reports an error.
fn git(dir: &Path, args: &[&str]) -> Result<String> {
    let out = git_raw(dir, args)?;
    if !out.status.success() {
        return Err(Error::Git(git_error(&out)));
    }
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}

/// Runs `git` and returns `None` when it fails, e.g. because there is no
/// upstream branch yet.
fn git_optional(dir: &Path, args: &[&str]) -> Option<String> {
    let out = git_raw(dir, args).ok()?;
    if !out.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&out.stdout).to_string())
}

/// Turns git's own output into a message for the user. The common cases get
/// a friendlier text; everything else is passed through as git wrote it.
fn git_error(out: &Output) -> String {
    let stderr = String::from_utf8_lossy(&out.stderr);
    // Some errors only show up on stdout, e.g. "nothing to commit".
    let text = if stderr.trim().is_empty() {
        String::from_utf8_lossy(&out.stdout)
    } else {
        stderr
    };
    let text = text.trim();
    if text.is_empty() {
        return "git reported an error.".into();
    }
    if text.contains("Please tell me who you are") {
        return "No author identity is set yet. Please run `git config --global user.name \"Your Name\"` and `git config --global user.email \"mail@example.com\"`.".into();
    }
    if text.contains("nothing to commit") {
        return "There is nothing to commit.".into();
    }
    if text.contains("CONFLICT") {
        return "Pulling caused conflicts. Please resolve the files in the vault and commit."
            .into();
    }
    if text.contains("would be overwritten") || text.contains("Please commit your changes") {
        return "Local changes would be overwritten. Please commit or discard them first.".into();
    }
    if text.contains("Authentication failed") || text.contains("could not read Username") {
        return "git could not sign in. Please check your credentials.".into();
    }
    if text.contains("does not have any commits yet") {
        return "There are no commits yet.".into();
    }
    if text.contains("already exists") {
        return "The branch already exists.".into();
    }
    if text.contains("not a valid ref") || text.contains("is not a valid branch name") {
        return "The name is not a valid branch name.".into();
    }
    text.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    /// True when the machine has a git binary, otherwise the tests are skipped.
    fn git_available() -> bool {
        Command::new("git")
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    /// An empty repository info to read branch headers into.
    fn empty_info() -> RepoInfo {
        RepoInfo {
            root: String::new(),
            branch: String::new(),
            upstream: None,
            remote: None,
            ahead: 0,
            behind: 0,
            vault_is_root: true,
        }
    }

    #[test]
    fn branch_headers_give_branch_upstream_and_distance() {
        let mut info = empty_info();
        branch_header(
            &mut info,
            "branch.oid 292a5af7913bc5d64a669d432ffa40f5a5aba4ec",
        );
        branch_header(&mut info, "branch.head main");
        branch_header(&mut info, "branch.upstream origin/main");
        branch_header(&mut info, "branch.ab +3 -2");
        assert_eq!(info.branch, "main");
        assert_eq!(info.upstream.as_deref(), Some("origin/main"));
        assert_eq!(info.ahead, 3);
        assert_eq!(info.behind, 2);
    }

    #[test]
    fn a_detached_head_shows_the_short_commit() {
        let mut info = empty_info();
        branch_header(
            &mut info,
            "branch.oid 292a5af7913bc5d64a669d432ffa40f5a5aba4ec",
        );
        branch_header(&mut info, "branch.head (detached)");
        assert_eq!(info.branch, "292a5af");
        assert!(info.upstream.is_none());
    }

    #[test]
    fn a_repository_without_a_commit_shows_its_branch() {
        let mut info = empty_info();
        branch_header(&mut info, "branch.oid (initial)");
        branch_header(&mut info, "branch.head main");
        assert_eq!(info.branch, "main");
        assert_eq!(info.ahead, 0);
        assert_eq!(info.behind, 0);
    }

    #[test]
    fn the_letters_of_an_entry_split_into_staged_and_unstaged() {
        let mut staged = Vec::new();
        let mut unstaged = Vec::new();
        push_changes(&mut staged, &mut unstaged, "M.", "a.md".into());
        push_changes(&mut staged, &mut unstaged, ".M", "b.md".into());
        push_changes(&mut staged, &mut unstaged, "MM", "c.md".into());
        push_changes(&mut staged, &mut unstaged, "R.", "d.md".into());
        let listed: Vec<(&str, &str)> = staged
            .iter()
            .map(|f| (f.path.as_str(), f.status.as_str()))
            .collect();
        assert_eq!(listed, vec![("a.md", "M"), ("c.md", "M"), ("d.md", "R")]);
        let listed: Vec<(&str, &str)> = unstaged
            .iter()
            .map(|f| (f.path.as_str(), f.status.as_str()))
            .collect();
        assert_eq!(listed, vec![("b.md", "M"), ("c.md", "M")]);
    }

    /// A vault with a repository in it and one file to work with.
    fn vault_with_repo() -> (tempfile::TempDir, Vault) {
        let dir = tempdir().expect("temp dir");
        let vault = Vault::open(dir.path()).expect("vault");
        git(dir.path(), &["init", "--initial-branch=main"]).expect("init");
        git(dir.path(), &["config", "user.name", "dvoix"]).expect("config");
        git(dir.path(), &["config", "user.email", "dvoix@example.com"]).expect("config");
        fs::write(dir.path().join("note.md"), "# Note\n").expect("write");
        (dir, vault)
    }

    /// Registers a bare repository as a remote, so no network is needed. The
    /// directory must outlive its use; dropping it removes the remote's repo.
    fn add_bare_remote(root: &Path, name: &str) -> tempfile::TempDir {
        let remote = tempdir().expect("temp dir");
        git(remote.path(), &["init", "--bare", "--initial-branch=main"]).expect("init");
        git(
            root,
            &["remote", "add", name, &remote.path().to_string_lossy()],
        )
        .expect("remote");
        remote
    }

    #[test]
    fn status_of_a_plain_folder_has_no_repository() {
        if !git_available() {
            return;
        }
        let dir = tempdir().expect("temp dir");
        let vault = Vault::open(dir.path()).expect("vault");
        let state = status(&vault).expect("status");
        assert!(state.repo.is_none());
        assert!(state.staged.is_empty());
        assert!(state.untracked.is_empty());
    }

    #[test]
    fn new_files_are_untracked_until_they_are_staged() {
        if !git_available() {
            return;
        }
        let (_dir, vault) = vault_with_repo();
        let state = status(&vault).expect("status");
        assert_eq!(state.untracked.len(), 1);
        assert_eq!(state.untracked[0].path, "note.md");
        assert_eq!(state.untracked[0].status, "?");

        stage(&vault, &["note.md".to_string()]).expect("stage");
        let state = status(&vault).expect("status");
        assert!(state.untracked.is_empty());
        assert_eq!(state.staged.len(), 1);
        assert_eq!(state.staged[0].path, "note.md");
        assert_eq!(state.staged[0].status, "A");
    }

    #[test]
    fn a_commit_empties_the_status() {
        if !git_available() {
            return;
        }
        let (_dir, vault) = vault_with_repo();
        stage(&vault, &["note.md".to_string()]).expect("stage");
        let id = commit(&vault, "Add note", false).expect("commit");
        assert_eq!(id.len(), 7);

        let state = status(&vault).expect("status");
        assert!(state.staged.is_empty());
        assert!(state.unstaged.is_empty());
        assert!(state.untracked.is_empty());
        assert_eq!(state.repo.expect("repo").branch, "main");
    }

    #[test]
    fn commit_without_a_message_is_refused() {
        if !git_available() {
            return;
        }
        let (_dir, vault) = vault_with_repo();
        let err = commit(&vault, "  ", false).expect_err("no message");
        assert!(err.to_string().contains("commit message"));
    }

    #[test]
    fn unstaging_a_new_file_makes_it_untracked_again() {
        if !git_available() {
            return;
        }
        let (_dir, vault) = vault_with_repo();
        stage(&vault, &["note.md".to_string()]).expect("stage");
        unstage(&vault, &["note.md".to_string()]).expect("unstage");
        let state = status(&vault).expect("status");
        assert!(state.staged.is_empty());
        assert_eq!(state.untracked.len(), 1);
    }

    #[test]
    fn changes_of_a_file_are_shown_while_it_is_staged() {
        if !git_available() {
            return;
        }
        let (_dir, vault) = vault_with_repo();
        stage(&vault, &["note.md".to_string()]).expect("stage");
        commit(&vault, "Add note", false).expect("commit");

        fs::write(vault.root().join("note.md"), "# Note\nmore\n").expect("write");
        let state = status(&vault).expect("status");
        assert_eq!(state.unstaged.len(), 1);
        assert_eq!(state.unstaged[0].path, "note.md");
        assert_eq!(state.unstaged[0].status, "M");

        // Throwing the change away brings the file back to the last commit.
        discard(&vault, "note.md", false).expect("discard");
        let state = status(&vault).expect("status");
        assert!(state.unstaged.is_empty());
    }

    #[test]
    fn a_renamed_file_keeps_its_new_name() {
        if !git_available() {
            return;
        }
        let (_dir, vault) = vault_with_repo();
        stage(&vault, &["note.md".to_string()]).expect("stage");
        commit(&vault, "Add note", false).expect("commit");
        fs::rename(vault.root().join("note.md"), vault.root().join("other.md")).expect("rename");

        // Once staged, git reports the rename with the old and the new name.
        stage_all(&vault).expect("stage all");
        let state = status(&vault).expect("status");
        let staged: Vec<&str> = state.staged.iter().map(|f| f.path.as_str()).collect();
        assert_eq!(staged, vec!["other.md"]);
        assert_eq!(state.staged[0].status, "R");
    }

    #[test]
    fn the_history_lists_the_commits_newest_first() {
        if !git_available() {
            return;
        }
        let (_dir, vault) = vault_with_repo();
        commit(&vault, "First", true).expect("commit");
        fs::write(vault.root().join("second.md"), "# Second\n").expect("write");
        commit(&vault, "Second", true).expect("commit");

        let commits = log(&vault, 10).expect("log");
        assert_eq!(commits.len(), 2);
        assert_eq!(commits[0].message, "Second");
        assert_eq!(commits[1].message, "First");
        assert_eq!(commits[0].author, "dvoix");

        let files = commit_files(&vault, &commits[1].id).expect("files");
        assert_eq!(files, vec!["note.md".to_string()]);
    }

    #[test]
    fn branches_can_be_created_and_switched() {
        if !git_available() {
            return;
        }
        let (_dir, vault) = vault_with_repo();
        commit(&vault, "First", true).expect("commit");

        create_branch(&vault, "topic").expect("create");
        let state = status(&vault).expect("status");
        assert_eq!(state.repo.expect("repo").branch, "topic");

        checkout(&vault, "main").expect("checkout");
        let state = status(&vault).expect("status");
        assert_eq!(state.repo.expect("repo").branch, "main");

        let branches = branches(&vault).expect("branches");
        assert!(branches.iter().any(|b| b.name == "topic" && !b.remote));
        assert!(branches.iter().any(|b| b.name == "main" && b.current));
    }

    #[test]
    fn a_diff_shows_the_changed_lines() {
        if !git_available() {
            return;
        }
        let (_dir, vault) = vault_with_repo();
        commit(&vault, "First", true).expect("commit");
        fs::write(vault.root().join("note.md"), "# Note\nmore\n").expect("write");

        let diff = diff(&vault, "note.md", false, None).expect("diff");
        assert_eq!(diff.old, "# Note\n");
        assert_eq!(diff.new, "# Note\nmore\n");
    }

    #[test]
    fn the_index_and_the_open_tabs_stay_out_of_commits() {
        if !git_available() {
            return;
        }
        let (dir, vault) = vault_with_repo();
        fs::create_dir_all(dir.path().join(".dvoix")).expect("mkdir");
        fs::write(dir.path().join(".dvoix/index.sqlite"), "cache").expect("write");
        fs::write(dir.path().join(".dvoix/workspace.json"), "{}").expect("write");
        fs::write(dir.path().join(".dvoix/app.json"), "{}").expect("write");

        let state = status(&vault).expect("status");
        let listed: Vec<_> = state.untracked.iter().map(|f| f.path.as_str()).collect();
        assert!(listed.contains(&".dvoix/app.json"), "listed: {listed:?}");
        assert!(
            !listed
                .iter()
                .any(|p| p.contains("index.sqlite") || p.contains("workspace"))
        );

        commit(&vault, "First", true).expect("commit");
        let files = git(dir.path(), &["ls-files"]).expect("ls-files");
        assert!(files.contains(".dvoix/app.json"), "files: {files}");
        assert!(!files.contains("index.sqlite"), "files: {files}");
        assert!(!files.contains("workspace.json"), "files: {files}");
    }

    #[test]
    fn a_diff_has_both_versions_of_a_commit() {
        if !git_available() {
            return;
        }
        let (_dir, vault) = vault_with_repo();
        commit(&vault, "First", true).expect("commit");
        fs::write(vault.root().join("note.md"), "# Note\nmore\n").expect("write");
        commit(&vault, "Second", true).expect("commit");
        let id = git(vault.root(), &["rev-parse", "HEAD"]).expect("rev-parse");
        let id = id.trim();

        let second = diff(&vault, "note.md", false, Some(id)).expect("diff");
        assert_eq!(second.old, "# Note\n");
        assert_eq!(second.new, "# Note\nmore\n");

        // The first commit has no parent, so the old side is empty.
        let first = diff(&vault, "note.md", false, Some(&format!("{id}^"))).expect("diff");
        assert_eq!(first.old, "");
        assert_eq!(first.new, "# Note\n");
    }

    #[test]
    fn a_staged_diff_compares_the_index_with_head() {
        if !git_available() {
            return;
        }
        let (_dir, vault) = vault_with_repo();
        commit(&vault, "First", true).expect("commit");
        fs::write(vault.root().join("note.md"), "# Note\nstaged\n").expect("write");
        stage(&vault, &["note.md".into()]).expect("stage");
        fs::write(vault.root().join("note.md"), "# Note\nstaged\nnot yet\n").expect("write");

        let staged = diff(&vault, "note.md", true, None).expect("diff");
        assert_eq!(staged.old, "# Note\n");
        assert_eq!(staged.new, "# Note\nstaged\n");

        // An untracked file is all new.
        fs::write(vault.root().join("new.md"), "fresh\n").expect("write");
        let untracked = diff(&vault, "new.md", false, None).expect("diff");
        assert_eq!(untracked.old, "");
        assert_eq!(untracked.new, "fresh\n");
    }

    #[test]
    fn files_outside_of_the_vault_stay_invisible() {
        if !git_available() {
            return;
        }
        let dir = tempdir().expect("temp dir");
        let inner = dir.path().join("inner");
        fs::create_dir_all(&inner).expect("mkdir");
        let vault = Vault::open(&inner).expect("vault");
        git(dir.path(), &["init", "--initial-branch=main"]).expect("init");
        git(dir.path(), &["config", "user.name", "dvoix"]).expect("config");
        git(dir.path(), &["config", "user.email", "dvoix@example.com"]).expect("config");
        fs::write(dir.path().join("outside.md"), "outside\n").expect("write");
        fs::write(inner.join("inside.md"), "inside\n").expect("write");

        let state = status(&vault).expect("status");
        assert_eq!(state.untracked.len(), 1);
        assert_eq!(state.untracked[0].path, "inside.md");
        assert!(!state.repo.expect("repo").vault_is_root);

        // Staging everything only takes what lies in the vault.
        stage_all(&vault).expect("stage all");
        let state = status(&vault).expect("status");
        assert_eq!(state.staged.len(), 1);
        assert_eq!(state.staged[0].path, "inside.md");
    }

    #[test]
    fn init_makes_the_vault_a_repository() {
        if !git_available() {
            return;
        }
        let dir = tempdir().expect("temp dir");
        let vault = Vault::open(dir.path()).expect("vault");
        assert!(status(&vault).expect("status").repo.is_none());
        init(&vault).expect("init");
        assert!(status(&vault).expect("status").repo.is_some());
    }

    #[test]
    fn a_detached_head_shows_the_commit_in_the_status() {
        if !git_available() {
            return;
        }
        let (_dir, vault) = vault_with_repo();
        commit(&vault, "First", true).expect("commit");
        git(vault.root(), &["checkout", "--detach", "HEAD"]).expect("detach");
        let state = status(&vault).expect("status");
        let repo = state.repo.expect("repo");
        assert_eq!(repo.branch.len(), 7);
        assert!(repo.branch.chars().all(|c| c.is_ascii_hexdigit()));
        assert!(repo.upstream.is_none());
    }

    #[test]
    fn the_status_reads_the_upstream_and_the_distance() {
        if !git_available() {
            return;
        }
        let (dir, vault) = vault_with_repo();
        commit(&vault, "First", true).expect("commit");
        // A second repository plays the remote, so no network is needed.
        let remote = tempdir().expect("temp dir");
        git(remote.path(), &["init", "--bare", "--initial-branch=main"]).expect("init");
        git(
            vault.root(),
            &["remote", "add", "origin", &remote.path().to_string_lossy()],
        )
        .expect("remote");
        git(vault.root(), &["push", "-u", "origin", "main"]).expect("push");

        let state = status(&vault).expect("status");
        let repo = state.repo.expect("repo");
        assert_eq!(repo.branch, "main");
        assert_eq!(repo.upstream.as_deref(), Some("origin/main"));
        assert_eq!(repo.remote.as_deref(), Some("origin"));
        assert_eq!((repo.ahead, repo.behind), (0, 0));

        // A commit of our own moves ahead; one on the remote moves behind.
        fs::write(dir.path().join("second.md"), "# Second\n").expect("write");
        commit(&vault, "Second", true).expect("commit");
        let state = status(&vault).expect("status");
        let repo = state.repo.expect("repo");
        assert_eq!((repo.ahead, repo.behind), (1, 0));

        let other = tempdir().expect("temp dir");
        git(
            other.path(),
            &["clone", &remote.path().to_string_lossy(), "."],
        )
        .expect("clone");
        git(other.path(), &["config", "user.name", "dvoix"]).expect("config");
        git(other.path(), &["config", "user.email", "dvoix@example.com"]).expect("config");
        fs::write(other.path().join("third.md"), "# Third\n").expect("write");
        git(other.path(), &["add", "third.md"]).expect("add");
        git(other.path(), &["commit", "-m", "Third"]).expect("commit");
        git(other.path(), &["push"]).expect("push");
        git(vault.root(), &["fetch"]).expect("fetch");

        let state = status(&vault).expect("status");
        let repo = state.repo.expect("repo");
        assert_eq!((repo.ahead, repo.behind), (1, 1));
    }

    /// A conflict keeps the file visible in both lists with `U`, as the v1
    /// format showed it; resolving it empties the status again.
    #[test]
    fn a_conflict_is_shown_until_it_is_resolved() {
        if !git_available() {
            return;
        }
        let (dir, vault) = vault_with_repo();
        commit(&vault, "First", true).expect("commit");
        create_branch(&vault, "other").expect("branch");
        fs::write(dir.path().join("note.md"), "# Other\n").expect("write");
        commit(&vault, "Other side", true).expect("commit");
        checkout(&vault, "main").expect("checkout");
        fs::write(dir.path().join("note.md"), "# Main\n").expect("write");
        commit(&vault, "Main side", true).expect("commit");

        // The merge conflicts; that is expected here.
        let _ = git_raw(vault.root(), &["merge", "other"]);
        let state = status(&vault).expect("status");
        assert_eq!(state.staged.len(), 1, "staged: {:?}", state.staged);
        assert_eq!(state.staged[0].path, "note.md");
        assert_eq!(state.staged[0].status, "U");
        assert_eq!(state.unstaged.len(), 1, "unstaged: {:?}", state.unstaged);
        assert_eq!(state.unstaged[0].status, "U");

        // Resolving the file takes it out of both lists again.
        fs::write(dir.path().join("note.md"), "# Resolved\n").expect("write");
        stage(&vault, &["note.md".into()]).expect("stage");
        let state = status(&vault).expect("status");
        assert!(state.unstaged.is_empty());
        assert_eq!(state.staged[0].status, "M");
    }

    /// The remote is the first part of the upstream name; only the fallback
    /// of `pick_remote` needs a call of git.
    #[test]
    fn the_remote_is_the_first_part_of_the_upstream() {
        assert_eq!(
            remote_of(Some("origin/topic/with-slashes")).as_deref(),
            Some("origin")
        );
        assert_eq!(remote_of(Some("main")).as_deref(), None);
        assert_eq!(remote_of(None).as_deref(), None);
    }

    /// `pick_remote` prefers the upstream's remote and only asks git for the
    /// first remote when there is no upstream to name one.
    #[test]
    fn pick_remote_prefers_the_upstream_and_falls_back_to_the_first() {
        if !git_available() {
            return;
        }
        let (dir, _vault) = vault_with_repo();
        // With an upstream the answer needs no call of git: even a path that
        // is no repository gives the upstream's remote.
        assert_eq!(
            pick_remote(Path::new("/no/such/repo"), Some("upstream/main")).as_deref(),
            Some("upstream")
        );
        // Without remotes there is nothing to name.
        assert_eq!(pick_remote(dir.path(), None), None);
        let _backup = add_bare_remote(dir.path(), "backup");
        let _origin = add_bare_remote(dir.path(), "origin");
        assert_eq!(pick_remote(dir.path(), None).as_deref(), Some("backup"));
        assert_eq!(
            pick_remote(dir.path(), Some("origin/main")).as_deref(),
            Some("origin")
        );
        // An upstream without a remote part (a local branch) falls back too.
        assert_eq!(
            pick_remote(dir.path(), Some("main")).as_deref(),
            Some("backup")
        );
    }

    /// Without any remote the status names none, and so do the push/pull
    /// defaults.
    #[test]
    fn a_repository_without_remotes_names_none() {
        if !git_available() {
            return;
        }
        let (_dir, vault) = vault_with_repo();
        commit(&vault, "First", true).expect("commit");
        let state = status(&vault).expect("status");
        assert!(state.repo.expect("repo").remote.is_none());
        let info = repo_info(&Repo::of(&vault).expect("repo")).expect("info");
        assert!(info.remote.is_none());
    }

    /// With two remotes and the upstream on the second one, the status, the
    /// push/pull defaults and a real push all agree on that remote, not on
    /// the first entry of `git remote`.
    #[test]
    fn status_and_push_agree_on_the_remote_of_the_upstream() {
        if !git_available() {
            return;
        }
        let (dir, vault) = vault_with_repo();
        commit(&vault, "First", true).expect("commit");
        // `backup` sorts before `origin`, so the first entry of `git remote`
        // is not the remote of the upstream.
        let backup = add_bare_remote(dir.path(), "backup");
        let origin = add_bare_remote(dir.path(), "origin");
        assert_eq!(
            git(dir.path(), &["remote"])
                .expect("remotes")
                .lines()
                .next(),
            Some("backup")
        );

        // Without an upstream, the status names the first remote, the same
        // one a first push would use.
        let state = status(&vault).expect("status");
        let repo = state.repo.expect("repo");
        assert!(repo.upstream.is_none());
        assert_eq!(repo.remote.as_deref(), Some("backup"));
        let info = repo_info(&Repo::of(&vault).expect("repo")).expect("info");
        assert_eq!(info.remote.as_deref(), Some("backup"));

        // The upstream points at the second remote: both name it now.
        git(dir.path(), &["push", "-u", "origin", "main"]).expect("push");
        let state = status(&vault).expect("status");
        let repo = state.repo.expect("repo");
        assert_eq!(repo.upstream.as_deref(), Some("origin/main"));
        assert_eq!(repo.remote.as_deref(), Some("origin"));
        let info = repo_info(&Repo::of(&vault).expect("repo")).expect("info");
        assert_eq!(info.remote.as_deref(), Some("origin"));

        // A push through the app follows the upstream: the commit lands on
        // `origin`, and `backup` stays empty. A pull uses the same remote.
        fs::write(dir.path().join("second.md"), "# Second\n").expect("write");
        commit(&vault, "Second", true).expect("commit");
        push(&vault).expect("push");
        pull(&vault).expect("pull");
        let head = git(dir.path(), &["rev-parse", "HEAD"]).expect("rev-parse");
        let on_origin = git(origin.path(), &["rev-parse", "main"]).expect("origin main");
        assert_eq!(on_origin.trim(), head.trim());
        let on_backup =
            git_raw(backup.path(), &["rev-parse", "--verify", "main"]).expect("git runs");
        assert!(
            !on_backup.status.success(),
            "the first remote must stay untouched"
        );
    }
}
