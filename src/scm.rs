
use crate::i18n::tr;
use crate::theme::{self, c};
use crate::workbench::*;
use gpui_kit::assets::IconName as Ico;
use gpui_kit::component::{
    button::{Button, ButtonVariants},
    h_flex,
    input::{InputEvent, InputState},
    menu::{DropdownMenu, PopupMenu},
    scroll::ScrollableElement,
    v_flex, Disableable, ElementExt, Icon, IconName, Sizable,
};
use gpui_kit::{prelude::FluentBuilder, *};
use std::cell::Cell;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::rc::Rc;
use gpui_kit::component::tooltip::Tooltip;

use crate::scm_git::{git_step, run_step, snapshot, FileChange, Group, RemoteAction, Step};
pub use crate::scm_git::{parse_log, Commit, RepoStatus};

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

#[derive(Clone, Copy, PartialEq)]
pub enum After {
    Nothing,
    Push,
    Sync,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ScmPane {
    Conflicts = 0,
    Staged = 1,
    Changes = 2,
    Graph = 3,
    Actions = 4,
    Untracked = 5,
}

#[derive(Clone, Copy, Debug)]
struct PaneSpec {
    id: ScmPane,
    flexible: bool,
    min_h: f32,
    natural_h: f32,
    saved: Option<f32>,
}

#[derive(Clone, Debug)]
pub struct ScmPaneDrag {
    pub(crate) sash: usize,
    pub(crate) y0: f32,
    pub(crate) ids: Vec<ScmPane>,
    pub(crate) start: Vec<f32>,
    pub(crate) mins: Vec<f32>,
    pub(crate) maxs: Vec<f32>,
}

const SASH_H: f32 = 5.;

fn water_fill_panes(budget: f32, specs: &[PaneSpec], flex_ix: &[usize]) -> Vec<f32> {
    let mut out = vec![0.; specs.len()];
    let mut active: Vec<usize> = flex_ix.to_vec();
    let mut rem = budget;
    loop {
        if active.is_empty() {
            break;
        }
        let share = rem / active.len() as f32;
        let mut settled = false;
        let mut next = Vec::with_capacity(active.len());
        for i in active {
            if specs[i].natural_h <= share {
                let h = specs[i].natural_h.max(specs[i].min_h).min(rem);
                out[i] = h;
                rem = (rem - h).max(0.);
                settled = true;
            } else {
                next.push(i);
            }
        }
        active = next;
        if !settled {
            break;
        }
    }
    if !active.is_empty() {
        let share = rem / active.len() as f32;
        for i in active {
            out[i] = share.max(specs[i].min_h);
        }
    } else if rem > 0.5 {
        if let Some(&last) = flex_ix.last() {
            out[last] += rem;
        }
    }
    out
}

fn resolve_pane_heights(avail_h: f32, specs: &[PaneSpec]) -> Vec<f32> {
    let mut out: Vec<f32> = specs.iter().map(|s| s.min_h).collect();
    let fixed_sum: f32 = specs.iter().filter(|s| !s.flexible).map(|s| s.min_h).sum();
    let budget = (avail_h - fixed_sum).max(0.);
    let flex_ix: Vec<usize> = specs
        .iter()
        .enumerate()
        .filter(|(_, s)| s.flexible)
        .map(|(i, _)| i)
        .collect();
    if flex_ix.is_empty() {
        return out;
    }
    if flex_ix.len() == 1 {
        let i = flex_ix[0];
        out[i] = budget.max(specs[i].min_h);
        return out;
    }
    let min_sum: f32 = flex_ix.iter().map(|&i| specs[i].min_h).sum();
    if budget <= min_sum {
        let denom = min_sum.max(1.);
        for &i in &flex_ix {
            out[i] = (budget * (specs[i].min_h / denom)).max(1.);
        }
        return out;
    }
    let def = water_fill_panes(budget, specs, &flex_ix);
    if flex_ix.iter().all(|&i| specs[i].saved.is_none()) {
        for &i in &flex_ix {
            out[i] = def[i];
        }
        return out;
    }
    for &i in &flex_ix {
        out[i] = specs[i].saved.unwrap_or(def[i]).max(specs[i].min_h);
    }
    let cur_sum: f32 = flex_ix.iter().map(|&i| out[i]).sum();
    if cur_sum < budget {
        let extra = budget - cur_sum;
        let denom = cur_sum.max(1.);
        for &i in &flex_ix {
            out[i] += extra * (out[i] / denom);
        }
    } else if cur_sum > budget {
        let excess = cur_sum - budget;
        let slack_sum: f32 = flex_ix
            .iter()
            .map(|&i| (out[i] - specs[i].min_h).max(0.))
            .sum();
        if slack_sum > 0. {
            for &i in &flex_ix {
                let slack = (out[i] - specs[i].min_h).max(0.);
                out[i] -= excess * (slack / slack_sum);
            }
        }
    }
    out
}

fn apply_sash_drag(drag: &ScmPaneDrag, y: f32) -> Vec<(ScmPane, f32)> {
    let n = drag.start.len();
    if drag.sash == 0 || drag.sash >= n {
        return Vec::new();
    }
    let delta = y - drag.y0;
    let mut h = drag.start.clone();
    if delta > 0. {
        let grow_above: f32 = (0..drag.sash).map(|i| (drag.maxs[i] - h[i]).max(0.)).sum();
        let shrink_below: f32 = (drag.sash..n).map(|i| (h[i] - drag.mins[i]).max(0.)).sum();
        let shift = delta.min(grow_above).min(shrink_below);
        let mut rem = shift;
        for i in (0..drag.sash).rev() {
            let add = rem.min((drag.maxs[i] - h[i]).max(0.));
            h[i] += add;
            rem -= add;
        }
        let mut rem = shift;
        for (cur, &min_h) in h[drag.sash..].iter_mut().zip(&drag.mins[drag.sash..]) {
            let sub = rem.min((*cur - min_h).max(0.));
            *cur -= sub;
            rem -= sub;
        }
    } else if delta < 0. {
        let shrink_above: f32 = (0..drag.sash).map(|i| (h[i] - drag.mins[i]).max(0.)).sum();
        let grow_below: f32 = (drag.sash..n).map(|i| (drag.maxs[i] - h[i]).max(0.)).sum();
        let shift = (-delta).min(shrink_above).min(grow_below);
        let mut rem = shift;
        for i in (0..drag.sash).rev() {
            let sub = rem.min((h[i] - drag.mins[i]).max(0.));
            h[i] -= sub;
            rem -= sub;
        }
        let mut rem = shift;
        for (cur, &max_h) in h[drag.sash..].iter_mut().zip(&drag.maxs[drag.sash..]) {
            let add = rem.min((max_h - *cur).max(0.));
            *cur += add;
            rem -= add;
        }
    }
    drag.ids
        .iter()
        .copied()
        .zip(h)
        .enumerate()
        .filter(|(i, _)| drag.maxs[*i] > drag.mins[*i])
        .map(|(_, pair)| pair)
        .collect()
}

pub struct Scm {
    pub repo: Option<PathBuf>,
    pub status: RepoStatus,
    pub log: Vec<Commit>,
    pub graph: crate::scm_graph::GraphLayout,
    pub commit_open: Option<String>,
    pub commit_files: HashMap<String, Vec<(char, String)>>,
    pub runs: Vec<Run>,
    pub workflows: Vec<Workflow>,
    pub busy: Option<SharedString>,
    pub error: Option<String>,
    repo_error: Option<String>,
    pub note: Option<(String, Option<String>)>,
    pub message: Entity<InputState>,
    pub msg_non_empty: bool,
    pub workspace_generation: u64,
    refreshing: bool,
    again: bool,
    show_changes_view: bool,
    show_tree: bool,
    collapsed_folders: [HashSet<String>; 4],
    pub show_conflicts: bool,
    pub show_staged: bool,
    pub show_changes: bool,
    pub show_untracked: bool,
    row_limits: [usize; 4],
    pub show_graph: bool,
    pub show_actions: bool,
    pub actions_loaded: bool,
    confirm_discard: Option<(Group, Option<String>, Vec<FileChange>)>,
    pub confirm_uninit: bool,
    pub marks: Rc<HashMap<String, char>>,
    pub dirty_dirs: Rc<HashSet<String>>,
    pub clear_message: bool,
    pub last_refresh: Option<std::time::Instant>,
    pub incoming: Vec<Commit>,
    pub incoming_head: Option<String>,
    pub incoming_dismissed: Option<String>,
    pub incoming_shown: bool,
    pub incoming_open: Option<String>,
    pub incoming_files: HashMap<String, Vec<(char, String)>>,
    pub fetching: bool,
    pub fetch_loop: bool,
    pub last_fetch: Option<std::time::Instant>,
    pub pane_h: [Option<f32>; 6],
    pub painted_h: Rc<Cell<[f32; 6]>>,
    pub stack_h: Rc<Cell<f32>>,
    pub pane_drag: Option<ScmPaneDrag>,
}

impl Scm {
    pub(crate) fn is_busy(&self) -> bool {
        self.busy.is_some() || self.fetching
    }

    pub fn new(window: &mut Window, cx: &mut Context<Workbench>) -> (Self, Subscription) {
        let message = cx.new(|cx| InputState::new(window, cx).placeholder(tr("Commit message")));
        let sub = cx.subscribe_in(
            &message,
            window,
            |this: &mut Workbench, _, ev: &InputEvent, window, cx| {
                if let InputEvent::PressEnter { .. } = ev {
                    this.scm_commit(After::Nothing, false, window, cx);
                } else {
                    let non_empty = !this.scm.message.read(cx).value().trim().is_empty();
                    let cleared_err = non_empty
                        && this.scm.error.as_deref() == Some(tr("Enter a commit message."))
                        && {
                            this.scm.error = None;
                            true
                        };
                    if non_empty != this.scm.msg_non_empty || cleared_err {
                        this.scm.msg_non_empty = non_empty;
                        cx.notify();
                    }
                }
            },
        );
        (
            Self {
                repo: None,
                status: RepoStatus::default(),
                log: Vec::new(),
                graph: Default::default(),
                commit_open: None,
                commit_files: HashMap::new(),
                runs: Vec::new(),
                workflows: Vec::new(),
                busy: None,
                error: None,
                repo_error: None,
                note: None,
                message,
                msg_non_empty: false,
                workspace_generation: 0,
                refreshing: false,
                again: false,
                show_changes_view: true,
                show_tree: false,
                collapsed_folders: Default::default(),
                show_conflicts: true,
                show_staged: true,
                show_changes: true,
                show_untracked: true,
                row_limits: [MAX_ROWS; 4],
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
                pane_h: [None; 6],
                painted_h: Rc::new(Cell::new([0.; 6])),
                stack_h: Rc::new(Cell::new(0.)),
                pane_drag: None,
            },
            sub,
        )
    }

}

pub fn code_color(code: char) -> theme::Tok {
    match code {
        'A' | 'U' => theme::GREEN,
        'D' | '!' => theme::DANGER,
        'R' | 'C' => theme::BLUE,
        _ => theme::WARN,
    }
}


fn find_url(s: &str) -> Option<String> {
    let i = s.find("https://")?;
    Some(s[i..].split_whitespace().next()?.trim_end_matches(['.', ',', ')']).to_string())
}


fn group_title(group: Group) -> &'static str {
    match group {
        Group::Conflicts => tr("Merge Conflicts"),
        Group::Staged => tr("Staged Changes"),
        Group::Changes => tr("Changes"),
        Group::Untracked => tr("Untracked Files"),
    }
}

const MAX_ROWS: usize = 200;
pub(crate) const ROW_H: f32 = 24.;
// Clear gpui-base's 16px overlay scrollbar track so badges and controls never sit underneath it.
pub(crate) const SCROLLBAR_PAD: f32 = 18.;

// `.overflow_hidden()` drops Taffy's min-height to 0, so `.flex_none()` keeps rows from collapsing under scroll overflow.
pub(crate) fn scm_row() -> Div {
    h_flex()
        .flex_none()
        .w_full()
        .min_w_0()
        .overflow_hidden()
        .h(theme::u(ROW_H))
}

#[derive(Debug, PartialEq, Eq)]
enum ChangeTreeRow<'a> {
    Folder(&'a str, usize),
    File(usize, usize),
}

fn change_tree_rows<'a>(files: &'a [FileChange], collapsed: &HashSet<String>) -> Vec<ChangeTreeRow<'a>> {
    let mut sorted: Vec<_> = files.iter().enumerate().collect();
    sorted.sort_unstable_by(|(_, a), (_, b)| a.path.cmp(&b.path));
    let mut folders = HashSet::new();
    let mut rows = Vec::new();
    'file: for (index, file) in sorted {
        let mut depth = 0;
        for (offset, _) in file.path.match_indices('/') {
            let folder = &file.path[..offset];
            if folders.insert(folder) {
                rows.push(ChangeTreeRow::Folder(folder, depth));
            }
            if collapsed.contains(folder) {
                continue 'file;
            }
            depth += 1;
        }
        rows.push(ChangeTreeRow::File(index, depth));
    }
    rows
}

pub(crate) fn lane_geometry(index: usize, count: usize, extends_below: bool) -> (f32, f32, f32) {
    let mid = ROW_H / 2.;
    let top = if index == 0 { mid } else { 0. };
    let bottom = if index + 1 >= count && !extends_below {
        mid
    } else {
        0.
    };
    let dot_top = (ROW_H - 8.) / 2.;
    (top, bottom, dot_top)
}

impl Workbench {
    pub(crate) fn scm_reset_workspace(&mut self) {
        let s = &mut self.scm;
        s.workspace_generation = s.workspace_generation.wrapping_add(1);
        s.repo = None;
        s.status = RepoStatus::default();
        s.log.clear();
        s.graph = Default::default();
        s.commit_open = None;
        s.commit_files.clear();
        s.incoming.clear();
        s.incoming_head = None;
        s.incoming_dismissed = None;
        s.incoming_open = None;
        s.incoming_files.clear();
        s.runs.clear();
        s.workflows.clear();
        s.actions_loaded = false;
        s.refreshing = false;
        s.again = false;
        s.fetching = false;
        s.last_fetch = None;
        s.last_refresh = None;
        s.busy = None;
        s.error = None;
        s.repo_error = None;
        s.note = None;
        s.confirm_discard = None;
        s.confirm_uninit = false;
        s.marks = Rc::default();
        s.dirty_dirs = Rc::default();
        s.row_limits = [MAX_ROWS; 4];
        s.collapsed_folders = Default::default();
        s.msg_non_empty = false;
        s.clear_message = true;
    }

    pub(crate) fn scm_refresh(&mut self, cx: &mut Context<Self>) {
        let Some(root) = self.root.clone() else { return };
        if self.scm.refreshing || self.scm.is_busy() {
            self.scm.again = true;
            return;
        }
        self.scm.refreshing = true;
        self.scm.again = false;
        let generation = self.scm.workspace_generation;
        self.scm.last_refresh = Some(std::time::Instant::now());
        let with_log = self.scm.show_graph;
        let wsl = self.wsl.clone();
        cx.spawn(async move |this, cx| {
            let snap = cx.background_spawn(async move { snapshot(&root, with_log, wsl.as_ref()) }).await;
            _ = this.update(cx, |this, cx| {
                if this.scm.workspace_generation != generation {
                    return;
                }
                let s = &mut this.scm;
                s.refreshing = false;
                if s.is_busy() {
                    s.again = true;
                    return;
                }
                let snap = match snap {
                    Ok(snap) => snap,
                    Err(error) => {
                        s.repo_error = Some(error);
                        cx.notify();
                        if s.again {
                            this.scm_refresh(cx);
                        }
                        return;
                    }
                };
                s.repo_error = None;
                let mut marks = HashMap::new();
                let mut dirty_dirs = HashSet::new();
                if let Some(repo) = &snap.repo {
                    for (ch, mark) in snap
                        .status
                        .changes
                        .iter()
                        .chain(&snap.status.untracked)
                        .chain(&snap.status.staged)
                        .map(|ch| (ch, ch.code))
                        .chain(snap.status.conflicts.iter().map(|ch| (ch, '!')))
                    {
                        let full = repo.join(&ch.path);
                        let key = |path: &std::path::Path| path.to_string_lossy().replace('/', "\\").to_lowercase();
                        marks.entry(key(&full)).or_insert(mark);
                        let mut p = full.parent();
                        while let Some(d) = p {
                            if !d.starts_with(repo) {
                                break;
                            }
                            dirty_dirs.insert(key(d));
                            p = d.parent();
                        }
                    }
                }
                s.marks = Rc::new(marks);
                s.dirty_dirs = Rc::new(dirty_dirs);
                if s.repo != snap.repo {
                    s.log.clear();
                    s.graph = Default::default();
                    s.commit_open = None;
                    s.commit_files.clear();
                    s.confirm_discard = None;
                }
                s.repo = snap.repo;
                s.status = snap.status;
                if let Some(log) = snap.log {
                    s.graph = crate::scm_graph::compute_graph(&log);
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

    fn scm_has_dirty_repo_docs(&self) -> bool {
        let Some(repo) = self.scm.repo.as_deref() else {
            return false;
        };
        self.docs.iter().any(|d| {
            d.dirty
                && !d.read_only
                && !d.flux_lazy
                && d.image.is_none()
                && d.diff.is_none()
                && d.editor.is_some()
                && crate::panel::path_in_repo(repo, &d.path)
        })
    }

    pub(crate) fn scm_run(&mut self, label: &str, steps: Vec<Step>, cx: &mut Context<Self>, done: impl FnOnce(&mut Self, &str, &mut Context<Self>) + 'static) {
        let Some(dir) = self.scm.repo.clone().or_else(|| self.root.clone()) else { return };
        if self.scm.is_busy() {
            self.status = tr("Source Control is busy, try again in a moment").into();
            cx.notify();
            return;
        }
        let reload = steps.iter().any(|step| {
            matches!(
                step,
                Step::Remote(RemoteAction::Pull | RemoteAction::Sync) | Step::Resolve { .. }
            )
        });
        let generation = self.scm.workspace_generation;
        self.scm.busy = Some(label.to_string().into());
        self.scm.error = None;
        self.scm.note = None;
        self.scm.confirm_discard = None;
        cx.notify();
        let wsl = self.wsl.clone();
        cx.spawn(async move |this, cx| {
            let (saved, res) = cx
                .background_spawn(async move {
                    let mut saved = Vec::new();
                    let mut last = String::new();
                    for step in &steps {
                        match run_step(&dir, step, wsl.as_ref()) {
                            Ok(out) => {
                                if let Step::Save { path, gen, .. } = step {
                                    saved.push((path.clone(), *gen));
                                }
                                last = out;
                            }
                            Err(e) => return (saved, Err::<String, String>(e)),
                        }
                    }
                    (saved, Ok(last))
                })
                .await;
            _ = this.update(cx, |this, cx| {
                if this.scm.workspace_generation != generation {
                    return;
                }
                for (path, gen) in saved {
                    if let Some(d) = this
                        .docs
                        .iter_mut()
                        .find(|d| crate::panel::same_path(&d.path, &path) && d.gen == gen)
                    {
                        d.dirty = false;
                    }
                }
                this.scm.busy = None;
                match res {
                    Ok(out) => done(this, &out, cx),
                    Err(e) => {
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
                if reload {
                    this.agent.reloads.extend(
                        this.docs
                            .iter()
                            .filter(|d| !d.dirty && d.diff.as_ref().is_none_or(|x| !x.virtual_tab))
                            .map(|d| d.path.clone()),
                    );
                    this.refresh_tree(cx);
                }
                this.scm_refresh(cx);
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn scm_commit(&mut self, after: After, amend: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.scm_commit_with_options(after, amend, false, window, cx);
    }

    fn scm_commit_with_options(&mut self, after: After, amend: bool, signoff: bool, window: &mut Window, cx: &mut Context<Self>) {
        let msg = self.scm.message.read(cx).value().trim().to_string();
        let has_dirty_docs = self.scm_has_dirty_repo_docs();
        let smart_all = match self.scm.status.check_smart_commit(&msg, amend, has_dirty_docs) {
            Ok(v) => v,
            Err(key) => {
                self.scm.error = Some(tr(key).to_string());
                if msg.is_empty() {
                    self.scm.message.focus_handle(cx).focus(window, cx);
                }
                cx.notify();
                return;
            }
        };
        if after != After::Nothing && !self.scm_can_push(cx) {
            return;
        }
        let mut steps = Vec::new();
        if let Some(repo) = self.scm.repo.as_deref() {
            for d in &self.docs {
                if !d.dirty
                    || d.read_only
                    || d.flux_lazy
                    || d.image.is_some()
                    || d.diff.is_some()
                    || !crate::panel::path_in_repo(repo, &d.path)
                {
                    continue;
                }
                if let Some(ed) = d.editor.as_ref() {
                    steps.push(Step::Save {
                        path: d.path.clone(),
                        gen: d.gen,
                        text: ed.read(cx).value().to_string(),
                        crlf: d.crlf,
                        nbt: d.nbt_format,
                    });
                }
            }
        }
        if smart_all {
            steps.push(git_step(&["add", "-A"]));
        }
        steps.push(Step::Commit {
            message: msg.clone(),
            amend,
            signoff,
        });
        self.scm_run(tr("Committing…"), steps, cx, move |this, _, cx| {
            if this.scm.message.read(cx).value().trim() == msg {
                this.scm.clear_message = true;
            }
            this.status = tr("Committed").into();
            this.scm_after_commit(after, cx);
        });
    }

    fn scm_after_commit(&mut self, after: After, cx: &mut Context<Self>) {
        match after {
            After::Nothing => {}
            After::Push => self.scm_run(tr("Pushing…"), vec![Step::Remote(RemoteAction::Push)], cx, |this, _, _| {
                this.status = tr("Committed and pushed").into();
            }),
            After::Sync => self.scm_sync(cx),
        }
    }

    fn scm_can_push(&mut self, cx: &mut Context<Self>) -> bool {
        if self.scm.status.branch.is_some() {
            return true;
        }
        self.scm.error = Some(tr("Switch to a branch before pushing or syncing.").to_string());
        cx.notify();
        false
    }

    pub(crate) fn scm_sync(&mut self, cx: &mut Context<Self>) {
        self.scm_run(tr("Syncing…"), vec![Step::Remote(RemoteAction::Sync)], cx, |this, _, _| {
            this.status = tr("Synced with the remote").into();
        });
    }

    fn scm_create_pr(&mut self, cx: &mut Context<Self>) {
        if !crate::github::gh_on_path() {
            self.scm.error = Some("Pull requests need the GitHub CLI (gh). Install it from cli.github.com, then sign in under Settings.".into());
            cx.notify();
            return;
        }
        let mut steps = vec![Step::Remote(RemoteAction::Push)];
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
        let generation = self.scm.workspace_generation;
        let dir_guard = dir.clone();
        let wsl = self.wsl.clone();
        cx.spawn(async move |this, cx| {
            let (wf, runs) = cx
                .background_spawn(async move {
                    let wf = run_step(&dir, &Step::Gh(["workflow", "list", "--json", "id,name,state"].map(String::from).to_vec()), wsl.as_ref());
                    let runs = run_step(
                        &dir,
                        &Step::Gh(["run", "list", "-L", "6", "--json", "displayTitle,workflowName,status,conclusion,headBranch,url"].map(String::from).to_vec()),
                        wsl.as_ref(),
                    );
                    (wf, runs)
                })
                .await;
            _ = this.update(cx, |this, cx| {
                if this.scm.workspace_generation != generation || this.scm.repo.as_ref() != Some(&dir_guard) {
                    return;
                }
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
            cx.spawn(async move |this, cx| {
                cx.background_executor().timer(std::time::Duration::from_secs(4)).await;
                _ = this.update(cx, |this, cx| this.scm_load_actions(cx));
            })
            .detach();
        });
    }

    pub(crate) fn scm_paths(
        &mut self,
        group: Group,
        path: Option<String>,
        discard: bool,
        cx: &mut Context<Self>,
    ) {
        let Some(repo) = self.scm.repo.as_ref() else {
            return;
        };
        if self.scm.is_busy() {
            return;
        }
        let files: Vec<_> = self
            .scm
            .status
            .files(group)
            .iter()
            .filter(|file| path.as_ref().is_none_or(|path| *path == file.path))
            .cloned()
            .collect();
        if files.is_empty() {
            return;
        }
        if discard {
            let selection = (group, path, files.clone());
            if self.scm.confirm_discard.as_ref() != Some(&selection) {
                self.scm.confirm_discard = Some(selection);
                cx.notify();
                return;
            }
            if crate::scm_git::file_paths(&files).iter().any(|p| {
                let full = repo.join(p);
                self.docs.iter().any(|doc| doc.dirty && crate::panel::same_path(&doc.path, &full))
            }) {
                self.scm.error =
                    Some(tr("Save or close the modified editor before discarding.").to_string());
                cx.notify();
                return;
            }
        }
        let label = if discard {
            tr("Discarding…")
        } else if group == Group::Staged {
            tr("Unstaging…")
        } else {
            tr("Staging…")
        };
        self.scm_run(
            label,
            vec![Step::Paths {
                group,
                files,
                discard,
            }],
            cx,
            move |this, _, cx| {
                if discard {
                    this.agent.reloads.extend(
                        this.docs
                            .iter()
                            .filter(|doc| !doc.dirty && doc.diff.as_ref().is_none_or(|x| !x.virtual_tab))
                            .map(|doc| doc.path.clone()),
                    );
                    this.refresh_tree(cx);
                }
            },
        );
    }

    fn scm_diff_key(repo: &std::path::Path, group: Group, path: &str) -> PathBuf {
        PathBuf::from(format!(
            "flux-diff://{}/{}/{}",
            repo.display(),
            group as u8,
            path
        ))
    }

    fn scm_show_diff(
        &mut self,
        ch: FileChange,
        group: Group,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(repo) = self.scm.repo.clone() else {
            return;
        };
        let generation = self.scm.workspace_generation;
        let file_name = ch.path.rsplit('/').next().unwrap_or(&ch.path).to_string();
        let badge = match group {
            Group::Staged => tr("Index"),
            Group::Conflicts => tr("Conflict"),
            Group::Changes | Group::Untracked => tr("Working Tree"),
        };
        let virtual_key = Self::scm_diff_key(&repo, group, &ch.path);
        let wsl = self.wsl.clone();
        cx.spawn_in(window, async move |this, cx| {
            let dir = repo.clone();
            let file = ch.clone();
            let result = cx
                .background_spawn(async move {
                    crate::scm_git::file_diff(&dir, &file, group, wsl.as_ref())
                })
                .await;
            _ = this.update_in(cx, |this, window, cx| {
                if this.scm.workspace_generation != generation
                    || this.scm.repo.as_ref() != Some(&repo)
                {
                    return;
                }
                let (patch, conflict_text, target) = match result {
                    Ok(diff) => diff,
                    Err(error) => {
                        this.scm.error = Some(error);
                        cx.notify();
                        return;
                    }
                };
                this.open_scm_diff_tab(
                    crate::diff_view::ScmDiffSpec {
                        virtual_key,
                        tab_title: format!("{file_name} ({badge})"),
                        display_name: ch.path.clone(),
                        badge: badge.to_string(),
                        patch,
                        conflict_text,
                        target,
                        scm_stage: Some((repo, ch.path, group)),
                        multi_file: false,
                    },
                    window,
                    cx,
                );
            });
        })
        .detach();
    }

    fn scm_toggle_commit(&mut self, hash: String, cx: &mut Context<Self>) {
        if self.scm.commit_open.as_deref() == Some(hash.as_str()) {
            self.scm.commit_open = None;
            return cx.notify();
        }
        self.scm.commit_open = Some(hash.clone());
        cx.notify();
        if self.scm.commit_files.contains_key(&hash) {
            return;
        }
        let Some(dir) = self.scm.repo.clone() else { return };
        let generation = self.scm.workspace_generation;
        let dir_guard = dir.clone();
        let wsl = self.wsl.clone();
        cx.spawn(async move |this, cx| {
            let h = hash.clone();
            let result = cx
                .background_spawn(async move {
                    run_step(
                        &dir,
                        &Step::Git(
                            [
                                "-c",
                                "core.quotepath=off",
                                "show",
                                "--format=",
                                "--name-status",
                                "-m",
                                "--first-parent",
                                "-z",
                                &h,
                            ]
                            .map(String::from)
                            .to_vec(),
                            None,
                        ),
                        wsl.as_ref(),
                    )
                    .map(|s| crate::scm_incoming::parse_name_status(&s))
                })
                .await;
            _ = this.update(cx, |this, cx| {
                if this.scm.workspace_generation != generation
                    || this.scm.repo.as_ref() != Some(&dir_guard)
                {
                    return;
                }
                match result {
                    Ok(files) => {
                        this.scm.commit_files.insert(hash, files);
                    }
                    Err(e) => {
                        if this.scm.commit_open.as_deref() == Some(hash.as_str()) {
                            this.scm.commit_open = None;
                        }
                        this.scm.error = Some(e);
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn scm_show_commit_file(
        &mut self,
        hash: String,
        short: String,
        path: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(dir) = self.scm.repo.clone() else { return };
        let generation = self.scm.workspace_generation;
        let dir_guard = dir.clone();
        let wsl = self.wsl.clone();
        let file_name = path.rsplit('/').next().unwrap_or(&path).to_string();
        let full_path = dir.join(&path);
        let target = if wsl.is_some() || full_path.is_file() {
            Some(full_path)
        } else {
            None
        };
        let virtual_key = PathBuf::from(format!("flux-diff://commit/{hash}/{path}"));
        let p_bg = path.clone();
        cx.spawn_in(window, async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    crate::scm_git::show_commit_diff(&dir, &hash, Some(&p_bg), true, wsl.as_ref())
                })
                .await;
            _ = this.update_in(cx, |this, window, cx| {
                if this.scm.workspace_generation != generation
                    || this.scm.repo.as_ref() != Some(&dir_guard)
                {
                    return;
                }
                let patch = match result {
                    Ok(patch) => patch,
                    Err(e) => {
                        this.scm.error = Some(e);
                        cx.notify();
                        return;
                    }
                };
                this.open_scm_diff_tab(
                    crate::diff_view::ScmDiffSpec {
                        virtual_key,
                        tab_title: format!("{file_name} ({short})"),
                        display_name: path,
                        badge: short,
                        patch,
                        conflict_text: None,
                        target,
                        scm_stage: None,
                        multi_file: false,
                    },
                    window,
                    cx,
                );
            });
        })
        .detach();
    }

    fn scm_show_commit_all(
        &mut self,
        hash: String,
        short: String,
        subject: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(dir) = self.scm.repo.clone() else {
            return;
        };
        let generation = self.scm.workspace_generation;
        let dir_guard = dir.clone();
        let wsl = self.wsl.clone();
        let virtual_key = PathBuf::from(format!("flux-diff://commit/{hash}"));
        cx.spawn_in(window, async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    crate::scm_git::show_commit_diff(&dir, &hash, None, true, wsl.as_ref())
                })
                .await;
            _ = this.update_in(cx, |this, window, cx| {
                if this.scm.workspace_generation != generation
                    || this.scm.repo.as_ref() != Some(&dir_guard)
                {
                    return;
                }
                let patch = match result {
                    Ok(patch) => patch,
                    Err(e) => {
                        this.scm.error = Some(e);
                        cx.notify();
                        return;
                    }
                };
                this.open_scm_diff_tab(
                    crate::diff_view::ScmDiffSpec {
                        virtual_key,
                        tab_title: format!("{short} · {subject}"),
                        display_name: subject,
                        badge: short,
                        patch,
                        conflict_text: None,
                        target: None,
                        scm_stage: None,
                        multi_file: true,
                    },
                    window,
                    cx,
                );
            });
        })
        .detach();
    }

    pub(crate) fn scm_drag_pane(&mut self, y: f32) {
        let Some(drag) = self.scm.pane_drag.as_ref() else {
            return;
        };
        for (id, h) in apply_sash_drag(drag, y) {
            self.scm.pane_h[id as usize] = Some(h);
        }
    }


    pub(crate) fn render_scm(&self, cx: &mut Context<Self>) -> AnyElement {
        let s = &self.scm;
        if self.root.is_none() {
            return div().p_3().text_sm().text_color(c(theme::FG_DIM)).child(tr("Open a folder to use Source Control.")).into_any_element();
        }
        if s.repo.is_none() {
            let status = if s.refreshing || s.last_refresh.is_none() {
                tr("Checking for a Git repository…")
            } else {
                tr("This folder is not a Git repository.")
            };
            return v_flex()
                .p_3()
                .gap_3()
                .text_sm()
                .child(
                    div()
                        .text_color(c(theme::FG_MUTED))
                        .child(s.repo_error.clone().unwrap_or_else(|| status.to_string())),
                )
                .when(!s.refreshing && s.repo_error.is_none(), |v| {
                    v.child(
                        Button::new("scm-init")
                            .label(tr("Initialize Repository"))
                            .primary()
                            .small()
                            .w_full()
                            .disabled(s.is_busy())
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.scm_run(
                                    tr("Initializing…"),
                                    vec![git_step(&["init"])],
                                    cx,
                                    |this, _, _| {
                                        this.status = tr("Git repository created").into();
                                    },
                                );
                            })),
                    )
                })
                .when_some(s.busy.clone().or_else(|| s.fetching.then(|| tr("Fetching…").into())), |v, status| {
                    v.child(div().text_xs().child(status))
                })
                .when_some(s.error.clone(), |v, error| {
                    v.child(div().text_color(c(theme::DANGER)).child(error))
                })
                .into_any_element();
        }
        let st = &s.status;
        let has_dirty_docs = self.scm_has_dirty_repo_docs();
        let smart_all = st.staged.is_empty()
            && (!st.changes.is_empty() || !st.untracked.is_empty() || has_dirty_docs);
        let can_commit = !s.is_busy()
            && st
                .check_smart_commit(&s.message.read(cx).value(), false, has_dirty_docs)
                .is_ok();
        let commit_row = h_flex()
            .w_full()
            .gap_px()
            .child(
                Button::new("scm-commit")
                    .icon(Icon::new(IconName::Check))
                    .label(if smart_all {
                        tr("Commit All")
                    } else {
                        tr("Commit")
                    })
                    .disabled(!can_commit)
                    .primary()
                    .small()
                    .flex_1()
                    .tooltip(if smart_all {
                        tr("Stage everything and commit")
                    } else {
                        tr("Commit the staged changes (Enter)")
                    })
                    .on_click(
                        cx.listener(|this, _, w, cx| this.scm_commit(After::Nothing, false, w, cx)),
                    ),
            )
            .child(
                Button::new("scm-commit-more")
                    .icon(Icon::new(IconName::ChevronDown))
                    .disabled(s.is_busy())
                    .primary()
                    .small()
                    .dropdown_menu(|m: PopupMenu, _, _| {
                        m.min_w(px(220.))
                            .menu(tr("Commit & Push"), Box::new(ScmCommitPush))
                            .menu(tr("Commit & Sync"), Box::new(ScmCommitSync))
                            .separator()
                            .menu(tr("Commit (Amend)"), Box::new(ScmCommitAmend))
                            .menu(tr("Commit (Sign Off)"), Box::new(ScmCommitSignoff))
                    }),
            );

        let mut top = v_flex().w_full().min_w_0().flex_none().pt_1p5();
        if s.confirm_uninit {
            let home = std::env::var("USERPROFILE").unwrap_or_default();
            let mut where_: String = s.repo.as_ref().map(|r| r.display().to_string()).unwrap_or_default();
            let full: SharedString = where_.clone().into();
            if !home.is_empty() && where_.starts_with(&home) {
                where_ = format!("~{}", &where_[home.len()..]);
            }
            let folder = s.repo.as_ref().and_then(|r| r.file_name()).map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            top = top.child(
                v_flex()
                    .flex_none()
                    .ml_3()
                    .mr(px(SCROLLBAR_PAD))
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
            let incoming_el: AnyElement = if s.incoming_shown {
                div()
                    .w_full()
                    .min_w_0()
                    .max_h(theme::u(180.))
                    .overflow_y_scrollbar()
                    .id("scm-in-scroll")
                    .scroll_bounce(false)
                    .child(card)
                    .into_any_element()
            } else {
                card
            };
            top = top.child(incoming_el);
        }
        top = top.child(
            v_flex()
                .w_full()
                .min_w_0()
                .flex_none()
                .px_3()
                .pb_2()
                .gap_1p5()
                .child(div().key_context("ScmMessage").child(crate::ui::McInput::new(&s.message).small()))
                .child(commit_row)
                .when_some(s.busy.clone().or_else(|| s.fetching.then(|| tr("Fetching…").into())), |v, b| v.child(div().text_xs().text_color(c(theme::BLUE)).child(b)))
                .when_some(s.error.clone().or_else(|| s.repo_error.clone()), |v, e| {
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

        if let Some((group, path, files)) = &s.confirm_discard {
            let group = *group;
            let path = path.clone();
            let detail = path.clone().unwrap_or_else(|| {
                if files.len() == 1 {
                    files[0].path.clone()
                } else {
                    crate::trf!("Discard {n} files in {group}?"; n => files.len(), group => group_title(group)).to_string()
                }
            });
            top = top.child(
                v_flex()
                    .mx_3()
                    .mb_2()
                    .p_2()
                    .gap_1()
                    .border_1()
                    .border_color(c(theme::BORDER))
                    .text_xs()
                    .child(div().text_color(c(theme::FG_STRONG)).child(detail))
                    .child(div().text_color(c(theme::FG_MUTED)).child(match group {
                        Group::Untracked => tr("Untracked files will be deleted."),
                        Group::Staged if st.head.is_none() => {
                            tr("Remove these files from the index and working tree.")
                        }
                        Group::Staged => {
                            tr("Restore the index and working tree to the last commit.")
                        }
                        Group::Conflicts => {
                            tr("Restore the original merge conflict in the working tree.")
                        }
                        Group::Changes => tr("Restore the working tree from the index."),
                    }))
                    .child(
                        h_flex()
                            .gap_2()
                            .child(
                                Button::new("scm-discard-yes")
                                    .label(tr("Discard"))
                                    .danger()
                                    .xsmall()
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.scm_paths(group, path.clone(), true, cx)
                                    })),
                            )
                            .child(
                                Button::new("scm-discard-no")
                                    .label(tr("Cancel"))
                                    .ghost()
                                    .xsmall()
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.scm.confirm_discard = None;
                                        cx.notify();
                                    })),
                            ),
                    ),
            );
        }

        let section = |id: &'static str, title: String, count: usize, open: bool| {
            scm_row()
                .id(id)
                .pl_2()
                .pr(px(SCROLLBAR_PAD))
                .gap_1()
                .items_center()
                .cursor_pointer()
                .hover(|s| s.bg(c(theme::BG_HOVER)))
                .text_xs()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(c(theme::FG_MUTED))
                .child(
                    Icon::new(if open {
                        IconName::ChevronDown
                    } else {
                        IconName::ChevronRight
                    })
                    .flex_none()
                    .size(theme::u(14.)),
                )
                .child(div().flex_1().min_w_0().truncate().child(title))
                .when(count > 0, |h| {
                    h.child(
                        div()
                            .flex_none()
                            .px_1p5()
                            .rounded(px(8.))
                            .bg(c(theme::BG_ELEVATED))
                            .text_color(c(theme::FG))
                            .child(count.to_string()),
                    )
                })
        };

        let active_doc_path = self
            .active
            .and_then(|i| self.docs.get(i))
            .map(|d| d.path.clone());
        let file_row =
            |i: usize, depth: usize, group: Group, ch: &FileChange, this: &Self, cx: &mut Context<Self>| {
                let kind = group as u8;
                let name = ch.path.rsplit('/').next().unwrap_or(&ch.path).to_string();
                let dir = if this.scm.show_tree {
                    String::new()
                } else {
                    ch.path.rsplit_once('/').map(|(parent, _)| parent.to_string()).unwrap_or_default()
                };
                let col = if group == Group::Conflicts {
                    theme::DANGER
                } else {
                    code_color(ch.code)
                };
                let deleted = ch.code == 'D';
                let full = this.scm.repo.as_ref().map(|r| r.join(&ch.path));
                let staged = kind == 1;
                let busy = this.scm.is_busy();
                let is_active = this.scm.repo.as_ref().is_some_and(|repo| {
                    active_doc_path.as_ref() == Some(&Self::scm_diff_key(repo, group, &ch.path))
                });
                let (ch_click, discard_path, full_open) = (ch.clone(), ch.path.clone(), full);
                scm_row()
                    .id(("scm-f", i * 4 + kind as usize))
                    .group("scm-row")
                    .relative()
                    .pl(theme::u(22. + depth.min(12) as f32 * 12.))
                    .pr(px(SCROLLBAR_PAD))
                    .gap_1p5()
                    .items_center()
                    .text_size(theme::u(13.))
                    .cursor_pointer()
                    .when(is_active, |h| h.bg(c(theme::SELECT)))
                    .hover(|s| s.bg(c(theme::BG_HOVER)))
                    .child(crate::files::file_icon_el(&name, theme::u(14.)))
                    .child(
                        div()
                            .flex_shrink(1.)
                            .min_w_0()
                            .truncate()
                            .text_color(c(theme::FG))
                            .when(deleted, |d| d.line_through())
                            .child(name),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_xs()
                            .text_color(c(theme::FG_DIM))
                            .child(dir),
                    )
                    .child(
                        div()
                            .flex_none()
                            .w(theme::u(12.))
                            .text_right()
                            .text_xs()
                            .font_weight(FontWeight::BOLD)
                            .text_color(c(col))
                            .child(ch.code.to_string()),
                    )
                    // Overlay actions on hover so unhovered rows use the full width for filename and path.
                    .child(
                        h_flex()
                            .absolute()
                            .top_0()
                            .bottom_0()
                            .right(px(SCROLLBAR_PAD) + theme::u(14.))
                            .items_center()
                            .gap_px()
                            .pl_1p5()
                            .pr_0p5()
                            .bg(c(theme::BG_HOVER))
                            .invisible()
                            .group_hover("scm-row", |s| s.visible())
                            .when(!deleted, |h| {
                                h.child(
                                    Button::new(("scm-open", i * 4 + kind as usize))
                                        .icon(Icon::new(Ico::FileCode))
                                        .ghost()
                                        .xsmall()
                                        .tooltip(tr("Open file"))
                                        .on_click(cx.listener(move |this, _, w, cx| {
                                            cx.stop_propagation();
                                            if let Some(f) =
                                                full_open.clone().filter(|f| f.is_file())
                                            {
                                                this.open_file(f, w, cx);
                                            }
                                        })),
                                )
                            })
                            .child(
                                Button::new(("scm-discard", i * 4 + kind as usize))
                                    .icon(Icon::new(Ico::Undo2))
                                    .ghost()
                                    .xsmall()
                                    .disabled(busy)
                                    .tooltip(tr("Discard"))
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        cx.stop_propagation();
                                        this.scm_paths(group, Some(discard_path.clone()), true, cx);
                                    })),
                            )
                            .child({
                                let p = ch.path.clone();
                                Button::new(("scm-stage", i * 4 + kind as usize))
                                    .icon(Icon::new(if staged {
                                        IconName::Minus
                                    } else {
                                        IconName::Plus
                                    }))
                                    .ghost()
                                    .xsmall()
                                    .disabled(busy)
                                    .tooltip(if staged { tr("Unstage") } else { tr("Stage") })
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        cx.stop_propagation();
                                        this.scm_paths(group, Some(p.clone()), false, cx);
                                    }))
                            }),
                    )
                    .on_click(cx.listener(move |this, _, w, cx| {
                        this.scm_show_diff(ch_click.clone(), group, w, cx);
                    }))
            };

        struct PaneBuilt {
            spec: PaneSpec,
            scroll_id: &'static str,
            header: AnyElement,
            rows: Option<AnyElement>,
        }

        let row_h = f32::from(theme::u(ROW_H));
        let graph_h = f32::from(theme::u(crate::scm_graph::GRAPH_ROW_H));
        let header_h = row_h;
        let flex_min = |natural: f32, unit: f32, min_rows: f32| {
            (header_h + min_rows * unit).min(natural).max(header_h)
        };
        let mut panes: Vec<PaneBuilt> = Vec::with_capacity(6);

        for (group, pane, open, id) in [
            (
                Group::Conflicts,
                ScmPane::Conflicts,
                s.show_conflicts,
                "scm-conflicts",
            ),
            (Group::Staged, ScmPane::Staged, s.show_staged, "scm-staged"),
            (
                Group::Changes,
                ScmPane::Changes,
                s.show_changes,
                "scm-changes",
            ),
            (
                Group::Untracked,
                ScmPane::Untracked,
                s.show_untracked,
                "scm-untracked",
            ),
        ] {
            if !s.show_changes_view {
                break;
            }
            let files = st.files(group);
            if files.is_empty() && !(group == Group::Changes && st.count() == 0) {
                continue;
            }
            let staged = group == Group::Staged;
            let hdr = section(id, group_title(group).into(), files.len(), open)
                .pl_3()
                .when(!files.is_empty(), |h| {
                    h.child(
                        Button::new(("scm-group-stage", group as usize))
                            .icon(Icon::new(if staged {
                                IconName::Minus
                            } else {
                                IconName::Plus
                            }))
                            .ghost()
                            .xsmall()
                            .disabled(s.is_busy())
                            .tooltip(if staged {
                                tr("Unstage All")
                            } else {
                                tr("Stage All")
                            })
                            .on_click(cx.listener(move |this, _, _, cx| {
                                cx.stop_propagation();
                                this.scm_paths(group, None, false, cx);
                            })),
                    )
                    .child(
                        Button::new(("scm-group-discard", group as usize))
                            .icon(Icon::new(Ico::Undo2))
                            .ghost()
                            .xsmall()
                            .disabled(s.is_busy())
                            .tooltip(tr("Discard All"))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                cx.stop_propagation();
                                this.scm_paths(group, None, true, cx);
                            })),
                    )
                })
                .on_click(cx.listener(move |this, _, _, cx| {
                    let expanded = match group {
                        Group::Conflicts => &mut this.scm.show_conflicts,
                        Group::Staged => &mut this.scm.show_staged,
                        Group::Changes => &mut this.scm.show_changes,
                        Group::Untracked => &mut this.scm.show_untracked,
                    };
                    *expanded = !*expanded;
                    cx.notify();
                }))
                .into_any_element();
            let limit = s.row_limits[group as usize];
            let more = files.len().saturating_sub(limit).min(MAX_ROWS);
            let displayed = &files[..files.len().min(limit)];
            let collapsed = &s.collapsed_folders[group as usize];
            let tree = (s.show_tree && open).then(|| change_tree_rows(displayed, collapsed));
            let rows_n = (tree.as_ref().map_or(displayed.len(), Vec::len) + usize::from(more > 0)).max(1);
            let extra = if group == Group::Untracked && files.len() > MAX_ROWS {
                190.
            } else {
                0.
            };
            let natural_h = header_h + rows_n as f32 * row_h + extra;
            let rows = open.then(|| {
                let mut col = v_flex().w_full().min_w_0().pb_1();
                if group == Group::Untracked && files.len() > MAX_ROWS {
                    let mut by: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
                    for ch in files {
                        if let Some((top, _)) = ch.path.split_once('/') {
                            *by.entry(top).or_default() += 1;
                        }
                    }
                    let mut top_dirs: Vec<(&str, usize)> = by.into_iter().collect();
                    top_dirs.sort_by_key(|a| std::cmp::Reverse(a.1));
                    col = col.child(
                        v_flex()
                            .flex_shrink_0()
                            .ml_3()
                            .mr(px(SCROLLBAR_PAD))
                            .my_1()
                            .p_2()
                            .gap_1()
                            .rounded(px(6.))
                            .border_1()
                            .border_color(c(theme::BORDER))
                            .bg(c(theme::BG_ELEVATED))
                            .text_xs()
                            .child(div().text_color(c(theme::FG_STRONG)).child(crate::trf!("{n} untracked files"; n => files.len())))
                            .child(div().text_color(c(theme::FG_DIM)).child(tr("Folders with the most of them. Ignore the ones git should not track (they go to .gitignore).")))
                            .children(top_dirs.into_iter().take(6).enumerate().map(|(i, (dir, n))| {
                                let pattern = format!("{dir}/");
                                h_flex()
                                    .gap_2()
                                    .h(theme::u(ROW_H))
                                    .items_center()
                                    .child(Icon::new(IconName::Folder).size(theme::u(13.)).text_color(c(theme::FOLDER)))
                                    .child(div().flex_1().min_w_0().truncate().text_color(c(theme::FG)).child(pattern.clone()))
                                    .child(div().flex_none().text_color(c(theme::FG_DIM)).child(n.to_string()))
                                    .child(Button::new(("scm-ignore", i)).label(tr("Ignore")).xsmall().on_click(cx.listener(move |this, _, _, cx| this.scm_ignore(pattern.clone(), cx))))
                            })),
                    );
                }
                if let Some(tree) = tree {
                    for (row, entry) in tree.into_iter().enumerate() {
                        match entry {
                            ChangeTreeRow::File(i, depth) => {
                                col = col.child(file_row(i, depth, group, &displayed[i], self, cx));
                            }
                            ChangeTreeRow::Folder(path, depth) => {
                                let expanded = !collapsed.contains(path);
                                let name = path.rsplit('/').next().unwrap_or(path).to_string();
                                let path = path.to_string();
                                col = col.child(
                                    h_flex()
                                        .id(("scm-folder", row * 4 + group as usize))
                                        .w_full()
                                        .min_w_0()
                                        .flex_none()
                                        .h(theme::u(ROW_H))
                                        .pl(theme::u(10. + depth.min(12) as f32 * 12.))
                                        .pr(px(SCROLLBAR_PAD))
                                        .gap_1()
                                        .cursor_pointer()
                                        .hover(|h| h.bg(c(theme::BG_HOVER)))
                                        .child(Icon::new(if expanded { IconName::ChevronDown } else { IconName::ChevronRight }).size(theme::u(12.)))
                                        .child(Icon::new(IconName::Folder).size(theme::u(14.)).text_color(c(theme::FOLDER)))
                                        .child(div().min_w_0().truncate().text_sm().child(name))
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            let folders = &mut this.scm.collapsed_folders[group as usize];
                                            if !folders.remove(&path) {
                                                folders.insert(path.clone());
                                            }
                                            cx.notify();
                                        })),
                                );
                            }
                        }
                    }
                } else {
                    for (i, ch) in displayed.iter().enumerate() {
                        col = col.child(file_row(i, 0, group, ch, self, cx));
                    }
                }
                if more > 0 {
                    col = col.child(Button::new(("scm-show-more", group as usize)).ghost().xsmall()
                        .label(crate::trf!("Show {n} more files"; n => more))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.scm.row_limits[group as usize] += more;
                            cx.notify();
                        })));
                }
                if files.is_empty() {
                    col = col.child(div().px(theme::u(18.)).py_1().text_xs().text_color(c(theme::FG_MUTED)).child(tr("No changes.")));
                }
                col.into_any_element()
            });
            panes.push(PaneBuilt {
                spec: PaneSpec {
                    id: pane,
                    flexible: open,
                    min_h: if open {
                        flex_min(natural_h, row_h, 2.)
                    } else {
                        header_h
                    },
                    natural_h,
                    saved: s.pane_h[pane as usize],
                },
                scroll_id: id,
                header: hdr,
                rows,
            });
        }

        {
            let hdr = section("scm-sec-graph", tr("Graph").to_uppercase(), 0, s.show_graph)
                .border_t_1()
                .border_color(c(theme::BORDER))
                .on_click(cx.listener(|this, _, _, cx| {
                    this.scm.show_graph = !this.scm.show_graph;
                    if this.scm.show_graph {
                        this.scm_refresh(cx);
                    }
                    cx.notify();
                }))
                .into_any_element();
            let open_extra = s
                .commit_open
                .as_ref()
                .map(|h| {
                    (1 + s.commit_files.get(h).map(|f| f.len().min(60)).unwrap_or(1)) as f32 * row_h
                })
                .unwrap_or(0.);
            let natural = header_h + s.log.len().max(1) as f32 * graph_h + open_extra;
            let flexible = s.show_graph;
            let rows = flexible.then(|| {
                let mut col = v_flex().w_full().min_w_0().pb_1();
                let n = s.log.len();
                if n == 0 {
                    col = col.child(div().flex_shrink_0().px(theme::u(18.)).py_1().text_xs().text_color(c(theme::FG_DIM)).child(tr("No commits yet.")));
                }
                for (i, cm) in s.log.iter().enumerate() {
                    let hash = cm.hash.clone();
                    let open = s.commit_open.as_deref() == Some(cm.hash.as_str());
                    let grow = s.graph.rows.get(i).cloned().unwrap_or(crate::scm_graph::GraphRow {
                        col: 0,
                        color: 0,
                        merge: cm.merge,
                        is_head: cm.is_head,
                        continues_up: i > 0,
                        continues_down: i + 1 < n,
                        row_lanes: 1,
                        pass_lanes: Vec::new(),
                        in_edges: Vec::new(),
                        out_edges: Vec::new(),
                        after_lanes: if i + 1 < n { vec![(0, 0)] } else { Vec::new() },
                    });
                    let after_lanes = grow.after_lanes.clone();
                    let badges = crate::scm_graph::sort_ref_badges(&cm.refs, cm.is_head);
                    let primary_badge = badges.first().cloned();
                    let extra_badge = badges.get(1).cloned();
                    let has_refs = !badges.is_empty();
                    col = col.child(
                        h_flex()
                            .id(("scm-c", i))
                            .w_full()
                            .min_w_0()
                            .flex_shrink_0()
                            .overflow_hidden()
                            .h(theme::u(crate::scm_graph::GRAPH_ROW_H))
                            .pr(px(SCROLLBAR_PAD))
                            .gap_1()
                            .items_center()
                            .text_size(theme::u(11.5))
                            .cursor_pointer()
                            .when(open, |h| h.bg(c(theme::SELECT)))
                            .hover(|s| s.bg(c(theme::BG_HOVER)))
                            .child(crate::scm_graph::render_graph_cell(grow))
                            .child(
                                h_flex()
                                    .flex_1()
                                    .min_w_0()
                                    .overflow_hidden()
                                    .gap_1p5()
                                    .items_center()
                                    .child(
                                        div()
                                            .flex_shrink(1.)
                                            .min_w_0()
                                            .truncate()
                                            .when(cm.is_head, |d| d.font_weight(FontWeight::SEMIBOLD).text_color(c(theme::FG_STRONG)))
                                            .when(!cm.is_head, |d| d.text_color(c(theme::FG)))
                                            .child(cm.subject.clone()),
                                    )
                                    .when(!has_refs && !cm.author.is_empty(), |h| {
                                        h.child(
                                            div()
                                                .flex_shrink(1.)
                                                .min_w_0()
                                                .max_w(theme::u(86.))
                                                .truncate()
                                                .text_size(theme::u(10.5))
                                                .text_color(c(theme::FG_DIM))
                                                .child(cm.author.clone()),
                                        )
                                    }),
                            )
                            .children(primary_badge.map(|b| {
                                let (ico, tint, text_tok) = match b.kind {
                                    crate::scm_graph::RefKind::Head => (Ico::GitBranch, theme::BLUE, theme::BLUE),
                                    crate::scm_graph::RefKind::Branch => (Ico::GitBranch, theme::ACCENT, theme::ACCENT_TEXT),
                                    crate::scm_graph::RefKind::Remote => (Ico::Cloud, theme::WARN, theme::WARN),
                                    crate::scm_graph::RefKind::Tag => (Ico::Tag, theme::WARN, theme::WARN),
                                };
                                h_flex()
                                    .flex_shrink(1.)
                                    .min_w_0()
                                    .max_w(theme::u(108.))
                                    .h(theme::u(17.))
                                    .px_1p5()
                                    .gap_1()
                                    .items_center()
                                    .rounded_full()
                                    .border_1()
                                    .border_color(c(tint).opacity(0.45))
                                    .bg(c(tint).opacity(0.18))
                                    .text_size(theme::u(10.5))
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(c(text_tok))
                                    .child(Icon::new(ico).flex_none().size(theme::u(10.)))
                                    .child(div().min_w_0().truncate().child(b.label))
                            }))
                            .children(extra_badge.map(|b| {
                                let (ico, tint) = match b.kind {
                                    crate::scm_graph::RefKind::Head => (Ico::GitBranch, theme::BLUE),
                                    crate::scm_graph::RefKind::Branch => (Ico::GitBranch, theme::ACCENT),
                                    crate::scm_graph::RefKind::Remote => (Ico::Cloud, theme::ACCENT),
                                    crate::scm_graph::RefKind::Tag => (Ico::Tag, theme::WARN),
                                };
                                h_flex()
                                    .flex_none()
                                    .h(theme::u(17.))
                                    .px_1()
                                    .items_center()
                                    .justify_center()
                                    .rounded_full()
                                    .border_1()
                                    .border_color(c(tint).opacity(0.45))
                                    .bg(c(tint).opacity(0.18))
                                    .text_color(c(tint))
                                    .child(Icon::new(ico).size(theme::u(10.)))
                            }))
                            .when(badges.len() > 2, |h| {
                                h.child(div().flex_none().text_size(theme::u(10.)).text_color(c(theme::FG_DIM)).child(format!("+{}", badges.len() - 2)))
                            })
                            .tooltip({
                                let refs = if cm.refs.is_empty() { String::new() } else { format!("\n{}", cm.refs.join(", ")) };
                                let tip = format!("{}  {}\n{} · {}{refs}", cm.short, cm.subject, cm.author, cm.when);
                                move |w, cx| Tooltip::new(tip.clone()).build(w, cx)
                            })
                            .on_click(cx.listener(move |this, _, _, cx| this.scm_toggle_commit(hash.clone(), cx))),
                    );
                    if !open {
                        continue;
                    }
                    let (h_all, short_all, subj_all) = (cm.hash.clone(), cm.short.clone(), cm.subject.clone());
                    col = col.child(
                        h_flex()
                            .w_full()
                            .min_w_0()
                            .flex_shrink_0()
                            .overflow_hidden()
                            .h(theme::u(ROW_H))
                            .pr(px(SCROLLBAR_PAD))
                            .gap_1p5()
                            .items_center()
                            .bg(c(theme::BG_ELEVATED).opacity(0.6))
                            .text_size(theme::u(10.5))
                            .text_color(c(theme::FG_DIM))
                            .child(crate::scm_graph::render_rails_cell(after_lanes.clone()))
                            .child(div().flex_none().font_weight(FontWeight::MEDIUM).text_color(c(theme::ACCENT_TEXT)).child(cm.short.clone()))
                            .child(div().flex_1().min_w_0().truncate().child(format!("{} · {}", cm.author, cm.when)))
                            .child(
                                div()
                                    .id(("scm-c-all", i))
                                    .flex_none()
                                    .px_1p5()
                                    .py(px(1.))
                                    .rounded(px(4.))
                                    .cursor_pointer()
                                    .text_color(c(theme::FG_MUTED))
                                    .hover(|s| s.bg(c(theme::BG_HOVER)).text_color(c(theme::FG_STRONG)))
                                    .child(tr("View full diff"))
                                    .on_click(cx.listener(move |this, _, w, cx| {
                                        this.scm_show_commit_all(h_all.clone(), short_all.clone(), subj_all.clone(), w, cx);
                                    })),
                            ),
                    );
                    match s.commit_files.get(&cm.hash) {
                        None => {
                            col = col.child(
                                h_flex()
                                    .w_full()
                                    .min_w_0()
                                    .flex_shrink_0()
                                    .h(theme::u(ROW_H))
                                    .pr(px(SCROLLBAR_PAD))
                                    .gap_1p5()
                                    .items_center()
                                    .text_xs()
                                    .text_color(c(theme::FG_DIM))
                                    .child(crate::scm_graph::render_rails_cell(after_lanes.clone()))
                                    .child(tr("Reading the changed files…")),
                            );
                        }
                        Some(files) if files.is_empty() => {
                            col = col.child(
                                h_flex()
                                    .w_full()
                                    .min_w_0()
                                    .flex_shrink_0()
                                    .h(theme::u(ROW_H))
                                    .pr(px(SCROLLBAR_PAD))
                                    .gap_1p5()
                                    .items_center()
                                    .text_xs()
                                    .text_color(c(theme::FG_DIM))
                                    .child(crate::scm_graph::render_rails_cell(after_lanes.clone()))
                                    .child(tr("No changed files.")),
                            );
                        }
                        Some(files) => {
                            for (k, (code, fpath)) in files.iter().enumerate().take(60) {
                                let (h_f, short_f, p_f) = (cm.hash.clone(), cm.short.clone(), fpath.clone());
                                let name = fpath.rsplit('/').next().unwrap_or(fpath).to_string();
                                let dir = fpath.rsplit_once('/').map(|(d, _)| d.replace('/', "\\")).unwrap_or_default();
                                let col_tok = code_color(*code);
                                let file_key = PathBuf::from(format!("flux-diff://commit/{}/{}", cm.hash, fpath));
                                let is_active = active_doc_path.as_ref() == Some(&file_key);
                                col = col.child(
                                    h_flex()
                                        .id(("scm-cf", i * 1000 + k))
                                        .w_full()
                                        .min_w_0()
                                        .flex_none()
                                        .overflow_hidden()
                                        .h(theme::u(ROW_H))
                                        .pr(px(SCROLLBAR_PAD))
                                        .gap_1p5()
                                        .items_center()
                                        .text_xs()
                                        .cursor_pointer()
                                        .when(is_active, |h| h.bg(c(theme::SELECT)))
                                        .hover(|s| s.bg(c(theme::BG_HOVER)))
                                        .child(crate::scm_graph::render_rails_cell(after_lanes.clone()))
                                        .child(crate::files::file_icon_el(&name, theme::u(13.)))
                                        .child(div().flex_shrink(1.).min_w_0().truncate().text_color(c(col_tok)).child(name))
                                        .child(div().flex_1().min_w_0().truncate().text_color(c(theme::FG_DIM)).child(dir))
                                        .child(div().flex_none().w(theme::u(12.)).text_right().font_weight(FontWeight::BOLD).text_color(c(col_tok)).child(code.to_string()))
                                        .on_click(cx.listener(move |this, _, w, cx| {
                                            this.scm_show_commit_file(h_f.clone(), short_f.clone(), p_f.clone(), w, cx);
                                        })),
                                );
                            }
                            if files.len() > 60 {
                                col = col.child(
                                    h_flex()
                                        .w_full()
                                        .min_w_0()
                                        .flex_shrink_0()
                                        .h(theme::u(ROW_H))
                                        .pr(px(SCROLLBAR_PAD))
                                        .gap_1p5()
                                        .items_center()
                                        .text_xs()
                                        .text_color(c(theme::FG_DIM))
                                        .child(crate::scm_graph::render_rails_cell(after_lanes.clone()))
                                        .child(crate::trf!("… and {n} more files"; n => files.len() - 60)),
                                );
                            }
                        }
                    }
                }
                col.into_any_element()
            });
            panes.push(PaneBuilt {
                spec: PaneSpec {
                    id: ScmPane::Graph,
                    flexible,
                    min_h: if flexible {
                        flex_min(natural, graph_h, 3.)
                    } else {
                        header_h
                    },
                    natural_h: natural,
                    saved: s.pane_h[ScmPane::Graph as usize],
                },
                scroll_id: "scm-scroll-graph",
                header: hdr,
                rows,
            });
        }

        {
            let hdr = section(
                "scm-sec-actions",
                "GitHub Actions (deploy)".into(),
                0,
                s.show_actions,
            )
            .on_click(cx.listener(|this, _, _, cx| {
                this.scm.show_actions = !this.scm.show_actions;
                if this.scm.show_actions {
                    this.scm_load_actions(cx);
                }
                cx.notify();
            }))
            .into_any_element();
            let rows_n = (s.workflows.len() + s.runs.len()).max(1);
            let natural = header_h + rows_n as f32 * row_h;
            let flexible = s.show_actions;
            let rows = flexible.then(|| {
                let mut col = v_flex().w_full().min_w_0().pb_1();
                if s.workflows.is_empty() && s.runs.is_empty() {
                    col = col.child(div().flex_shrink_0().px(theme::u(18.)).py_1().text_xs().text_color(c(theme::FG_DIM)).child(if s.actions_loaded { "No active workflows in this repository." } else { "Loading workflows…" }));
                }
                for (i, wf) in s.workflows.iter().enumerate() {
                    let w = wf.clone();
                    col = col.child(
                        h_flex()
                            .w_full()
                            .min_w_0()
                            .flex_shrink_0()
                            .overflow_hidden()
                            .h(theme::u(ROW_H))
                            .pl(theme::u(18.))
                            .pr(px(SCROLLBAR_PAD))
                            .gap_1p5()
                            .items_center()
                            .text_size(theme::u(11.5))
                            .child(Icon::new(Ico::Rocket).flex_none().size(theme::u(14.)).text_color(c(theme::FG_MUTED)))
                            .child(div().flex_1().min_w_0().truncate().child(wf.name.clone()))
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
                    let (mark, col_tok) = match (r.status.as_str(), r.conclusion.as_str()) {
                        ("completed", "success") => ("✓", theme::GREEN),
                        ("completed", "failure") => ("✕", theme::DANGER),
                        ("completed", _) => ("–", theme::FG_DIM),
                        _ => ("●", theme::BLUE),
                    };
                    let url = r.url.clone();
                    col = col.child(
                        h_flex()
                            .id(("scm-run", i))
                            .w_full()
                            .min_w_0()
                            .flex_shrink_0()
                            .overflow_hidden()
                            .h(theme::u(ROW_H))
                            .pl(theme::u(18.))
                            .pr(px(SCROLLBAR_PAD))
                            .gap_1p5()
                            .items_center()
                            .text_size(theme::u(11.5))
                            .cursor_pointer()
                            .hover(|s| s.bg(c(theme::BG_HOVER)))
                            .child(
                                div()
                                    .flex_none()
                                    .w(theme::u(12.))
                                    .text_color(c(col_tok))
                                    .child(mark),
                            )
                            .child(div().flex_1().min_w_0().truncate().child(r.title.clone()))
                            .child(
                                div()
                                    .flex_shrink(1.)
                                    .min_w_0()
                                    .max_w(theme::u(130.))
                                    .truncate()
                                    .text_xs()
                                    .text_color(c(theme::FG_DIM))
                                    .child(format!("{} · {}", r.workflow, r.branch)),
                            )
                            .on_click(move |_, _, cx| cx.open_url(&url)),
                    );
                }
                col.into_any_element()
            });
            panes.push(PaneBuilt {
                spec: PaneSpec {
                    id: ScmPane::Actions,
                    flexible,
                    min_h: if flexible {
                        flex_min(natural, row_h, 2.)
                    } else {
                        header_h
                    },
                    natural_h: natural,
                    saved: s.pane_h[ScmPane::Actions as usize],
                },
                scroll_id: "scm-scroll-actions",
                header: hdr,
                rows,
            });
        }

        let specs: Vec<PaneSpec> = panes.iter().map(|p| p.spec).collect();
        let has_sash: Vec<bool> = (0..specs.len())
            .map(|i| {
                i > 0
                    && specs[..i].iter().any(|p| p.flexible)
                    && specs[i..].iter().any(|p| p.flexible)
            })
            .collect();
        let sash_count = has_sash.iter().filter(|&&b| b).count();
        let stack_h = s.stack_h.get();
        let avail_h = if stack_h > 1. {
            (stack_h - sash_count as f32 * SASH_H).max(120.)
        } else {
            600.
        };
        let heights = resolve_pane_heights(avail_h, &specs);
        let active_sash = (self.resizing == 5)
            .then(|| self.scm.pane_drag.as_ref().map(|d| d.sash))
            .flatten();
        let stack_ref = s.stack_h.clone();
        let mut stack = v_flex()
            .flex_1()
            .min_h_0()
            .w_full()
            .min_w_0()
            .relative()
            .on_prepaint(move |bounds, _, _| {
                stack_ref.set(f32::from(bounds.size.height));
            });

        let drag_ids: Vec<ScmPane> = specs.iter().map(|p| p.id).collect();
        let drag_mins: Vec<f32> = specs.iter().map(|p| p.min_h).collect();
        let drag_maxs: Vec<f32> = specs
            .iter()
            .map(|p| if p.flexible { f32::INFINITY } else { p.min_h })
            .collect();

        for (i, pane) in panes.into_iter().enumerate() {
            let id = pane.spec.id;
            if i > 0 {
                if has_sash[i] {
                    let on = active_sash == Some(i);
                    let ids = drag_ids.clone();
                    let mins = drag_mins.clone();
                    let maxs = drag_maxs.clone();
                    let fallback = heights.clone();
                    stack = stack.child(
                        div()
                            .id(("scm-sash", i))
                            .group("scm-sash")
                            .w_full()
                            .h(px(SASH_H))
                            .flex_none()
                            .flex()
                            .items_center()
                            .cursor_row_resize()
                            .child(
                                div()
                                    .w_full()
                                    .h(if on { px(2.) } else { px(1.) })
                                    .bg(if on {
                                        c(theme::ACCENT)
                                    } else {
                                        c(theme::BORDER)
                                    })
                                    .group_hover("scm-sash", |st| {
                                        st.h(px(2.)).bg(c(theme::ACCENT))
                                    }),
                            )
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |this, ev: &MouseDownEvent, _, cx| {
                                    if ev.click_count == 2 {
                                        this.scm.pane_h = [None; 6];
                                        cx.notify();
                                        return;
                                    }
                                    let painted = this.scm.painted_h.get();
                                    let start: Vec<f32> = ids
                                        .iter()
                                        .enumerate()
                                        .map(|(j, pid)| {
                                            let p = painted[*pid as usize];
                                            if p > 1. {
                                                p
                                            } else {
                                                fallback[j]
                                            }
                                        })
                                        .collect();
                                    this.scm.pane_drag = Some(ScmPaneDrag {
                                        sash: i,
                                        y0: f32::from(ev.position.y),
                                        ids: ids.clone(),
                                        start,
                                        mins: mins.clone(),
                                        maxs: maxs.clone(),
                                    });
                                    this.resizing = 5;
                                    cx.notify();
                                }),
                            ),
                    );
                } else {
                    stack = stack.child(div().w_full().h(px(1.)).flex_none().bg(c(theme::BORDER)));
                }
            }
            let painted_ref = s.painted_h.clone();
            let mut box_el = v_flex()
                .w_full()
                .min_w_0()
                .relative()
                .overflow_hidden()
                .child(pane.header)
                .on_prepaint(move |bounds, _, _| {
                    let mut a = painted_ref.get();
                    a[id as usize] = f32::from(bounds.size.height);
                    painted_ref.set(a);
                });
            if pane.spec.flexible {
                if let Some(rows_el) = pane.rows {
                    box_el = box_el.child(
                        div()
                            .flex_1()
                            .min_h_0()
                            .w_full()
                            .min_w_0()
                            .overflow_y_scrollbar()
                            .id(pane.scroll_id)
                            .scroll_bounce(false)
                            .child(rows_el),
                    );
                }
                let h = heights[i].max(1.);
                let eff_min = pane.spec.min_h.min(h);
                box_el.style().flex_grow = Some(h);
                box_el = box_el.flex_shrink_1().flex_basis(px(0.)).min_h(px(eff_min));
            } else {
                box_el = box_el.flex_none().h(px(header_h));
            }
            stack = stack.child(box_el);
        }

        let heading = section("scm-changes-view", tr("Changes").to_uppercase(), 0, s.show_changes_view)
            .border_t_1()
            .border_b_1()
            .border_color(c(theme::BORDER))
            .when(s.show_changes_view, |h| {
                h.child(
                    Button::new("scm-view-mode")
                        .label(if s.show_tree { tr("List") } else { tr("Tree") })
                        .ghost()
                        .xsmall()
                        .tooltip(if s.show_tree { tr("View as List") } else { tr("View as Tree") })
                        .on_click(cx.listener(|this, _, _, cx| {
                            cx.stop_propagation();
                            this.scm.show_tree = !this.scm.show_tree;
                            cx.notify();
                        })),
                )
            })
            .on_click(cx.listener(|this, _, _, cx| {
                this.scm.show_changes_view = !this.scm.show_changes_view;
                cx.notify();
            }));

        v_flex()
            .size_full()
            .min_h_0()
            .bg(c(theme::BG_SIDE))
            .child(heading)
            .when(
                s.show_changes_view || s.is_busy() || s.error.is_some() || s.repo_error.is_some()
                    || s.confirm_discard.is_some() || s.confirm_uninit,
                |v| v.child(top),
            )
            .child(stack)
            .into_any_element()
    }

    pub(crate) fn render_scm_header(&self, cx: &mut Context<Self>) -> impl IntoElement {
        h_flex()
            .gap_px()
            .child(tool_button(
                "scm-h-refresh",
                IconName::RefreshCw,
                "Refresh",
                cx,
                |this, _, cx| {
                    this.scm_refresh(cx);
                },
            ))
            .when(self.scm.repo.is_some(), |h| {
                h.child(
                    Button::new("scm-h-more")
                        .icon(Icon::new(IconName::Ellipsis))
                        .ghost()
                        .xsmall()
                        .disabled(self.scm.is_busy())
                        .dropdown_menu(|m: PopupMenu, _, _| {
                            m.min_w(px(230.))
                                .menu(tr("Pull"), Box::new(ScmPull))
                                .menu(tr("Push"), Box::new(ScmPush))
                                .menu(tr("Sync (Pull, then Push)"), Box::new(ScmSync))
                                .menu(tr("Fetch"), Box::new(ScmFetch))
                                .separator()
                                .menu(tr("Commit & Push"), Box::new(ScmCommitPush))
                                .menu(tr("Commit (Amend)"), Box::new(ScmCommitAmend))
                            .menu(tr("Commit (Sign Off)"), Box::new(ScmCommitSignoff))
                                .separator()
                                .menu(tr("Stage All Changes"), Box::new(ScmStageAll))
                                .menu(tr("Unstage All Changes"), Box::new(ScmUnstageAll))
                                .separator()
                                .menu(tr("Create Pull Request"), Box::new(ScmCreatePr))
                                .menu(tr("Open Repository on GitHub"), Box::new(ScmOpenGithub))
                                .separator()
                                .menu(tr("Remove Git from This Folder…"), Box::new(ScmRemoveGit))
                        }),
                )
            })
    }


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
    pub(crate) fn on_scm_commit_default(&mut self, _: &ScmCommitDefault, w: &mut Window, cx: &mut Context<Self>) {
        self.scm_commit(After::Nothing, false, w, cx);
    }
    pub(crate) fn on_scm_commit_signoff(&mut self, _: &ScmCommitSignoff, w: &mut Window, cx: &mut Context<Self>) {
        self.scm_commit_with_options(After::Nothing, false, true, w, cx);
    }
    pub(crate) fn on_scm_commit_amend(&mut self, _: &ScmCommitAmend, w: &mut Window, cx: &mut Context<Self>) {
        self.scm_commit(After::Nothing, true, w, cx);
    }
    pub(crate) fn on_scm_pull(&mut self, _: &ScmPull, _: &mut Window, cx: &mut Context<Self>) {
        self.scm_run(tr("Pulling…"), vec![Step::Remote(RemoteAction::Pull)], cx, |this, _, _| {
            this.status = tr("Pulled").into();
        });
    }
    pub(crate) fn on_scm_push(&mut self, _: &ScmPush, _: &mut Window, cx: &mut Context<Self>) {
        self.scm_run(tr("Pushing…"), vec![Step::Remote(RemoteAction::Push)], cx, |this, _, _| {
            this.status = tr("Pushed").into();
        });
    }
    pub(crate) fn on_scm_sync(&mut self, _: &ScmSync, _: &mut Window, cx: &mut Context<Self>) {
        self.scm_sync(cx);
    }
    pub(crate) fn on_scm_fetch(&mut self, _: &ScmFetch, _: &mut Window, cx: &mut Context<Self>) {
        self.scm_run(tr("Fetching…"), vec![Step::Remote(RemoteAction::Fetch)], cx, |this, _, _| {
            this.status = tr("Fetched").into();
        });
    }
    pub(crate) fn on_scm_remove_git(&mut self, _: &ScmRemoveGit, _: &mut Window, cx: &mut Context<Self>) {
        if self.scm.repo.is_some() {
            self.scm.confirm_uninit = true;
            self.view = SideView::Scm;
            self.sidebar_open = true;
            cx.notify();
        }
    }

    fn scm_remove_git(&mut self, cx: &mut Context<Self>) {
        self.scm.confirm_uninit = false;
        let Some(repo) = self.scm.repo.clone() else { return };
        let generation = self.scm.workspace_generation;
        let git = repo.join(".git");
        self.scm.busy = Some(tr("Removing Git…").into());
        cx.notify();
        cx.spawn(async move |this, cx| {
            let r = cx.background_spawn(async move { crate::core::delete_path(&git) }).await;
            _ = this.update(cx, |this, cx| {
                if this.scm.workspace_generation != generation {
                    return;
                }
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

    pub(crate) fn scm_stage_hunk(
        &mut self,
        path: String,
        group: Group,
        expected: String,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(repo) = self.scm.repo.clone() else {
            return;
        };
        if self.scm.is_busy() {
            return;
        }
        let key = Self::scm_diff_key(&repo, group, &path);
        let generation = self.scm.workspace_generation;
        let label = tr(if group == Group::Staged { "Unstaging…" } else { "Staging…" });
        self.scm.busy = Some(label.into());
        self.scm.error = None;
        self.scm.note = None;
        cx.notify();
        let wsl = self.wsl.clone();
        cx.spawn_in(window, async move |this, cx| {
            let dir = repo.clone();
            let step = Step::Hunk {
                path: path.clone(),
                group,
                expected,
                index,
            };
            let result = cx
                .background_spawn(async move {
                    run_step(&dir, &step, wsl.as_ref())?;
                    let status = crate::scm_git::read_status(&dir, wsl.as_ref())?;
                    let Some(file) = status
                        .files(group)
                        .iter()
                        .find(|f| f.path == path && f.code == 'M')
                        .cloned()
                    else {
                        return Ok::<_, String>(None);
                    };
                    let diff = crate::scm_git::file_diff(&dir, &file, group, wsl.as_ref())?;
                    Ok(Some((file, diff)))
                })
                .await;
            _ = this.update_in(cx, |this, window, cx| {
                if this.scm.workspace_generation != generation {
                    return;
                }
                this.scm.busy = None;
                this.scm_refresh(cx);
                match result {
                    Ok(Some((file, (patch, conflict_text, target)))) => {
                        let file_name = file.path.rsplit('/').next().unwrap_or(&file.path).to_string();
                        let badge = match group {
                            Group::Staged => tr("Index"),
                            Group::Conflicts => tr("Conflict"),
                            Group::Changes | Group::Untracked => tr("Working Tree"),
                        };
                        this.open_scm_diff_tab(
                            crate::diff_view::ScmDiffSpec {
                                virtual_key: key.clone(),
                                tab_title: format!("{file_name} ({badge})"),
                                display_name: file.path.clone(),
                                badge: badge.to_string(),
                                patch,
                                conflict_text,
                                target,
                                scm_stage: Some((repo, file.path, group)),
                                multi_file: false,
                            },
                            window,
                            cx,
                        );
                        if let Some(state) = this
                            .docs
                            .iter()
                            .find(|d| d.path == key && d.diff.as_ref().is_some_and(|x| x.virtual_tab))
                            .and_then(|d| d.diff.as_ref())
                            .and_then(|d| d.state.clone())
                        {
                            state.update(cx, |st, cx| {
                                let count = st.hunk_position().map(|(_, c)| c).unwrap_or(0);
                                if count > 0 {
                                    st.jump_to_hunk(index.min(count - 1), cx);
                                }
                            });
                        }
                        this.status = tr("Hunk updated.").into();
                    }
                    Ok(None) => {
                        this.close_tabs_where(window, cx, |_, doc| {
                            doc.path == key && doc.diff.as_ref().is_some_and(|d| d.virtual_tab)
                        });
                        this.status = tr("Hunk updated.").into();
                    }
                    Err(e) => this.scm.error = Some(e),
                }
                cx.notify();
            });
        })
        .detach();
    }

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
        if self.scm.repo.is_some() {
            self.scm_run(
                tr("Staging…"),
                vec![git_step(&["add", "-A"])],
                cx,
                |_, _, _| {},
            );
        }
    }
    pub(crate) fn on_scm_unstage_all(&mut self, _: &ScmUnstageAll, _: &mut Window, cx: &mut Context<Self>) {
        self.scm_paths(Group::Staged, None, false, cx);
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
    use super::{
        apply_sash_drag, change_tree_rows, find_url, lane_geometry, parse_log,
        resolve_pane_heights, scm_row, ChangeTreeRow, FileChange, PaneSpec, ScmPane, ScmPaneDrag,
        ROW_H,
    };
    use crate::theme;
    use gpui_kit::{Length, Overflow, Styled as _};

    #[test]
    fn changes_tree_groups_paths_and_hides_only_collapsed_descendants() {
        use ChangeTreeRow::{File, Folder};
        let files = ["src/lib.rs", "README.md", "src/nested/main.rs", "other/end.rs"]
            .map(|path| FileChange { path: path.into(), code: 'M', orig: None });
        assert_eq!(
            change_tree_rows(&files, &["src".to_string()].into_iter().collect()),
            vec![File(1, 0), Folder("other", 0), File(3, 1), Folder("src", 0)]
        );
        assert_eq!(
            change_tree_rows(&files, &["src/nested".to_string()].into_iter().collect()),
            vec![File(1, 0), Folder("other", 0), File(3, 1), Folder("src", 0), File(0, 1), Folder("src/nested", 1)]
        );
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

    #[test]
    fn scm_row_is_non_shrinking_and_clipped() {
        let mut row = scm_row();
        let st = row.style();
        assert_eq!(st.flex_shrink, Some(0.));
        assert_eq!(st.overflow.x, Some(Overflow::Hidden));
        assert_eq!(st.overflow.y, Some(Overflow::Hidden));
        assert_eq!(
            st.size.height,
            Some(Length::Definite(theme::u(ROW_H).into()))
        );
    }

    #[test]
    fn graph_lane_geometry_centers_dot_and_caps_ends() {
        assert_eq!(lane_geometry(0, 1, false), (12., 12., 8.));
        assert_eq!(lane_geometry(0, 3, false), (12., 0., 8.));
        assert_eq!(lane_geometry(1, 3, false), (0., 0., 8.));
        assert_eq!(lane_geometry(2, 3, false), (0., 12., 8.));
        assert_eq!(lane_geometry(2, 3, true), (0., 0., 8.));
    }

    #[test]
    fn pane_layout_splits_open_sections_and_cascades_sash_drag() {
        let specs = vec![
            PaneSpec {
                id: ScmPane::Staged,
                flexible: true,
                min_h: 72.,
                natural_h: 576.,
                saved: None,
            },
            PaneSpec {
                id: ScmPane::Changes,
                flexible: false,
                min_h: 24.,
                natural_h: 24.,
                saved: None,
            },
            PaneSpec {
                id: ScmPane::Graph,
                flexible: true,
                min_h: 100.,
                natural_h: 2000.,
                saved: None,
            },
            PaneSpec {
                id: ScmPane::Actions,
                flexible: false,
                min_h: 24.,
                natural_h: 24.,
                saved: None,
            },
        ];
        let h = resolve_pane_heights(600., &specs);
        assert!((h[0] - 276.).abs() < 0.1);
        assert!((h[1] - 24.).abs() < 0.1);
        assert!((h[2] - 276.).abs() < 0.1);
        assert!((h[3] - 24.).abs() < 0.1);

        // Dragging sash above empty Changes cascades straight between Staged and Graph.
        let drag = ScmPaneDrag {
            sash: 1,
            y0: 300.,
            ids: specs.iter().map(|s| s.id).collect(),
            start: h.clone(),
            mins: specs.iter().map(|s| s.min_h).collect(),
            maxs: specs
                .iter()
                .map(|s| if s.flexible { f32::INFINITY } else { s.min_h })
                .collect(),
        };
        let moved = apply_sash_drag(&drag, 200.);
        assert_eq!(moved.len(), 2);
        assert_eq!(moved[0].0, ScmPane::Staged);
        assert!((moved[0].1 - 176.).abs() < 0.1);
        assert_eq!(moved[1].0, ScmPane::Graph);
        assert!((moved[1].1 - 376.).abs() < 0.1);

        // Dragging further respects min_h floor on Staged.
        let clamped = apply_sash_drag(&drag, 0.);
        assert!((clamped[0].1 - 72.).abs() < 0.1);
        assert!((clamped[1].1 - 480.).abs() < 0.1);
    }

    #[test]
    fn pane_layout_shrinks_saved_heights_gracefully_on_small_window() {
        let specs = vec![
            PaneSpec {
                id: ScmPane::Staged,
                flexible: true,
                min_h: 72.,
                natural_h: 500.,
                saved: Some(380.),
            },
            PaneSpec {
                id: ScmPane::Changes,
                flexible: true,
                min_h: 72.,
                natural_h: 500.,
                saved: Some(220.),
            },
        ];
        let h = resolve_pane_heights(400., &specs);
        assert!((h[0] + h[1] - 400.).abs() < 0.1);
        assert!(h[0] >= 72. && h[1] >= 72.);
        assert!(h[0] > h[1]);
    }
}
