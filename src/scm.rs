//! Source Control, natively: the same view as VS Code's (changes, commit box,
//! push/pull/sync, graph) plus GitHub pull requests and Actions "deploys".
//!
//! VS Code's git extension (checked in its shipped `extensions/git/dist/main.js`)
//! runs `git status -z -uall`, then `for-each-ref` and `rev-list --left-right
//! --count` for the branch and ahead/behind. Here one
//! `git status --porcelain=v2 --branch -z -uall` gives all of that in a single
//! process, and the log and GitHub calls only run while their section is open.
//! Commits use `git commit --quiet --file -` (message on stdin) like VS Code,
//! including its "smart commit": with nothing staged, everything is committed.

use crate::i18n::tr;
use crate::proc;
use crate::theme::{self, c};
use crate::workbench::*;
use gpui_kit::assets::IconName as Ico;
use gpui_kit::component::{
    button::{Button, ButtonVariants},
    h_flex,
    input::{Input, InputEvent, InputState},
    menu::{DropdownMenu, PopupMenu},
    v_flex, Icon, IconName, Sizable,
};
use gpui_kit::{prelude::FluentBuilder, *};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use gpui_kit::component::tooltip::Tooltip;

#[derive(Clone, Debug, PartialEq)]
pub struct FileChange {
    /// Repository-relative, with `/`.
    pub path: String,
    /// VS Code's letters: M modified, A added, D deleted, R renamed, U untracked, ! conflict.
    pub code: char,
    pub orig: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct RepoStatus {
    pub branch: Option<String>,
    pub upstream: Option<String>,
    pub ahead: u32,
    pub behind: u32,
    pub staged: Vec<FileChange>,
    pub changes: Vec<FileChange>,
    pub conflicts: Vec<FileChange>,
}

impl RepoStatus {
    pub fn count(&self) -> usize {
        self.staged.len() + self.changes.len() + self.conflicts.len()
    }
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
}

#[derive(Clone, Debug)]
pub struct Run {
    pub title: String,
    pub workflow: String,
    pub status: String,
    pub conclusion: String,
    pub branch: String,
    pub url: String,
}

#[derive(Clone, Debug)]
pub struct Workflow {
    pub id: String,
    pub name: String,
}

/// What to do after a commit (VS Code's `git.postCommitCommand`).
#[derive(Clone, Copy, PartialEq)]
pub enum After {
    Nothing,
    Push,
    Sync,
}

pub struct Scm {
    /// Top level of the repository, if the open folder is in one.
    pub repo: Option<PathBuf>,
    pub status: RepoStatus,
    pub log: Vec<Commit>,
    pub runs: Vec<Run>,
    pub workflows: Vec<Workflow>,
    pub busy: Option<SharedString>,
    pub error: Option<String>,
    /// Last success worth showing (a PR link, "Pushed to origin/main").
    pub note: Option<(String, Option<String>)>,
    pub message: Entity<InputState>,
    refreshing: bool,
    again: bool,
    pub show_graph: bool,
    pub show_actions: bool,
    /// The workflow list came back (possibly empty or with an error).
    pub actions_loaded: bool,
    /// File whose Discard button was clicked once (the second click discards).
    pub confirm_discard: Option<String>,
    /// "Remove Git from this folder" was chosen and waits for a yes.
    pub confirm_uninit: bool,
    /// Lower-cased absolute path → letter, for the explorer (shared with its row closure).
    pub marks: Rc<HashMap<String, char>>,
    /// Lower-cased folders that contain changes.
    pub dirty_dirs: Rc<HashSet<String>>,
    /// A commit went through: empty the message box on the next render (needs a Window).
    pub clear_message: bool,
    /// When the last refresh started (focus changes refresh at most every 2 s).
    pub last_refresh: Option<std::time::Instant>,
    /// The upstream's commits this checkout doesn't have yet, newest first.
    pub incoming: Vec<Commit>,
    /// The upstream's head when `incoming` was read.
    pub incoming_head: Option<String>,
    /// "Not now" was chosen at this upstream head: hidden until it moves.
    pub incoming_dismissed: Option<String>,
    /// The Incoming section is open (its header folds it like the others).
    pub incoming_shown: bool,
    /// The incoming commit opened to its files.
    pub incoming_open: Option<String>,
    /// Hash → (letter, path) of the files it changed, read when opened.
    pub incoming_files: HashMap<String, Vec<(char, String)>>,
    /// A background fetch is running.
    pub fetching: bool,
    /// The background fetch loop has been started.
    pub fetch_loop: bool,
    pub last_fetch: Option<std::time::Instant>,
}

impl Scm {
    pub fn new(window: &mut Window, cx: &mut Context<Workbench>) -> (Self, Subscription) {
        let message = cx.new(|cx| InputState::new(window, cx).placeholder(tr("Commit message")));
        let sub = cx.subscribe_in(&message, window, |this: &mut Workbench, _, ev: &InputEvent, window, cx| {
            if let InputEvent::PressEnter { .. } = ev {
                this.scm_commit(After::Nothing, false, window, cx);
            }
        });
        (
            Self {
                repo: None,
                status: RepoStatus::default(),
                log: Vec::new(),
                runs: Vec::new(),
                workflows: Vec::new(),
                busy: None,
                error: None,
                note: None,
                message,
                refreshing: false,
                again: false,
                show_graph: true,
                show_actions: false,
                actions_loaded: false,
                confirm_discard: None,
                confirm_uninit: false,
                marks: Rc::default(),
                dirty_dirs: Rc::default(),
                clear_message: false,
                last_refresh: None,
                incoming: Vec::new(),
                incoming_head: None,
                incoming_dismissed: None,
                incoming_shown: true,
                incoming_open: None,
                incoming_files: HashMap::new(),
                fetching: false,
                fetch_loop: false,
                last_fetch: None,
            },
            sub,
        )
    }

}

/// Theme color for a git letter (same meaning everywhere: explorer, list, badges).
pub fn code_color(code: char) -> theme::Tok {
    match code {
        'A' | 'U' => theme::GREEN,
        'D' | '!' => theme::DANGER,
        'R' | 'C' => theme::BLUE,
        _ => theme::WARN,
    }
}

// ------------------------------------------------------------ parsing

/// `git status --porcelain=v2 --branch -z -uall` → files and branch state.
pub fn parse_status(out: &str) -> RepoStatus {
    let mut st = RepoStatus::default();
    let mut recs = out.split('\0').filter(|r| !r.is_empty());
    let letter = |c: char| if c == '?' { 'U' } else if c == 'T' { 'M' } else { c };
    while let Some(r) = recs.next() {
        if let Some(h) = r.strip_prefix("# branch.head ") {
            st.branch = (h != "(detached)").then(|| h.to_string());
        } else if let Some(u) = r.strip_prefix("# branch.upstream ") {
            st.upstream = Some(u.to_string());
        } else if let Some(ab) = r.strip_prefix("# branch.ab ") {
            let mut it = ab.split_whitespace();
            st.ahead = it.next().and_then(|a| a.trim_start_matches('+').parse().ok()).unwrap_or(0);
            st.behind = it.next().and_then(|b| b.trim_start_matches('-').parse().ok()).unwrap_or(0);
        } else if let Some(p) = r.strip_prefix("? ") {
            st.changes.push(FileChange { path: p.to_string(), code: 'U', orig: None });
        } else if r.starts_with("1 ") || r.starts_with("2 ") {
            let renamed = r.starts_with('2');
            let fields: Vec<&str> = r.splitn(if renamed { 10 } else { 9 }, ' ').collect();
            let xy: Vec<char> = fields.get(1).unwrap_or(&"..").chars().collect();
            let path = fields.last().unwrap_or(&"").to_string();
            let orig = if renamed { recs.next().map(str::to_string) } else { None };
            if xy[0] != '.' {
                st.staged.push(FileChange { path: path.clone(), code: letter(xy[0]), orig: orig.clone() });
            }
            if xy.get(1).is_some_and(|y| *y != '.') {
                st.changes.push(FileChange { path, code: letter(xy[1]), orig });
            }
        } else if r.starts_with("u ") {
            let path = r.splitn(11, ' ').last().unwrap_or("").to_string();
            st.conflicts.push(FileChange { path, code: '!', orig: None });
        }
    }
    st
}

/// `git log --format=%H%x1f%h%x1f%s%x1f%an%x1f%ar%x1f%D%x1f%P -z`.
pub fn parse_log(out: &str) -> Vec<Commit> {
    out.split('\0')
        .filter(|r| !r.trim().is_empty())
        .filter_map(|r| {
            let f: Vec<&str> = r.trim_start_matches('\n').split('\x1f').collect();
            (f.len() >= 7).then(|| Commit {
                hash: f[0].into(),
                short: f[1].into(),
                subject: f[2].into(),
                author: f[3].into(),
                when: f[4].into(),
                refs: f[5].split(", ").filter(|s| !s.is_empty() && *s != "origin/HEAD").map(|s| s.replace("HEAD -> ", "")).collect(),
                merge: f[6].split_whitespace().count() > 1,
            })
        })
        .collect()
}

/// The first `https://github.com/...` URL in a message (gh prints the PR link).
fn find_url(s: &str) -> Option<String> {
    let i = s.find("https://")?;
    Some(s[i..].split_whitespace().next()?.trim_end_matches(['.', ',', ')']).to_string())
}

// ------------------------------------------------------------ running git / gh

#[derive(Clone)]
pub enum Step {
    Git(Vec<String>, Option<String>),
    Gh(Vec<String>),
}

fn git_step(args: &[&str]) -> Step {
    Step::Git(args.iter().map(|s| s.to_string()).collect(), None)
}

fn run_step(dir: &Path, step: &Step) -> Result<String, String> {
    use std::io::Write;
    let (prog, args, input) = match step {
        Step::Git(a, i) => ("git", a, i.clone()),
        Step::Gh(a) => ("gh", a, None),
    };
    // `git` runs inside the distro for a WSL folder, so it sees Linux paths and
    // line endings. `gh` stays on Windows: it is a network tool, and it finds the
    // repo through the UNC path.
    let target = matches!(step, Step::Git(..)).then(|| crate::remote::WslTarget::from_path(dir)).flatten();
    let mut cmd = match &target {
        Some(t) => {
            let cwd = t.linux_path(dir).unwrap_or_else(|| t.linux_root.clone());
            let refs: Vec<&str> = args.iter().map(String::as_str).collect();
            crate::remote::program_command_env(
                t,
                &cwd,
                &[("GIT_OPTIONAL_LOCKS", "0"), ("GIT_TERMINAL_PROMPT", "0"), ("NO_COLOR", "1")],
                prog,
                &refs,
            )
        }
        None => {
            let mut c = proc::command(prog);
            c.args(args).current_dir(dir);
            c
        }
    };
    cmd.env("GIT_OPTIONAL_LOCKS", "0")
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GH_PROMPT_DISABLED", "1")
        .env("NO_COLOR", "1")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    crate::github::apply_env(&mut cmd);
    let mut child = cmd.spawn().map_err(|e| format!("{prog} could not start: {e}"))?;
    if let Some(mut stdin) = child.stdin.take() {
        if let Some(text) = input {
            let _ = stdin.write_all(text.as_bytes());
        }
    }
    let out = child.wait_with_output().map_err(|e| e.to_string())?;
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
    if out.status.success() {
        // git push / gh pr create report useful lines on stderr.
        Ok(if stdout.trim().is_empty() { stderr } else { stdout })
    } else {
        let msg = if stderr.is_empty() { stdout.trim().to_string() } else { stderr };
        Err(format!("{prog} {}: {}", args.first().map(String::as_str).unwrap_or(""), msg))
    }
}

/// Everything the view shows, read off the UI thread.
struct Snapshot {
    repo: Option<PathBuf>,
    status: RepoStatus,
    log: Option<Vec<Commit>>,
    /// Commits on the upstream and not here, and the upstream's head (only when behind).
    incoming: (Vec<Commit>, Option<String>),
}

fn snapshot(dir: &Path, with_log: bool) -> Snapshot {
    let top = run_step(dir, &git_step(&["rev-parse", "--show-toplevel"])).ok().map(|s| {
        let p = s.trim();
        match crate::remote::WslTarget::from_path(dir) {
            Some(t) => t.unc_path(p),
            None => PathBuf::from(p.replace('/', "\\")),
        }
    });
    let Some(repo) = top else { return Snapshot { repo: None, status: RepoStatus::default(), log: None, incoming: Default::default() } };
    let status = run_step(&repo, &git_step(&["status", "--porcelain=v2", "--branch", "-z", "-uall"])).map(|s| parse_status(&s)).unwrap_or_default();
    let log = with_log.then(|| {
        run_step(
            &repo,
            &git_step(&["-c", "i18n.logOutputEncoding=UTF-8", "log", "-n", "60", "--format=%H%x1f%h%x1f%s%x1f%an%x1f%ar%x1f%D%x1f%P", "-z"]),
        )
        .map(|s| parse_log(&s))
        .unwrap_or_default()
    });
    let incoming = if status.behind > 0 { crate::scm_incoming::incoming(&repo) } else { Default::default() };
    Snapshot { repo: Some(repo), status, log, incoming }
}

/// Rows drawn per section. Thousands of rows (each with its buttons) made the
/// panel slow; past this the list says how many more there are.
const MAX_ROWS: usize = 200;

/// "… and N more" under a capped section.
fn more_rows(total: usize) -> Option<Div> {
    (total > MAX_ROWS).then(|| {
        div()
            .px(theme::u(18.))
            .py_1()
            .text_xs()
            .text_color(c(theme::FG_DIM))
            .child(crate::trf!("… and {n} more files (Stage All includes them)"; n => total - MAX_ROWS))
    })
}

impl Workbench {
    /// Re-read git state (debounced: a refresh while one runs queues one more).
    pub(crate) fn scm_refresh(&mut self, cx: &mut Context<Self>) {
        let Some(root) = self.root.clone() else { return };
        if self.scm.refreshing {
            self.scm.again = true;
            return;
        }
        self.scm.refreshing = true;
        self.scm.last_refresh = Some(std::time::Instant::now());
        let with_log = self.scm.show_graph;
        cx.spawn(async move |this, cx| {
            let snap = cx.background_spawn(async move { snapshot(&root, with_log) }).await;
            _ = this.update(cx, |this, cx| {
                let s = &mut this.scm;
                s.refreshing = false;
                let mut marks = HashMap::new();
                let mut dirty_dirs = HashSet::new();
                if let Some(repo) = &snap.repo {
                    for ch in snap.status.changes.iter().chain(&snap.status.staged).chain(&snap.status.conflicts) {
                        let full = repo.join(ch.path.replace('/', "\\"));
                        let key = full.to_string_lossy().to_lowercase();
                        // A file both staged and changed shows its working-tree letter.
                        marks.entry(key).or_insert(ch.code);
                        let mut p = full.parent();
                        while let Some(d) = p {
                            if !d.starts_with(repo) {
                                break;
                            }
                            dirty_dirs.insert(d.to_string_lossy().to_lowercase());
                            p = d.parent();
                        }
                    }
                }
                s.marks = Rc::new(marks);
                s.dirty_dirs = Rc::new(dirty_dirs);
                s.repo = snap.repo;
                s.status = snap.status;
                if let Some(log) = snap.log {
                    s.log = log;
                }
                let (incoming, head) = snap.incoming;
                if head != s.incoming_head {
                    s.incoming_open = None;
                    s.incoming_files.clear();
                }
                s.incoming = incoming;
                s.incoming_head = head;
                if s.repo.is_some() && s.status.upstream.is_some() {
                    this.scm_start_fetching(cx);
                }
                let s = &mut this.scm;
                if s.again {
                    s.again = false;
                    this.scm_refresh(cx);
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// Run git/gh steps in order off the UI thread; stop at the first failure.
    pub(crate) fn scm_run(&mut self, label: &str, steps: Vec<Step>, cx: &mut Context<Self>, done: impl FnOnce(&mut Self, &str, &mut Context<Self>) + 'static) {
        let Some(dir) = self.scm.repo.clone().or_else(|| self.root.clone()) else { return };
        if self.scm.busy.is_some() {
            self.status = tr("Source Control is busy, try again in a moment").into();
            cx.notify();
            return;
        }
        self.scm.busy = Some(label.to_string().into());
        self.scm.error = None;
        self.scm.note = None;
        self.scm.confirm_discard = None;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let res = cx
                .background_spawn(async move {
                    let mut last = String::new();
                    for s in &steps {
                        last = run_step(&dir, s)?;
                    }
                    Ok::<String, String>(last)
                })
                .await;
            _ = this.update(cx, |this, cx| {
                this.scm.busy = None;
                match res {
                    Ok(out) => done(this, &out, cx),
                    Err(e) => {
                        // gh: "a pull request for branch X already exists: URL" is good news.
                        if e.contains("already exists") {
                            if let Some(url) = find_url(&e) {
                                this.scm.note = Some(("A pull request for this branch already exists".into(), Some(url)));
                            } else {
                                this.scm.error = Some(e);
                            }
                        } else {
                            this.scm.error = Some(e);
                        }
                    }
                }
                this.scm_refresh(cx);
                cx.notify();
            });
        })
        .detach();
    }

    fn push_steps(&self) -> Vec<Step> {
        if self.scm.status.upstream.is_some() {
            vec![git_step(&["push"])]
        } else {
            vec![git_step(&["push", "-u", "origin", "HEAD"])]
        }
    }

    pub(crate) fn scm_commit(&mut self, after: After, amend: bool, window: &mut Window, cx: &mut Context<Self>) {
        let msg = self.scm.message.read(cx).value().trim().to_string();
        let st = self.scm.status.clone();
        if !st.conflicts.is_empty() {
            self.scm.error = Some("Resolve the merge conflicts first (files marked !).".into());
            cx.notify();
            return;
        }
        if msg.is_empty() && !amend {
            self.scm.error = Some("Type a commit message first.".into());
            let h = self.scm.message.focus_handle(cx);
            h.focus(window, cx);
            cx.notify();
            return;
        }
        if st.staged.is_empty() && st.changes.is_empty() && !amend {
            self.scm.error = Some("There are no changes to commit.".into());
            cx.notify();
            return;
        }
        // Save open editors first, like VS Code's promptToSaveFilesBeforeCommit.
        for ix in 0..self.docs.len() {
            if self.docs[ix].dirty {
                self.save_doc(ix, cx);
            }
        }
        let mut steps = Vec::new();
        if st.staged.is_empty() && !amend {
            steps.push(git_step(&["add", "-A"])); // smart commit
        }
        let mut args: Vec<String> = vec!["commit".into(), "--quiet".into()];
        if amend {
            args.push("--amend".into());
        }
        if msg.is_empty() {
            args.push("--no-edit".into());
            steps.push(Step::Git(args, None));
        } else {
            args.extend(["--file".into(), "-".into()]);
            steps.push(Step::Git(args, Some(msg)));
        }
        match after {
            After::Nothing => {}
            After::Push => steps.extend(self.push_steps()),
            After::Sync => {
                steps.push(git_step(&["pull", "--autostash"]));
                steps.extend(self.push_steps());
            }
        }
        let label = match after {
            After::Nothing => "Committing…",
            After::Push => "Committing and pushing…",
            After::Sync => "Committing and syncing…",
        };
        self.scm_run(label, steps, cx, move |this, _, _| {
            this.scm.clear_message = true;
            this.status = match after {
                After::Nothing => "Committed".into(),
                After::Push => "Committed and pushed".into(),
                After::Sync => "Committed and synced".into(),
            };
        });
    }

    pub(crate) fn scm_sync(&mut self, cx: &mut Context<Self>) {
        let mut steps = vec![git_step(&["pull", "--autostash"])];
        steps.extend(self.push_steps());
        self.scm_run("Syncing…", steps, cx, |this, _, _| this.status = tr("Synced with the remote").into());
    }

    fn scm_create_pr(&mut self, cx: &mut Context<Self>) {
        if !crate::github::gh_on_path() {
            self.scm.error = Some("Pull requests need the GitHub CLI (gh). Install it from cli.github.com, then sign in under Settings.".into());
            cx.notify();
            return;
        }
        let mut steps = self.push_steps();
        steps.push(Step::Gh(vec!["pr".into(), "create".into(), "--fill".into()]));
        self.scm_run("Creating pull request…", steps, cx, |this, out, _| {
            let url = find_url(out);
            this.scm.note = Some(("Pull request created".into(), url));
        });
    }

    fn scm_load_actions(&mut self, cx: &mut Context<Self>) {
        let Some(dir) = self.scm.repo.clone() else { return };
        if !crate::github::gh_on_path() {
            self.scm.error = Some("GitHub Actions needs the GitHub CLI (gh).".into());
            cx.notify();
            return;
        }
        cx.spawn(async move |this, cx| {
            let (wf, runs) = cx
                .background_spawn(async move {
                    let wf = run_step(&dir, &Step::Gh(["workflow", "list", "--json", "id,name,state"].map(String::from).to_vec()));
                    let runs = run_step(
                        &dir,
                        &Step::Gh(["run", "list", "-L", "6", "--json", "displayTitle,workflowName,status,conclusion,headBranch,url"].map(String::from).to_vec()),
                    );
                    (wf, runs)
                })
                .await;
            _ = this.update(cx, |this, cx| {
                this.scm.actions_loaded = true;
                let s = |v: &serde_json::Value, k: &str| v.get(k).map(|x| x.as_str().map(String::from).unwrap_or_else(|| x.to_string())).unwrap_or_default();
                match wf.and_then(|t| serde_json::from_str::<Vec<serde_json::Value>>(&t).map_err(|e| e.to_string())) {
                    Ok(list) => {
                        this.scm.workflows = list
                            .iter()
                            .filter(|w| s(w, "state") == "active")
                            .map(|w| Workflow { id: s(w, "id"), name: s(w, "name") })
                            .collect()
                    }
                    Err(e) => {
                        this.scm.error = Some(if e.contains("gh auth login") || e.contains("GH_TOKEN") {
                            "GitHub isn't signed in. Sign in under Settings → GitHub (or run  gh auth login  in the terminal).".into()
                        } else {
                            e
                        })
                    }
                }
                if let Ok(list) = runs.and_then(|t| serde_json::from_str::<Vec<serde_json::Value>>(&t).map_err(|e| e.to_string())) {
                    this.scm.runs = list
                        .iter()
                        .map(|r| Run {
                            title: s(r, "displayTitle"),
                            workflow: s(r, "workflowName"),
                            status: s(r, "status"),
                            conclusion: s(r, "conclusion"),
                            branch: s(r, "headBranch"),
                            url: s(r, "url"),
                        })
                        .collect();
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn scm_run_workflow(&mut self, wf: Workflow, cx: &mut Context<Self>) {
        let branch = self.scm.status.branch.clone().unwrap_or_else(|| "main".into());
        let steps = vec![Step::Gh(vec!["workflow".into(), "run".into(), wf.id.clone(), "--ref".into(), branch.clone()])];
        let name = wf.name.clone();
        self.scm_run(&format!("Starting {name}…"), steps, cx, move |this, _, cx| {
            this.scm.note = Some((format!("Started \"{name}\" on {branch}"), None));
            // The new run shows up in the list a moment later.
            cx.spawn(async move |this, cx| {
                cx.background_executor().timer(std::time::Duration::from_secs(4)).await;
                _ = this.update(cx, |this, cx| this.scm_load_actions(cx));
            })
            .detach();
        });
    }

    fn scm_stage(&mut self, path: Option<String>, stage: bool, cx: &mut Context<Self>) {
        let step = match (stage, path) {
            (true, Some(p)) => Step::Git(vec!["add".into(), "--".into(), p], None),
            (true, None) => git_step(&["add", "-A"]),
            (false, Some(p)) => Step::Git(vec!["restore".into(), "--staged".into(), "--".into(), p], None),
            (false, None) => git_step(&["restore", "--staged", "."]),
        };
        self.scm_run(if stage { "Staging…" } else { "Unstaging…" }, vec![step], cx, |_, _, _| {});
    }

    fn scm_discard(&mut self, ch: FileChange, cx: &mut Context<Self>) {
        if self.scm.confirm_discard.as_deref() != Some(ch.path.as_str()) {
            self.scm.confirm_discard = Some(ch.path.clone());
            cx.notify();
            return;
        }
        if ch.code == 'U' {
            // An untracked file goes to the Recycle Bin, not into the void.
            if let Some(repo) = &self.scm.repo {
                let full = repo.join(ch.path.replace('/', "\\"));
                if let Err(e) = crate::core::delete_path(&full) {
                    self.scm.error = Some(e);
                }
            }
            self.scm.confirm_discard = None;
            self.refresh_tree(cx);
            self.scm_refresh(cx);
            return;
        }
        let path = ch.path.clone();
        self.scm_run("Discarding…", vec![Step::Git(vec!["restore".into(), "--".into(), path], None)], cx, |this, _, cx| {
            this.agent.reloads.extend(this.docs.iter().map(|d| d.path.clone()));
            cx.notify();
        });
    }

    /// The file's diff in the Output panel, colored.
    fn scm_show_diff(&mut self, ch: FileChange, staged: bool, cx: &mut Context<Self>) {
        let Some(dir) = self.scm.repo.clone() else { return };
        let mut args = vec!["-c".to_string(), "core.quotepath=off".into(), "diff".into()];
        if staged {
            args.push("--cached".into());
        }
        args.extend(["--".into(), ch.path.clone()]);
        let untracked = ch.code == 'U';
        let path = ch.path.clone();
        cx.spawn(async move |this, cx| {
            let text = cx
                .background_spawn(async move {
                    if untracked {
                        std::fs::read_to_string(dir.join(&path)).map(|t| t.lines().map(|l| format!("+{l}")).collect::<Vec<_>>().join("\n")).unwrap_or_default()
                    } else {
                        run_step(&dir, &Step::Git(args, None)).unwrap_or_else(|e| e)
                    }
                })
                .await;
            _ = this.update(cx, |this, cx| {
                this.out_lines.clear();
                let mut head = Line::new(&ch.path, crate::term::LineKind::Cmd);
                head.meta = if staged { "Staged diff".into() } else { "Diff".into() };
                this.out_lines.push(head);
                for l in text.lines() {
                    let kind = if l.starts_with("+++") || l.starts_with("---") || l.starts_with("diff ") || l.starts_with("index ") {
                        crate::term::LineKind::Dim
                    } else if l.starts_with('+') {
                        crate::term::LineKind::Success
                    } else if l.starts_with('-') {
                        crate::term::LineKind::Err
                    } else if l.starts_with("@@") {
                        crate::term::LineKind::Hunk
                    } else {
                        crate::term::LineKind::Normal
                    };
                    this.out_lines.push(Line::new(l, kind));
                }
                if text.trim().is_empty() {
                    push_line(&mut this.out_lines, "No textual changes (binary file or mode change).", crate::term::LineKind::Dim);
                }
                this.panel_open = true;
                this.panel_tab = PanelTab::Output;
                this.out_scroll.scroll_to_item(0, ScrollStrategy::Top);
                cx.notify();
            });
        })
        .detach();
    }

    /// A commit's summary and changed files in the Output panel.
    fn scm_show_commit(&mut self, hash: String, cx: &mut Context<Self>) {
        let Some(dir) = self.scm.repo.clone() else { return };
        cx.spawn(async move |this, cx| {
            let text = cx
                .background_spawn(async move {
                    run_step(&dir, &Step::Git(["show", "--stat", "--format=%h  %s%n%an <%ae>, %ad%n%n%b", "--date=format:%Y-%m-%d %H:%M", &hash].map(String::from).to_vec(), None))
                        .unwrap_or_else(|e| e)
                })
                .await;
            _ = this.update(cx, |this, cx| {
                this.out_lines.clear();
                for (i, l) in text.lines().enumerate() {
                    let kind = if i == 0 {
                        crate::term::LineKind::Code
                    } else if l.contains(" | ") || l.contains("changed,") {
                        crate::term::LineKind::Normal
                    } else {
                        crate::term::LineKind::Dim
                    };
                    push_line(&mut this.out_lines, l, kind);
                }
                this.panel_open = true;
                this.panel_tab = PanelTab::Output;
                cx.notify();
            });
        })
        .detach();
    }

    // ------------------------------------------------------------ render

    pub(crate) fn render_scm(&self, cx: &mut Context<Self>) -> AnyElement {
        let s = &self.scm;
        if self.root.is_none() {
            return div().p_3().text_sm().text_color(c(theme::FG_DIM)).child(tr("Open a folder to use Source Control.")).into_any_element();
        }
        if s.repo.is_none() {
            return v_flex()
                .p_3()
                .gap_3()
                .text_sm()
                .child(div().text_color(c(theme::FG_MUTED)).child(tr("This folder is not a Git repository.")))
                .child(
                    Button::new("scm-init").label(tr("Initialize Repository")).primary().small().w_full().on_click(cx.listener(|this, _, _, cx| {
                        let dir = this.root.clone();
                        this.scm.repo = dir;
                        this.scm_run("Initializing…", vec![git_step(&["init"])], cx, |this, _, _| this.status = tr("Git repository created").into());
                    })),
                )
                .into_any_element();
        }
        let st = &s.status;
        let branch = st.branch.clone().unwrap_or_else(|| "detached HEAD".into());
        let nothing_staged = st.staged.is_empty();
        let commit_label = match (nothing_staged, st.count()) {
            (_, 0) => "Commit".to_string(),
            (true, _) => "Commit All".to_string(),
            (false, _) => "Commit".to_string(),
        };

        let commit_row = h_flex()
            .w_full()
            .child(
                Button::new("scm-commit")
                    .icon(Icon::new(IconName::Check))
                    .label(commit_label)
                    .primary()
                    .small()
                    .flex_1()
                    .tooltip(if nothing_staged { "Nothing is staged, so every change is committed (like VS Code's smart commit)" } else { "Commit the staged changes" })
                    .on_click(cx.listener(|this, _, w, cx| this.scm_commit(After::Nothing, false, w, cx))),
            )
            .child(
                Button::new("scm-commit-more").icon(Icon::new(IconName::ChevronDown)).primary().small().dropdown_menu(|m: PopupMenu, _, _| {
                    m.min_w(px(220.))
                        .menu(tr("Commit & Push"), Box::new(ScmCommitPush))
                        .menu(tr("Commit & Sync"), Box::new(ScmCommitSync))
                        .separator()
                        .menu(tr("Commit (Amend)"), Box::new(ScmCommitAmend))
                }),
            );

        let mut body = v_flex().id("scm-body").flex_1().min_h_0().overflow_y_scroll().pb_2();
        if s.confirm_uninit {
            // The whole repository path (the home folder as ~), so it is clear which one goes.
            let home = std::env::var("USERPROFILE").unwrap_or_default();
            let mut where_: String = s.repo.as_ref().map(|r| r.display().to_string()).unwrap_or_default();
            let full: SharedString = where_.clone().into();
            if !home.is_empty() && where_.starts_with(&home) {
                where_ = format!("~{}", &where_[home.len()..]);
            }
            let folder = s.repo.as_ref().and_then(|r| r.file_name()).map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            body = body.child(
                v_flex()
                    .mx_3()
                    .mb_2()
                    .p_2()
                    .gap_2()
                    .rounded(px(6.))
                    .border_1()
                    .border_color(c(theme::DANGER))
                    .bg(c(theme::BG_ELEVATED))
                    .text_xs()
                    .child(div().text_color(c(theme::FG_STRONG)).font_weight(FontWeight::SEMIBOLD).child(crate::trf!("Remove Git from {0}?"; 0 => folder)))
                    .child(
                        div()
                            .id("scm-uninit-path")
                            .truncate()
                            .text_color(c(theme::FG_DIM))
                            .tooltip(move |w, cx| gpui_kit::component::tooltip::Tooltip::new(full.clone()).build(w, cx))
                            .child(where_),
                    )
                    .child(div().text_color(c(theme::FG_MUTED)).child(tr("The .git folder (commits, branches, settings) goes to the Recycle Bin. Your files stay as they are.")))
                    .child(
                        h_flex()
                            .gap_2()
                            .child(Button::new("scm-uninit-yes").label(tr("Remove Git")).xsmall().danger().on_click(cx.listener(|this, _, _, cx| this.scm_remove_git(cx))))
                            .child(Button::new("scm-uninit-no").label(tr("Cancel")).xsmall().ghost().on_click(cx.listener(|this, _, _, cx| {
                                this.scm.confirm_uninit = false;
                                cx.notify();
                            }))),
                    ),
            );
        }
        if let Some(card) = self.render_scm_incoming(cx) {
            body = body.child(card);
        }
        body = body.child(
            v_flex()
                .px_3()
                .pb_2()
                .gap_2()
                .child(Input::new(&s.message).small())
                .child(commit_row)
                .when_some(s.busy.clone(), |v, b| v.child(div().text_xs().text_color(c(theme::BLUE)).child(b)))
                .when_some(s.error.clone(), |v, e| {
                    v.child(
                        div()
                            .p_2()
                            .rounded(px(4.))
                            .border_1()
                            .border_color(c(theme::DANGER))
                            .bg(c(theme::BG_ELEVATED))
                            .text_xs()
                            .text_color(c(theme::DANGER))
                            .child(e.lines().rev().take(4).collect::<Vec<_>>().into_iter().rev().collect::<Vec<_>>().join("\n")),
                    )
                })
                .when_some(s.note.clone(), |v, (text, url)| {
                    v.child(
                        h_flex()
                            .id("scm-note")
                            .gap_2()
                            .text_xs()
                            .text_color(c(theme::GREEN))
                            .child(text)
                            .when_some(url, |h, url| {
                                h.child(
                                    div()
                                        .id("scm-note-link")
                                        .text_color(c(theme::BLUE))
                                        .cursor_pointer()
                                        .hover(|s| s.underline())
                                        .child(tr("Open"))
                                        .on_click(move |_, _, cx| cx.open_url(&url)),
                                )
                            }),
                    )
                }),
        );

        let section = |id: &'static str, title: String, count: usize, open: bool| {
            h_flex()
                .id(id)
                .h(theme::u(22.))
                .px_2()
                .gap_1()
                .text_xs()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(c(theme::FG_MUTED))
                .child(Icon::new(if open { IconName::ChevronDown } else { IconName::ChevronRight }).size(theme::u(14.)))
                .child(div().flex_1().child(title))
                .when(count > 0, |h| h.child(div().px_1p5().rounded_full().bg(c(theme::BG_HOVER)).text_color(c(theme::FG)).child(count.to_string())))
        };

        let file_row = |i: usize, kind: u8, ch: &FileChange, this: &Self, cx: &mut Context<Self>| {
            let name = ch.path.rsplit('/').next().unwrap_or(&ch.path).to_string();
            let dir = ch.path.rsplit_once('/').map(|d| d.0.replace('/', "\\")).unwrap_or_default();
            let (icon, tint) = crate::files::icon_for(&name);
            let col = code_color(ch.code);
            let deleted = ch.code == 'D';
            let full = this.scm.repo.as_ref().map(|r| r.join(ch.path.replace('/', "\\")));
            let staged = kind == 1;
            let confirm = this.scm.confirm_discard.as_deref() == Some(ch.path.as_str());
            let (c1, c2, c3) = (ch.clone(), ch.clone(), ch.clone());
            h_flex()
                .id(("scm-f", i * 4 + kind as usize))
                .group("scm-row")
                .h(theme::u(22.))
                .pl(theme::u(18.))
                .pr_2()
                .gap_1p5()
                .text_size(theme::u(11.5))
                .cursor_pointer()
                .hover(|s| s.bg(c(theme::BG_HOVER)))
                .child(Icon::new(icon).size(theme::u(14.)).text_color(theme::tint(tint)))
                .child(div().text_color(c(col)).when(deleted, |d| d.line_through()).child(name))
                .child(div().flex_1().truncate().text_xs().text_color(c(theme::FG_DIM)).child(dir))
                .child(
                    h_flex()
                        .gap_px()
                        .invisible()
                        .group_hover("scm-row", |s| s.visible())
                        .when(!staged && kind != 2, |h| {
                            h.child(
                                Button::new(("scm-diff", i))
                                    .icon(Icon::new(Ico::FileDiff))
                                    .ghost()
                                    .xsmall()
                                    .tooltip(tr("Show changes"))
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        cx.stop_propagation();
                                        this.scm_show_diff(c1.clone(), false, cx);
                                    })),
                            )
                            .child(
                                Button::new(("scm-discard", i))
                                    .icon(Icon::new(Ico::Undo2))
                                    .ghost()
                                    .xsmall()
                                    .when(confirm, |b| b.label(tr("Discard?")))
                                    .tooltip(tr("Discard changes (click twice)"))
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        cx.stop_propagation();
                                        this.scm_discard(c2.clone(), cx);
                                    })),
                            )
                        })
                        .when(staged, |h| {
                            h.child(Button::new(("scm-sdiff", i)).icon(Icon::new(Ico::FileDiff)).ghost().xsmall().tooltip(tr("Show staged changes")).on_click(cx.listener(
                                move |this, _, _, cx| {
                                    cx.stop_propagation();
                                    this.scm_show_diff(c3.clone(), true, cx);
                                },
                            )))
                        })
                        .child({
                            let p = ch.path.clone();
                            Button::new(("scm-stage", i * 4 + kind as usize))
                                .icon(Icon::new(if staged { IconName::Minus } else { IconName::Plus }))
                                .ghost()
                                .xsmall()
                                .tooltip(if staged { "Unstage" } else { "Stage" })
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    cx.stop_propagation();
                                    this.scm_stage(Some(p.clone()), !staged, cx);
                                }))
                        }),
                )
                .child(div().w(theme::u(12.)).text_xs().font_weight(FontWeight::BOLD).text_color(c(col)).child(ch.code.to_string()))
                .on_click(cx.listener(move |this, _, w, cx| {
                    if let Some(f) = full.clone().filter(|f| f.is_file()) {
                        this.open_file(f, w, cx);
                    }
                }))
        };

        if !st.conflicts.is_empty() {
            body = body.child(section("scm-sec-conf", "Merge Changes".into(), st.conflicts.len(), true));
            for (i, ch) in st.conflicts.iter().enumerate().take(MAX_ROWS) {
                body = body.child(file_row(i, 2, ch, self, cx));
            }
        }
        if !st.staged.is_empty() {
            body = body.child(section("scm-sec-staged", "Staged Changes".into(), st.staged.len(), true).child(
                Button::new("scm-unstage-all").icon(Icon::new(IconName::Minus)).ghost().xsmall().tooltip(tr("Unstage All")).on_click(cx.listener(|this, _, _, cx| {
                    this.scm_stage(None, false, cx);
                })),
            ));
            for (i, ch) in st.staged.iter().enumerate().take(MAX_ROWS) {
                body = body.child(file_row(i, 1, ch, self, cx));
            }
            body = body.children(more_rows(st.staged.len()));
        }
        body = body.child(section("scm-sec-changes", "Changes".into(), st.changes.len(), true).when(!st.changes.is_empty(), |h| {
            h.child(Button::new("scm-stage-all").icon(Icon::new(IconName::Plus)).ghost().xsmall().tooltip(tr("Stage All")).on_click(cx.listener(|this, _, _, cx| {
                this.scm_stage(None, true, cx);
            })))
        }));
        // Lots of untracked files (a server folder, build output): offer to ignore the
        // folders they are in, biggest first, before listing them.
        let untracked = st.changes.iter().filter(|c| c.code == 'U').count();
        if untracked > 200 {
            let mut by: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
            for ch in st.changes.iter().filter(|c| c.code == 'U') {
                if let Some((top, _)) = ch.path.split_once('/') {
                    *by.entry(top).or_default() += 1;
                }
            }
            let mut top: Vec<(&str, usize)> = by.into_iter().collect();
            top.sort_by(|a, b| b.1.cmp(&a.1));
            body = body.child(
                v_flex()
                    .mx_3()
                    .my_1()
                    .p_2()
                    .gap_1()
                    .rounded(px(6.))
                    .border_1()
                    .border_color(c(theme::BORDER))
                    .bg(c(theme::BG_ELEVATED))
                    .text_xs()
                    .child(div().text_color(c(theme::FG_STRONG)).child(crate::trf!("{n} untracked files"; n => untracked)))
                    .child(div().text_color(c(theme::FG_DIM)).child(tr("Folders with the most of them. Ignore the ones git should not track (they go to .gitignore).")))
                    .children(top.into_iter().take(6).enumerate().map(|(i, (dir, n))| {
                        let pattern = format!("{dir}/");
                        h_flex()
                            .gap_2()
                            .h(theme::u(24.))
                            .child(Icon::new(IconName::Folder).size(theme::u(13.)).text_color(c(theme::FOLDER)))
                            .child(div().flex_1().min_w_0().truncate().text_color(c(theme::FG)).child(pattern.clone()))
                            .child(div().flex_none().text_color(c(theme::FG_DIM)).child(n.to_string()))
                            .child(Button::new(("scm-ignore", i)).label(tr("Ignore")).xsmall().on_click(cx.listener(move |this, _, _, cx| this.scm_ignore(pattern.clone(), cx))))
                    })),
            );
        }
        for (i, ch) in st.changes.iter().enumerate().take(MAX_ROWS) {
            body = body.child(file_row(i, 0, ch, self, cx));
        }
        body = body.children(more_rows(st.changes.len()));
        if st.count() == 0 {
            body = body.child(div().px(theme::u(18.)).py_1().text_xs().text_color(c(theme::FG_DIM)).child(tr("No changes. The working tree matches the last commit.")));
        }

        // Sync bar: VS Code puts this in the status bar; it also reads well here.
        let sync_label = match (&st.upstream, st.ahead, st.behind) {
            (None, ..) => "Publish Branch".to_string(),
            (Some(_), 0, 0) => "Up to date".to_string(),
            (Some(_), a, b) => format!("Sync  {b}↓ {a}↑"),
        };
        body = body.child(
            h_flex()
                .px_3()
                .pt_2()
                .gap_1()
                .child(
                    Button::new("scm-sync")
                        .icon(Icon::new(if st.upstream.is_none() { Ico::CloudUpload } else { Ico::RefreshCcw }))
                        .label(sync_label)
                        .tooltip(if st.upstream.is_none() { format!("Push {branch} to origin and track it") } else { format!("Pull, then push {branch}") })
                        .small()
                        .flex_1()
                        .on_click(cx.listener(|this, _, _, cx| {
                            if this.scm.status.upstream.is_none() {
                                let steps = this.push_steps();
                                this.scm_run("Publishing branch…", steps, cx, |this, _, _| this.status = tr("Branch published").into());
                            } else {
                                this.scm_sync(cx);
                            }
                        })),
                )
                .child(
                    Button::new("scm-pr")
                        .icon(Icon::new(Ico::GitPullRequest))
                        .small()
                        .tooltip(tr("Create a pull request on GitHub (gh pr create --fill)"))
                        .on_click(cx.listener(|this, _, _, cx| this.scm_create_pr(cx))),
                ),
        );

        // Graph
        body = body.child(
            section("scm-sec-graph", "Graph".into(), 0, s.show_graph).mt_2().cursor_pointer().on_click(cx.listener(|this, _, _, cx| {
                this.scm.show_graph = !this.scm.show_graph;
                if this.scm.show_graph {
                    this.scm_refresh(cx);
                }
                cx.notify();
            })),
        );
        if s.show_graph {
            let n = s.log.len();
            for (i, cm) in s.log.iter().enumerate() {
                let hash = cm.hash.clone();
                let lane = div()
                    .relative()
                    .w(theme::u(14.))
                    .h_full()
                    .flex_none()
                    .child(
                        div()
                            .absolute()
                            .left(theme::u(6.))
                            .w(px(2.))
                            .top(if i == 0 { theme::u(11.) } else { px(0.) })
                            .bottom(if i + 1 == n { theme::u(11.) } else { px(0.) })
                            .bg(c(theme::ACCENT).opacity(0.55)),
                    )
                    .child(
                        div()
                            .absolute()
                            .left(theme::u(3.))
                            .top(theme::u(7.))
                            .size(theme::u(8.))
                            .rounded_full()
                            .border_2()
                            .border_color(c(theme::ACCENT))
                            .bg(if cm.merge { c(theme::BG_SIDE) } else { c(theme::ACCENT) }),
                    );
                body = body.child(
                    h_flex()
                        .id(("scm-c", i))
                        .h(theme::u(22.))
                        .pl(theme::u(12.))
                        .pr_2()
                        .gap_1p5()
                        .text_size(theme::u(11.5))
                        .cursor_pointer()
                        .hover(|s| s.bg(c(theme::BG_HOVER)))
                        .child(lane)
                        .child(div().flex_1().min_w(theme::u(80.)).truncate().text_color(c(theme::FG)).child(cm.subject.clone()))
                        // One pill (the first ref) so the subject keeps its room; the rest are in the tooltip.
                        .children(cm.refs.first().map(|r| {
                            let remote = r.starts_with("origin/");
                            div()
                                .flex_none()
                                .max_w(theme::u(120.))
                                .truncate()
                                .px_1p5()
                                .rounded(px(8.))
                                .text_xs()
                                .bg(c(if remote { theme::BLUE } else { theme::ACCENT }).opacity(0.22))
                                .text_color(c(if remote { theme::BLUE } else { theme::ACCENT_TEXT }))
                                .child(r.clone())
                        }))
                        .when(cm.refs.len() > 1, |h| h.child(div().flex_none().text_xs().text_color(c(theme::FG_DIM)).child(format!("+{}", cm.refs.len() - 1))))
                        .tooltip({
                            let refs = if cm.refs.is_empty() { String::new() } else { format!("\n{}", cm.refs.join(", ")) };
                            let tip = format!("{}  {}\n{} · {}{refs}", cm.short, cm.subject, cm.author, cm.when);
                            move |w, cx| Tooltip::new(tip.clone()).build(w, cx)
                        })
                        .on_click(cx.listener(move |this, _, _, cx| this.scm_show_commit(hash.clone(), cx))),
                );
            }
        }

        // GitHub Actions: run a workflow (deploy/build) and see recent runs.
        body = body.child(
            section("scm-sec-actions", "GitHub Actions (deploy)".into(), 0, s.show_actions).mt_2().cursor_pointer().on_click(cx.listener(|this, _, _, cx| {
                this.scm.show_actions = !this.scm.show_actions;
                if this.scm.show_actions {
                    this.scm_load_actions(cx);
                }
                cx.notify();
            })),
        );
        if s.show_actions {
            if s.workflows.is_empty() && s.runs.is_empty() {
                body = body.child(div().px(theme::u(18.)).py_1().text_xs().text_color(c(theme::FG_DIM)).child(if s.actions_loaded { "No active workflows in this repository." } else { "Loading workflows…" }));
            }
            for (i, wf) in s.workflows.iter().enumerate() {
                let w = wf.clone();
                body = body.child(
                    h_flex()
                        .h(theme::u(22.))
                        .pl(theme::u(18.))
                        .pr_2()
                        .gap_1p5()
                        .text_size(theme::u(11.5))
                        .child(Icon::new(Ico::Rocket).size(theme::u(14.)).text_color(c(theme::FG_MUTED)))
                        .child(div().flex_1().truncate().child(wf.name.clone()))
                        .child(
                            Button::new(("scm-wf", i))
                                .icon(Icon::new(IconName::Play))
                                .label(tr("Run"))
                                .ghost()
                                .xsmall()
                                .tooltip(tr("gh workflow run on the current branch (needs workflow_dispatch)"))
                                .on_click(cx.listener(move |this, _, _, cx| this.scm_run_workflow(w.clone(), cx))),
                        ),
                );
            }
            for (i, r) in s.runs.iter().enumerate() {
                let (mark, col) = match (r.status.as_str(), r.conclusion.as_str()) {
                    ("completed", "success") => ("✓", theme::GREEN),
                    ("completed", "failure") => ("✕", theme::DANGER),
                    ("completed", _) => ("–", theme::FG_DIM),
                    _ => ("●", theme::BLUE),
                };
                let url = r.url.clone();
                body = body.child(
                    h_flex()
                        .id(("scm-run", i))
                        .h(theme::u(22.))
                        .pl(theme::u(18.))
                        .pr_2()
                        .gap_1p5()
                        .text_size(theme::u(11.5))
                        .cursor_pointer()
                        .hover(|s| s.bg(c(theme::BG_HOVER)))
                        .child(div().w(theme::u(12.)).text_color(c(col)).child(mark))
                        .child(div().flex_1().min_w(theme::u(70.)).truncate().child(r.title.clone()))
                        .child(div().flex_none().max_w(theme::u(150.)).truncate().text_xs().text_color(c(theme::FG_DIM)).child(format!("{} · {}", r.workflow, r.branch)))
                        .on_click(move |_, _, cx| cx.open_url(&url)),
                );
            }
        }
        v_flex().size_full().child(body).into_any_element()
    }

    /// Header buttons of the Source Control view (refresh, commit, more).
    pub(crate) fn render_scm_header(&self, cx: &mut Context<Self>) -> impl IntoElement {
        h_flex()
            .gap_px()
            .child(tool_button("scm-h-commit", IconName::Check, "Commit", cx, |this, w, cx| this.scm_commit(After::Nothing, false, w, cx)))
            .child(tool_button("scm-h-refresh", IconName::RefreshCw, "Refresh", cx, |this, _, cx| this.scm_refresh(cx)))
            .child(Button::new("scm-h-more").icon(Icon::new(IconName::Ellipsis)).ghost().xsmall().dropdown_menu(|m: PopupMenu, _, _| {
                m.min_w(px(230.))
                    .menu(tr("Pull"), Box::new(ScmPull))
                    .menu(tr("Push"), Box::new(ScmPush))
                    .menu(tr("Sync (Pull, then Push)"), Box::new(ScmSync))
                    .menu(tr("Fetch"), Box::new(ScmFetch))
                    .separator()
                    .menu(tr("Commit & Push"), Box::new(ScmCommitPush))
                    .menu(tr("Commit (Amend)"), Box::new(ScmCommitAmend))
                    .separator()
                    .menu(tr("Stage All Changes"), Box::new(ScmStageAll))
                    .menu(tr("Unstage All Changes"), Box::new(ScmUnstageAll))
                    .separator()
                    .menu(tr("Create Pull Request"), Box::new(ScmCreatePr))
                    .menu(tr("Open Repository on GitHub"), Box::new(ScmOpenGithub))
                    .separator()
                    .menu(tr("Remove Git from This Folder…"), Box::new(ScmRemoveGit))
            }))
    }

    // ------------------------------------------------------------ actions

    pub(crate) fn on_scm_show(&mut self, _: &ScmShow, _: &mut Window, cx: &mut Context<Self>) {
        self.view = SideView::Scm;
        self.sidebar_open = true;
        self.settings_open = false;
        self.scm_refresh(cx);
        cx.notify();
    }
    pub(crate) fn on_scm_commit_push(&mut self, _: &ScmCommitPush, w: &mut Window, cx: &mut Context<Self>) {
        self.scm_commit(After::Push, false, w, cx);
    }
    pub(crate) fn on_scm_commit_sync(&mut self, _: &ScmCommitSync, w: &mut Window, cx: &mut Context<Self>) {
        self.scm_commit(After::Sync, false, w, cx);
    }
    pub(crate) fn on_scm_commit_amend(&mut self, _: &ScmCommitAmend, w: &mut Window, cx: &mut Context<Self>) {
        self.scm_commit(After::Nothing, true, w, cx);
    }
    pub(crate) fn on_scm_pull(&mut self, _: &ScmPull, _: &mut Window, cx: &mut Context<Self>) {
        self.scm_run("Pulling…", vec![git_step(&["pull", "--autostash"])], cx, |this, _, cx| {
            this.agent.reloads.extend(this.docs.iter().filter(|d| !d.dirty).map(|d| d.path.clone()));
            this.refresh_tree(cx);
            this.status = tr("Pulled").into();
        });
    }
    pub(crate) fn on_scm_push(&mut self, _: &ScmPush, _: &mut Window, cx: &mut Context<Self>) {
        let steps = self.push_steps();
        self.scm_run("Pushing…", steps, cx, |this, _, _| this.status = tr("Pushed").into());
    }
    pub(crate) fn on_scm_sync(&mut self, _: &ScmSync, _: &mut Window, cx: &mut Context<Self>) {
        self.scm_sync(cx);
    }
    pub(crate) fn on_scm_fetch(&mut self, _: &ScmFetch, _: &mut Window, cx: &mut Context<Self>) {
        self.scm_run("Fetching…", vec![git_step(&["fetch", "--prune"])], cx, |this, _, _| this.status = tr("Fetched").into());
    }
    pub(crate) fn on_scm_remove_git(&mut self, _: &ScmRemoveGit, _: &mut Window, cx: &mut Context<Self>) {
        if self.scm.repo.is_some() {
            self.scm.confirm_uninit = true;
            self.view = SideView::Scm;
            self.sidebar_open = true;
            cx.notify();
        }
    }

    /// Undo "Initialize Repository": the `.git` folder goes to the Recycle Bin
    /// (history and settings with it); the project's files are not touched.
    fn scm_remove_git(&mut self, cx: &mut Context<Self>) {
        self.scm.confirm_uninit = false;
        let Some(repo) = self.scm.repo.clone() else { return };
        let git = repo.join(".git");
        self.scm.busy = Some(tr("Removing Git…").into());
        cx.notify();
        cx.spawn(async move |this, cx| {
            let r = cx.background_spawn(async move { crate::core::delete_path(&git) }).await;
            _ = this.update(cx, |this, cx| {
                this.scm.busy = None;
                match r {
                    Ok(()) => {
                        this.scm.repo = None;
                        this.scm.status = RepoStatus::default();
                        this.scm.marks = Rc::default();
                        this.scm.dirty_dirs = Rc::default();
                        this.status = crate::trf!("Git removed from {0}. The .git folder is in the Recycle Bin."; 0 => repo.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()).into();
                    }
                    Err(e) => this.scm.error = Some(e),
                }
                this.scm_refresh(cx);
                cx.notify();
            });
        })
        .detach();
    }

    /// Add `pattern` (a folder like `libraries/`) to the repository's .gitignore.
    fn scm_ignore(&mut self, pattern: String, cx: &mut Context<Self>) {
        let Some(repo) = self.scm.repo.clone() else { return };
        let path = repo.join(".gitignore");
        let mut text = std::fs::read_to_string(&path).unwrap_or_default();
        if text.lines().any(|l| l.trim() == pattern) {
            return;
        }
        if !text.is_empty() && !text.ends_with('\n') {
            text.push('\n');
        }
        text.push_str(&pattern);
        text.push('\n');
        match std::fs::write(&path, text) {
            Ok(()) => self.status = crate::trf!("{0} added to .gitignore"; 0 => pattern).into(),
            Err(e) => self.scm.error = Some(e.to_string()),
        }
        self.scm_refresh(cx);
        cx.notify();
    }

    pub(crate) fn on_scm_stage_all(&mut self, _: &ScmStageAll, _: &mut Window, cx: &mut Context<Self>) {
        self.scm_stage(None, true, cx);
    }
    pub(crate) fn on_scm_unstage_all(&mut self, _: &ScmUnstageAll, _: &mut Window, cx: &mut Context<Self>) {
        self.scm_stage(None, false, cx);
    }
    pub(crate) fn on_scm_create_pr(&mut self, _: &ScmCreatePr, _: &mut Window, cx: &mut Context<Self>) {
        self.scm_create_pr(cx);
    }
    pub(crate) fn on_scm_open_github(&mut self, _: &ScmOpenGithub, _: &mut Window, cx: &mut Context<Self>) {
        let branch = self.scm.status.branch.clone();
        self.scm_run("Opening GitHub…", vec![git_step(&["remote", "get-url", "origin"])], cx, move |_, out, cx| {
            let url = out.trim().trim_end_matches(".git").replace("git@github.com:", "https://github.com/");
            let url = match &branch {
                Some(b) if url.starts_with("https://github.com/") => format!("{url}/tree/{b}"),
                _ => url,
            };
            if url.starts_with("http") {
                cx.open_url(&url);
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::{find_url, parse_log, parse_status};

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
        assert_eq!((st.ahead, st.behind), (2, 21));
        assert_eq!(st.staged.iter().map(|c| (c.path.as_str(), c.code)).collect::<Vec<_>>(), vec![("new.txt", 'A'), ("both.rs", 'M'), ("renamed.py", 'R')]);
        assert_eq!(st.staged[2].orig.as_deref(), Some("old.py"));
        assert_eq!(st.changes.iter().map(|c| (c.path.as_str(), c.code)).collect::<Vec<_>>(), vec![("src/app.py", 'M'), ("both.rs", 'M'), ("notes.md", 'U')]);
        assert_eq!(st.conflicts[0].path, "conflict.txt");
        assert_eq!(st.count(), 7);
    }

    #[test]
    fn spaces_in_paths() {
        let st = parse_status("1 .M N... 100644 100644 100644 a b my folder/a file.py\0");
        assert_eq!(st.changes[0].path, "my folder/a file.py");
    }

    #[test]
    fn log_records() {
        let out = "h1\x1fs1\x1fFix bug\x1fAna\x1f2 hours ago\x1fHEAD -> main, origin/main, origin/HEAD\x1fp1\0\nh2\x1fs2\x1fMerge\x1fBudi\x1f1 day ago\x1f\x1fp1 p2\0";
        let log = parse_log(out);
        assert_eq!(log.len(), 2);
        assert_eq!(log[0].refs, vec!["main", "origin/main"]);
        assert!(log[1].merge && !log[0].merge);
    }

    #[test]
    fn urls() {
        assert_eq!(find_url("Creating pull request\nhttps://github.com/o/r/pull/7\n").as_deref(), Some("https://github.com/o/r/pull/7"));
    }
}
