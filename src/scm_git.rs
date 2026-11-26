use crate::{i18n::tr, proc, remote::WslTarget};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq)]
pub struct FileChange {
    pub path: String,
    pub code: char,
    pub orig: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct RepoStatus {
    pub head: Option<String>,
    pub branch: Option<String>,
    pub upstream: Option<String>,
    pub upstream_available: bool,
    pub ahead: u32,
    pub behind: u32,
    pub staged: Vec<FileChange>,
    pub changes: Vec<FileChange>,
    pub untracked: Vec<FileChange>,
    pub conflicts: Vec<FileChange>,
}

impl RepoStatus {
    pub fn count(&self) -> usize {
        self.staged.len() + self.changes.len() + self.untracked.len() + self.conflicts.len()
    }

    pub(crate) fn files(&self, group: Group) -> &[FileChange] {
        match group {
            Group::Changes => &self.changes,
            Group::Staged => &self.staged,
            Group::Conflicts => &self.conflicts,
            Group::Untracked => &self.untracked,
        }
    }

    pub(crate) fn check_commit(&self, message: &str, amend: bool) -> Result<(), &'static str> {
        if !self.conflicts.is_empty() {
            Err("Resolve the merge conflicts before committing.")
        } else if message.trim().is_empty() {
            Err("Enter a commit message.")
        } else if amend && self.head.is_none() {
            Err("There is no commit to amend.")
        } else if self.staged.is_empty() {
            Err("Stage changes before committing.")
        } else {
            Ok(())
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Group {
    Changes = 0,
    Staged = 1,
    Conflicts = 2,
    Untracked = 3,
}

pub(crate) fn file_paths(files: &[FileChange]) -> Vec<&str> {
    let mut paths: Vec<_> = files
        .iter()
        .flat_map(|file| {
            [
                Some(file.path.as_str()),
                file.orig.as_deref().filter(|_| file.code == 'R'),
            ]
            .into_iter()
            .flatten()
        })
        .collect();
    paths.sort_unstable();
    paths.dedup();
    paths
}

#[derive(Clone, Debug, PartialEq)]
pub struct Commit {
    pub hash: String,
    pub short: String,
    pub subject: String,
    pub author: String,
    pub when: String,
    pub refs: Vec<String>,
    pub merge: bool,
    pub parents: Vec<String>,
    pub is_head: bool,
}

pub fn parse_status(out: &str) -> RepoStatus {
    let mut st = RepoStatus::default();
    let mut recs = out.split('\0').filter(|r| !r.is_empty());
    let letter = |c: char| {
        if c == '?' {
            'U'
        } else if c == 'T' {
            'M'
        } else {
            c
        }
    };
    while let Some(r) = recs.next() {
        if let Some(oid) = r.strip_prefix("# branch.oid ") {
            st.head = (oid != "(initial)").then(|| oid.to_string());
        } else if let Some(h) = r.strip_prefix("# branch.head ") {
            st.branch = (h != "(detached)").then(|| h.to_string());
        } else if let Some(u) = r.strip_prefix("# branch.upstream ") {
            st.upstream = Some(u.to_string());
        } else if let Some(ab) = r.strip_prefix("# branch.ab ") {
            st.upstream_available = true;
            let mut it = ab.split_whitespace();
            st.ahead = it
                .next()
                .and_then(|a| a.trim_start_matches('+').parse().ok())
                .unwrap_or(0);
            st.behind = it
                .next()
                .and_then(|b| b.trim_start_matches('-').parse().ok())
                .unwrap_or(0);
        } else if let Some(p) = r.strip_prefix("? ") {
            st.untracked.push(FileChange {
                path: p.to_string(),
                code: 'U',
                orig: None,
            });
        } else if r.starts_with("1 ") || r.starts_with("2 ") {
            let renamed = r.starts_with('2');
            let fields: Vec<&str> = r.splitn(if renamed { 10 } else { 9 }, ' ').collect();
            let orig = if renamed {
                recs.next().map(str::to_string)
            } else {
                None
            };
            if fields.len() != if renamed { 10 } else { 9 } {
                continue;
            }
            let mut xy = fields[1].chars();
            let (Some(x), Some(y)) = (xy.next(), xy.next()) else {
                continue;
            };
            let path = fields[fields.len() - 1];
            for (code, group) in [(x, &mut st.staged), (y, &mut st.changes)] {
                if code != '.' {
                    group.push(FileChange {
                        path: path.to_string(),
                        code: letter(code),
                        orig: if matches!(code, 'R' | 'C') {
                            orig.clone()
                        } else {
                            None
                        },
                    });
                }
            }
        } else if r.starts_with("u ") {
            if let Some(path) = r.splitn(11, ' ').nth(10) {
                st.conflicts.push(FileChange {
                    path: path.to_string(),
                    code: 'U',
                    orig: None,
                });
            }
        }
    }
    st
}

pub fn parse_log(out: &str) -> Vec<Commit> {
    out.split('\0')
        .filter(|r| !r.trim().is_empty())
        .filter_map(|r| {
            let f: Vec<&str> = r.trim_start_matches('\n').split('\x1f').collect();
            (f.len() >= 7).then(|| {
                let raw_refs: Vec<&str> = f[5]
                    .split(", ")
                    .filter(|s| !s.is_empty() && *s != "origin/HEAD")
                    .collect();
                let is_head = raw_refs
                    .iter()
                    .any(|s| *s == "HEAD" || s.starts_with("HEAD -> "));
                let refs = raw_refs
                    .into_iter()
                    .filter(|s| *s != "HEAD")
                    .map(|s| s.replace("HEAD -> ", ""))
                    .collect();
                let parents: Vec<String> = f[6].split_whitespace().map(str::to_string).collect();
                let merge = parents.len() > 1;
                Commit {
                    hash: f[0].into(),
                    short: f[1].into(),
                    subject: f[2].into(),
                    author: f[3].into(),
                    when: f[4].into(),
                    refs,
                    merge,
                    parents,
                    is_head,
                }
            })
        })
        .collect()
}

#[derive(Clone)]
pub(crate) enum Step {
    Git(Vec<String>, Option<String>),
    WorkspaceGit(Vec<String>),
    Resolve {
        path: String,
        expected: String,
        resolved: String,
    },
    Hunk {
        path: String,
        group: Group,
        expected: String,
        index: usize,
    },
    Remote(RemoteAction),
    Gh(Vec<String>),
    Commit {
        message: String,
        amend: bool,
        signoff: bool,
    },
    Paths {
        group: Group,
        files: Vec<FileChange>,
        discard: bool,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RemoteAction {
    Fetch,
    Pull,
    Push,
    Sync,
}

pub(crate) const REMOTE_ENV: &[(&str, &str)] = &[
    ("GCM_INTERACTIVE", "0"),
    ("SSH_ASKPASS_REQUIRE", "never"),
    ("GIT_HTTP_LOW_SPEED_LIMIT", "1"),
    ("GIT_HTTP_LOW_SPEED_TIME", "30"),
];

pub(crate) fn git_step(args: &[&str]) -> Step {
    Step::Git(args.iter().map(|s| s.to_string()).collect(), None)
}

pub(crate) fn run_step(
    dir: &Path,
    step: &Step,
    target: Option<&WslTarget>,
) -> Result<String, String> {
    use std::io::Write;
    let (prog, args, input) = match step {
        Step::WorkspaceGit(args) => return run_step(dir, &Step::Git(args.clone(), None), target),
        Step::Resolve {
            path,
            expected,
            resolved,
        } => {
            let full = dir.join(path);
            let status = read_status(dir, target)?;
            if !status.conflicts.iter().any(|file| file.path == *path)
                || !std::fs::symlink_metadata(&full).is_ok_and(|meta| meta.is_file())
                || std::fs::read(&full).map_or(true, |data| data != expected.as_bytes())
            {
                return Err(
                    tr("The conflict changed. Reopen it before saving the resolution.").into(),
                );
            }
            crate::core::write_bytes(&full, resolved.as_bytes())?;
            return run_step(
                dir,
                &git_step(&["--literal-pathspecs", "add", "--", path]),
                target,
            );
        }
        Step::Hunk {
            path,
            group,
            expected,
            index,
        } => {
            return apply_hunk(dir, path, *group, expected, *index, target);
        }
        Step::Remote(action) => return run_remote(dir, *action, target),
        Step::Commit {
            message,
            amend,
            signoff,
        } => return commit(dir, message, *amend, *signoff, target),
        Step::Paths {
            group,
            files,
            discard,
        } => return update_paths(dir, *group, files, *discard, target),
        Step::Git(a, i) => ("git", a, i.clone()),
        Step::Gh(a) => ("gh", a, None),
    };
    let target = matches!(step, Step::Git(..)).then_some(target).flatten();
    let refs: Vec<&str> = args.iter().map(String::as_str).collect();
    let network = prog == "git" && matches!(refs.first(), Some(&("fetch" | "pull" | "push")));
    let mut cmd = match &target {
        Some(t) => {
            let cwd = t.linux_path(dir).unwrap_or_else(|| t.linux_root.clone());
            let owned: Vec<String> = args.iter().map(|s| s.to_string()).collect();
            // The helper can time out before an operation finishes, so network writes are never retried.
            if !network {
                if let Some(r) = crate::remote::with_client(&t.distro.clone(), |c| {
                    c.git(&cwd, &owned, input.as_deref())
                }) {
                    match r {
                        Ok(g) if g.success => {
                            return Ok(if g.stdout.trim().is_empty() {
                                g.stderr.trim().to_string()
                            } else {
                                g.stdout
                            })
                        }
                        Ok(g) => {
                            return Err(format!(
                                "{prog} {}: {}",
                                args.first().map(String::as_str).unwrap_or(""),
                                g.stderr.trim()
                            ))
                        }
                        Err(_) => {}
                    }
                }
            }
            let mut env = vec![
                ("LC_ALL", "C"),
                ("GIT_OPTIONAL_LOCKS", "0"),
                ("GIT_TERMINAL_PROMPT", "0"),
                ("NO_COLOR", "1"),
            ];
            if network {
                env.extend_from_slice(REMOTE_ENV);
            }
            crate::remote::program_command_env(t, &cwd, &env, prog, &refs)
        }
        None => {
            let mut c = proc::command(prog);
            c.args(args).current_dir(dir);
            if network {
                c.envs(REMOTE_ENV.iter().copied());
                if std::env::var_os("GIT_SSH_COMMAND").is_none()
                    && std::env::var_os("GIT_SSH").is_none()
                    && run_step(
                        dir,
                        &git_step(&["config", "--get", "core.sshCommand"]),
                        None,
                    )
                    .is_err()
                {
                    c.env(
                        "GIT_SSH_COMMAND",
                        "ssh -oBatchMode=yes -oConnectTimeout=15 -oServerAliveInterval=15 -oServerAliveCountMax=2",
                    );
                }
            }
            c
        }
    };
    cmd.env("LC_ALL", "C")
        .env("GIT_OPTIONAL_LOCKS", "0")
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GH_PROMPT_DISABLED", "1")
        .env("NO_COLOR", "1")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    crate::github::apply_env(&mut cmd);
    let mut child = cmd
        .spawn()
        .map_err(|e| format!("{prog} could not start: {e}"))?;
    if let Some(mut stdin) = child.stdin.take() {
        if let Some(text) = input {
            let _ = stdin.write_all(text.as_bytes());
        }
    }
    let out = child.wait_with_output().map_err(|e| e.to_string())?;
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
    if out.status.success() {
        Ok(if stdout.trim().is_empty() {
            stderr
        } else {
            stdout
        })
    } else {
        let msg = if stderr.is_empty() {
            stdout.trim().to_string()
        } else {
            stderr
        };
        Err(format!(
            "{prog} {}: {}",
            args.first().map(String::as_str).unwrap_or(""),
            msg
        ))
    }
}

fn remote_error(error: String) -> String {
    let lower = error.to_ascii_lowercase();
    let hint = if lower.contains("non-fast-forward") || lower.contains("(fetch first)") {
        Some(tr(
            "The remote branch has new commits. Pull or Sync before pushing again.",
        ))
    } else if lower.contains("permission denied (publickey")
        || lower.contains("authentication failed")
        || lower.contains("could not read username")
        || lower.contains("could not read password")
        || lower.contains("interactivity has been disabled")
        || lower.contains("host key verification failed")
    {
        Some(tr(
            "Check Git credentials or your SSH connection in a terminal, then retry.",
        ))
    } else {
        None
    };
    match hint {
        Some(hint) => format!("{error}\n{hint}"),
        None => error,
    }
}

fn run_remote(
    dir: &Path,
    action: RemoteAction,
    target: Option<&WslTarget>,
) -> Result<String, String> {
    let git = |args: &[&str]| run_step(dir, &git_step(args), target);
    let remotes = git(&["remote"])?;
    if remotes.trim().is_empty() {
        return Err(tr(
            "No Git remote is configured. Add a remote before fetching, pulling, or pushing.",
        )
        .into());
    }
    if action == RemoteAction::Fetch {
        return git(&["fetch", "--prune"]).map_err(remote_error);
    }
    let mut status = read_status(dir, target)?;
    if status.branch.is_none() {
        return Err(tr("Switch to a branch before pulling, pushing, or syncing.").into());
    }
    if !status.conflicts.is_empty() || git(&["rev-parse", "--verify", "-q", "MERGE_HEAD"]).is_ok() {
        return Err(
            tr("Resolve and finish the merge, or abort it before using the remote.").into(),
        );
    }
    if action == RemoteAction::Pull && status.upstream.is_none() {
        return Err(tr(
            "This branch has no upstream. Set one or publish the branch before pulling.",
        )
        .into());
    }
    if action == RemoteAction::Pull || action == RemoteAction::Sync && status.upstream.is_some() {
        let pulled = git(&["pull", "--autostash", "--no-edit"]).map_err(remote_error)?;
        status = read_status(dir, target)?;
        if !status.conflicts.is_empty() {
            return Err(
                tr("Pull left unresolved conflicts. Resolve them before continuing.").into(),
            );
        }
        if action == RemoteAction::Pull {
            return Ok(pulled);
        }
    }
    if status.head.is_none() {
        return Err(tr("Create a commit before publishing this branch.").into());
    }
    if status.upstream.is_some() {
        git(&["push"]).map_err(remote_error)
    } else if remotes.lines().any(|remote| remote == "origin") {
        git(&["push", "-u", "origin", "HEAD"]).map_err(remote_error)
    } else {
        Err(tr("No origin remote is configured. Set the branch upstream before pushing.").into())
    }
}

fn read_status(dir: &Path, target: Option<&WslTarget>) -> Result<RepoStatus, String> {
    run_step(
        dir,
        &git_step(&["status", "--porcelain=v2", "--branch", "-z", "-uall"]),
        target,
    )
    .map(|s| parse_status(&s))
}

pub(crate) fn file_diff(
    dir: &Path,
    file: &FileChange,
    group: Group,
    target: Option<&WslTarget>,
) -> Result<(String, Option<String>, Option<PathBuf>), String> {
    use std::io::Read as _;

    let full = dir.join(&file.path);
    let open_path = full.is_file().then(|| full.clone());
    if group == Group::Untracked {
        let limit = 2 * 1024 * 1024;
        let mut bytes = Vec::new();
        let meta = full.symlink_metadata().map_err(|e| e.to_string())?;
        if meta.is_symlink() {
            bytes = full
                .read_link()
                .map_err(|e| e.to_string())?
                .to_string_lossy()
                .as_bytes()
                .to_vec();
        } else if meta.is_file() {
            std::fs::File::open(&full)
                .and_then(|file| file.take(limit + 1).read_to_end(&mut bytes))
                .map_err(|e| e.to_string())?;
        }
        let patch = if bytes.len() as u64 <= limit {
            crate::diff_view::untracked_patch(&file.path, &bytes).unwrap_or_default()
        } else {
            return Err(tr("The untracked file is too large to preview.").to_string());
        };
        return Ok((patch, None, open_path));
    }
    let conflict = (group == Group::Conflicts)
        .then(|| std::fs::read_to_string(&full).ok())
        .flatten()
        .filter(|text| text.lines().any(|line| line.starts_with("<<<<<<<")));
    let mut args = vec![
        "--literal-pathspecs",
        "-c",
        "core.quotepath=off",
        "diff",
        "--no-color",
        "--no-ext-diff",
        "--no-textconv",
        "--no-relative",
        "--src-prefix=a/",
        "--dst-prefix=b/",
        "-U3",
    ];
    match group {
        Group::Staged => args.push("--cached"),
        Group::Conflicts => args.push("--ours"),
        _ => {}
    }
    args.push("--");
    args.extend(file_paths(std::slice::from_ref(file)));
    let patch = run_step(dir, &git_step(&args), target)?;
    Ok((patch, conflict, open_path))
}

fn patch_hunk(patch: &str, index: usize) -> Option<String> {
    let mut hunks = Vec::new();
    let mut offset = 0;
    let mut files = 0;
    for line in patch.split_inclusive('\n') {
        if line.starts_with("diff --git ") {
            files += 1;
        }
        if line.starts_with("@@ ") {
            hunks.push(offset);
        }
        offset += line.len();
    }
    if files != 1 {
        return None;
    }
    let start = *hunks.get(index)?;
    let end = hunks.get(index + 1).copied().unwrap_or(patch.len());
    Some(format!("{}{}", &patch[..hunks[0]], &patch[start..end]))
}

fn apply_hunk(
    dir: &Path,
    path: &str,
    group: Group,
    expected: &str,
    index: usize,
    target: Option<&WslTarget>,
) -> Result<String, String> {
    let stale =
        || tr("The diff changed. Reopen it before staging or unstaging a hunk.").to_string();
    if !matches!(group, Group::Changes | Group::Staged) {
        return Err(stale());
    }
    let status = read_status(dir, target)?;
    let file = status
        .files(group)
        .iter()
        .find(|file| file.path == path && file.code == 'M')
        .ok_or_else(stale)?;
    let (current, _, _) = file_diff(dir, file, group, target)?;
    if current != expected {
        return Err(stale());
    }
    let patch = patch_hunk(&current, index).ok_or_else(stale)?;
    let mut args = vec!["apply".into(), "--cached".into()];
    if group == Group::Staged {
        args.push("--reverse".into());
    }
    args.push("-".into());
    run_step(dir, &Step::Git(args, Some(patch)), target)
}

fn commit(
    dir: &Path,
    message: &str,
    amend: bool,
    signoff: bool,
    target: Option<&WslTarget>,
) -> Result<String, String> {
    read_status(dir, target)?
        .check_commit(message, amend)
        .map_err(|key| tr(key).to_string())?;
    for identity in ["GIT_AUTHOR_IDENT", "GIT_COMMITTER_IDENT"] {
        run_step(
            dir,
            &git_step(&["-c", "user.useConfigOnly=true", "var", identity]),
            target,
        )
        .map_err(|_| tr("Configure Git user.name and user.email before committing.").to_string())?;
    }
    let mut args = vec!["commit".into(), "--quiet".into(), "--file=-".into()];
    if amend {
        args.push("--amend".into());
    }
    if signoff {
        args.push("--signoff".into());
    }
    run_step(dir, &Step::Git(args, Some(message.to_string())), target)
}

fn update_paths(
    dir: &Path,
    group: Group,
    files: &[FileChange],
    discard: bool,
    target: Option<&WslTarget>,
) -> Result<String, String> {
    let status = read_status(dir, target)?;
    let current = status.files(group);
    let selected: std::collections::HashMap<_, _> = current
        .iter()
        .map(|file| (file.path.as_str(), file))
        .collect();
    if files.iter().any(|file| {
        selected
            .get(file.path.as_str())
            .is_none_or(|current| discard && *current != file)
    }) {
        return Err(
            tr("The selected files changed. Review Source Control and try again.").to_string(),
        );
    }
    let paths = file_paths(files);
    if paths.is_empty() {
        return Ok(String::new());
    }
    let args: &[&str] = match (group, discard) {
        (Group::Staged, false) if status.head.is_none() => &["rm", "--cached", "-f"],
        (Group::Staged, false) => &["restore", "--staged"],
        (_, false) => &["add", "-A"],
        (Group::Untracked, true) => {
            if paths
                .iter()
                .any(|path| dir.join(path).symlink_metadata().is_ok_and(|m| m.is_dir()))
            {
                return Err(
                    tr("Discard untracked files individually inside nested repositories.")
                        .to_string(),
                );
            }
            // Git clean only accepts pathspecs as arguments, which are size-limited on Windows.
            for path in paths {
                run_step(
                    dir,
                    &git_step(&["--literal-pathspecs", "clean", "-f", "--", path]),
                    target,
                )?;
            }
            return Ok(String::new());
        }
        (Group::Changes, true) => &["restore", "--worktree"],
        (Group::Conflicts, true) => &["restore", "--merge", "--worktree"],
        (Group::Staged, true) => {
            if status
                .changes
                .iter()
                .chain(&status.untracked)
                .chain(&status.conflicts)
                .any(|file| {
                    file_paths(std::slice::from_ref(file))
                        .iter()
                        .any(|p| paths.contains(p))
                })
            {
                return Err(
                    tr("Discard or stage the working tree changes to these files first.")
                        .to_string(),
                );
            }
            if status.head.is_none() {
                &["rm"]
            } else {
                &["restore", "--staged", "--worktree"]
            }
        }
    };
    let mut argv = vec!["--literal-pathspecs".into()];
    argv.extend(args.iter().map(|s| s.to_string()));
    argv.extend([
        "--pathspec-from-file=-".into(),
        "--pathspec-file-nul".into(),
    ]);
    let mut input = paths.join("\0");
    input.push('\0');
    run_step(dir, &Step::Git(argv, Some(input)), target)
}

#[derive(Default)]
pub(crate) struct Snapshot {
    pub repo: Option<PathBuf>,
    pub watch_paths: Vec<PathBuf>,
    pub status: RepoStatus,
    pub log: Option<Vec<Commit>>,
    pub incoming: (Vec<Commit>, Option<String>),
}

pub(crate) fn snapshot(
    dir: &Path,
    with_log: bool,
    target: Option<&WslTarget>,
) -> Result<Snapshot, String> {
    let resolve = |p: &str| match target {
        Some(t) => t.unc_path(p),
        None => PathBuf::from(p),
    };
    let top = match run_step(dir, &git_step(&["rev-parse", "--show-toplevel"]), target) {
        Ok(top) => top,
        Err(error) if error.contains("not a git repository") => {
            return Ok(Snapshot {
                watch_paths: vec![dir.to_path_buf()],
                ..Snapshot::default()
            });
        }
        Err(error) => return Err(error),
    };
    let repo = resolve(top.trim_end_matches(['\r', '\n']));
    let status = read_status(&repo, target)?;
    let mut watch_paths = vec![repo.clone()];
    if let Ok(paths) = run_step(
        &repo,
        &git_step(&[
            "rev-parse",
            "--path-format=absolute",
            "--git-dir",
            "--git-common-dir",
        ]),
        target,
    ) {
        for path in paths.lines().map(resolve) {
            if !watch_paths.iter().any(|root| path.starts_with(root)) {
                watch_paths.push(path);
            }
        }
    }
    let log = with_log.then(|| {
        run_step(
            &repo,
            &git_step(&[
                "-c",
                "i18n.logOutputEncoding=UTF-8",
                "log",
                "-n",
                "80",
                "--topo-order",
                "--format=%H%x1f%h%x1f%s%x1f%an%x1f%ar%x1f%D%x1f%P",
                "-z",
            ]),
            target,
        )
        .map(|s| parse_log(&s))
        .unwrap_or_default()
    });
    let incoming = if status.behind > 0 {
        crate::scm_incoming::incoming(&repo, target)
    } else {
        Default::default()
    };
    Ok(Snapshot {
        repo: Some(repo),
        watch_paths,
        status,
        log,
        incoming,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn porcelain_v2() {
        let out = "# branch.oid abc\0# branch.head main\0# branch.upstream origin/main\0# branch.ab +2 -21\0\
1 .M N... 100644 100644 100644 a b src/app.py\0\
1 A. N... 000000 100644 100644 0 b new.txt\0\
1 MM N... 100644 100644 100644 a b both.rs\0\
2 R. N... 100644 100644 100644 a b R100 renamed.py\0old.py\0\
? notes.md\0u UU N... 100644 100644 100644 100644 a b c conflict.txt\0";
        let st = parse_status(out);
        assert_eq!(st.branch.as_deref(), Some("main"));
        assert_eq!(st.upstream.as_deref(), Some("origin/main"));
        assert!(st.upstream_available);
        assert_eq!((st.ahead, st.behind), (2, 21));
        assert_eq!(
            st.staged
                .iter()
                .map(|c| (c.path.as_str(), c.code))
                .collect::<Vec<_>>(),
            vec![("new.txt", 'A'), ("both.rs", 'M'), ("renamed.py", 'R')]
        );
        assert_eq!(st.staged[2].orig.as_deref(), Some("old.py"));
        assert_eq!(
            st.changes
                .iter()
                .map(|c| (c.path.as_str(), c.code))
                .collect::<Vec<_>>(),
            vec![("src/app.py", 'M'), ("both.rs", 'M')]
        );
        assert_eq!(st.untracked[0].path, "notes.md");
        assert_eq!(st.conflicts[0].code, 'U');
        assert_eq!(st.conflicts[0].path, "conflict.txt");
        assert_eq!(st.count(), 7);
    }

    #[test]
    fn spaces_in_paths() {
        let st = parse_status("1 .M N... 100644 100644 100644 a b my folder/a file.py\0");
        assert_eq!(st.changes[0].path, "my folder/a file.py");
    }

    #[test]
    fn hunk_stage_and_unstage_preserve_other_edits() {
        let repo = TestRepo::new();
        let before = (1..=25).map(|n| format!("line {n}\n")).collect::<String>();
        repo.write("file.txt", &before);
        repo.change(Group::Untracked, false).unwrap();
        commit(&repo.0, "initial", false, true, None).unwrap();
        assert!(repo
            .git(&["log", "-1", "--format=%B"])
            .contains("Signed-off-by:"));
        let after = before
            .replace("line 2\n", "first edit\n")
            .replace("line 23\n", "second edit\n");
        repo.write("file.txt", &after);
        let patch = file_diff(&repo.0, &repo.status().changes[0], Group::Changes, None)
            .unwrap()
            .0;
        apply_hunk(&repo.0, "file.txt", Group::Changes, &patch, 1, None).unwrap();
        assert!(apply_hunk(&repo.0, "file.txt", Group::Changes, &patch, 0, None).is_err());
        let staged = file_diff(&repo.0, &repo.status().staged[0], Group::Staged, None)
            .unwrap()
            .0;
        assert!(staged.contains("+second edit"));
        assert!(!staged.contains("+first edit"));
        apply_hunk(&repo.0, "file.txt", Group::Staged, &staged, 0, None).unwrap();
        assert!(repo.status().staged.is_empty());
        assert_eq!(
            std::fs::read_to_string(repo.0.join("file.txt")).unwrap(),
            after
        );
    }

    #[test]
    fn remote_error_keeps_git_diagnostics_and_adds_the_next_step() {
        let rejected = "git push: ! [rejected] main -> main (fetch first)";
        let explained = remote_error(rejected.into());
        assert!(explained.starts_with(rejected));
        assert!(explained.ends_with(tr(
            "The remote branch has new commits. Pull or Sync before pushing again."
        )));
        let auth = remote_error("Permission denied (publickey).".into());
        assert!(auth.ends_with(tr(
            "Check Git credentials or your SSH connection in a terminal, then retry."
        )));
        assert_eq!(
            remote_error("Could not resolve host".into()),
            "Could not resolve host"
        );
    }

    struct TestRepo(PathBuf);

    impl TestRepo {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "fluxcode-scm-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            std::fs::create_dir(&path).unwrap();
            let repo = Self(path);
            assert!(snapshot(&repo.0, false, None).unwrap().repo.is_none());
            repo.git(&["init", "-b", "main", "--template="]);
            repo.git(&["config", "user.name", "FluxCode Test"]);
            repo.git(&["config", "user.email", "fluxcode@example.invalid"]);
            repo.git(&["config", "commit.gpgsign", "false"]);
            repo
        }

        fn git(&self, args: &[&str]) -> String {
            run_step(&self.0, &git_step(args), None).unwrap()
        }

        fn status(&self) -> RepoStatus {
            read_status(&self.0, None).unwrap()
        }

        fn write(&self, file: &str, text: &str) {
            std::fs::write(self.0.join(file), text).unwrap();
        }

        fn change(&self, group: Group, discard: bool) -> Result<String, String> {
            update_paths(&self.0, group, self.status().files(group), discard, None)
        }
    }

    impl Drop for TestRepo {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn remote_preflight_explains_missing_remote_origin_and_upstream() {
        let repo = TestRepo::new();
        for action in [
            RemoteAction::Fetch,
            RemoteAction::Pull,
            RemoteAction::Push,
            RemoteAction::Sync,
        ] {
            assert_eq!(run_remote(&repo.0, action, None).unwrap_err(),
                tr("No Git remote is configured. Add a remote before fetching, pulling, or pushing."));
        }
        repo.git(&[
            "remote",
            "add",
            "alternate",
            "https://example.invalid/project",
        ]);
        repo.write("staged.txt", "first\n");
        repo.change(Group::Untracked, false).unwrap();
        commit(&repo.0, "first", false, false, None).unwrap();
        assert_eq!(
            run_remote(&repo.0, RemoteAction::Pull, None).unwrap_err(),
            tr("This branch has no upstream. Set one or publish the branch before pulling.")
        );
        assert_eq!(
            run_remote(&repo.0, RemoteAction::Sync, None).unwrap_err(),
            tr("No origin remote is configured. Set the branch upstream before pushing.")
        );
    }

    #[test]
    fn initial_branch_requires_stage_and_message_and_can_unstage() {
        let repo = TestRepo::new();
        assert!(!repo.status().check_commit("first", false).is_ok());
        assert!(repo.status().check_commit("amend", true).is_err());
        repo.write("a[1].txt", "one\n");
        repo.write("a1.txt", "two\n");
        let before = repo.status();
        let one = before
            .untracked
            .iter()
            .find(|file| file.path == "a[1].txt")
            .unwrap();
        update_paths(
            &repo.0,
            Group::Untracked,
            std::slice::from_ref(one),
            false,
            None,
        )
        .unwrap();
        assert_eq!(repo.status().staged.len(), 1);
        assert!(repo.status().check_commit(" ", false).is_err());
        assert!(repo.status().check_commit("first", false).is_ok());
        assert!(repo.status().head.is_none());
        repo.change(Group::Staged, false).unwrap();
        assert_eq!(repo.status().untracked.len(), 2);
        repo.change(Group::Untracked, false).unwrap();
        commit(&repo.0, "first", false, false, None).unwrap();
        assert!(repo.status().head.is_some());
        assert_eq!(repo.status().count(), 0);
        repo.git(&["checkout", "--detach"]);
        repo.write("a1.txt", "amended\n");
        repo.change(Group::Changes, false).unwrap();
        assert!(repo.status().branch.is_none());
        commit(&repo.0, "amended", true, false, None).unwrap();
        assert_eq!(repo.git(&["log", "-1", "--format=%s"]).trim(), "amended");
    }

    #[test]
    fn rename_and_discard_keep_index_and_worktree_separate() {
        let repo = TestRepo::new();
        repo.write("old name.txt", "original\n");
        repo.change(Group::Untracked, false).unwrap();
        commit(&repo.0, "first", false, false, None).unwrap();
        repo.git(&["mv", "old name.txt", "new name.txt"]);
        let status = repo.status();
        let renamed = &status.staged[0];
        assert_eq!(renamed.orig.as_deref(), Some("old name.txt"));
        let (patch, _, _) = file_diff(&repo.0, renamed, Group::Staged, None).unwrap();
        assert!(patch.contains("rename from old name.txt\nrename to new name.txt"));
        repo.write("new name.txt", "worktree edit\n");
        repo.write("untracked.txt", "untracked\n");
        assert!(repo.change(Group::Staged, true).is_err());
        assert_eq!(repo.status().staged[0].code, 'R');
        let unstaged = repo.status();
        assert_eq!(unstaged.changes[0].orig, None);
        let (patch, _, _) = file_diff(&repo.0, &unstaged.changes[0], Group::Changes, None).unwrap();
        assert!(patch.contains("-original\n+worktree edit"));
        repo.change(Group::Changes, true).unwrap();
        assert!(repo.0.join("untracked.txt").is_file());
        repo.change(Group::Staged, true).unwrap();
        assert!(repo.0.join("old name.txt").is_file());
        assert!(!repo.0.join("new name.txt").exists());
        repo.change(Group::Untracked, true).unwrap();
        assert_eq!(repo.status().count(), 0);
    }

    #[test]
    fn resetting_conflict_edits_keeps_the_index_unmerged() {
        let repo = TestRepo::new();
        repo.write("same.txt", "base\n");
        repo.change(Group::Untracked, false).unwrap();
        commit(&repo.0, "base", false, false, None).unwrap();
        repo.git(&["checkout", "-b", "topic"]);
        repo.write("same.txt", "topic\n");
        repo.change(Group::Changes, false).unwrap();
        commit(&repo.0, "topic", false, false, None).unwrap();
        repo.git(&["checkout", "main"]);
        repo.write("same.txt", "main\n");
        repo.change(Group::Changes, false).unwrap();
        commit(&repo.0, "main", false, false, None).unwrap();
        assert!(run_step(&repo.0, &git_step(&["merge", "--no-edit", "topic"]), None).is_err());
        let unmerged = repo.status();
        assert_eq!(unmerged.conflicts.len(), 1);
        assert!(unmerged.check_commit("merge", false).is_err());
        repo.write("same.txt", "candidate resolution\n");
        repo.change(Group::Conflicts, true).unwrap();
        let restored = std::fs::read_to_string(repo.0.join("same.txt")).unwrap();
        assert!(restored.contains("<<<<<<<"));
        assert_eq!(repo.status().conflicts.len(), 1);
        let save = |expected: String| Step::Resolve {
            path: "same.txt".into(),
            expected,
            resolved: "resolved\n".into(),
        };
        assert!(run_step(&repo.0, &save("stale".into()), None).is_err());
        assert_eq!(
            std::fs::read_to_string(repo.0.join("same.txt")).unwrap(),
            restored
        );
        run_step(&repo.0, &save(restored), None).unwrap();
        assert!(repo.status().conflicts.is_empty());
        assert_eq!(repo.git(&["show", ":same.txt"]), "resolved\n");
        assert!(repo.status().check_commit("merge", false).is_ok());
    }
}
