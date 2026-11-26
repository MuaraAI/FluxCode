use crate::i18n::tr;
use crate::scm_git::{git_step, run_step, Step};
use crate::theme::{self, c};
use crate::workbench::{ScmCheckout, ScmFileHistory, ScmManage, Workbench};
use gpui_kit::assets::IconName;
use gpui_kit::component::{h_flex, v_flex, Icon};
use gpui_kit::{prelude::FluentBuilder, *};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static PICKER_REQUEST: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RefKind {
    Local,
    Remote,
    Tag,
}

#[derive(Clone, Debug)]
struct GitRef {
    kind: RefKind,
    name: String,
    full: String,
    detail: String,
}

const REF_FORMAT: &str = "%(refname)%00%(symref)%00%(HEAD)%00%(upstream:track)%00%(objectname:short)%00%(creatordate:relative)%00%(authorname)%00%(subject)";

fn parse_refs(output: &str) -> Vec<GitRef> {
    output
        .lines()
        .filter_map(|line| {
            let mut fields = line.splitn(8, '\0');
            let full = fields.next()?;
            if !fields.next()?.is_empty() {
                return None;
            }
            let head = fields.next()?;
            let tracking = fields.next()?;
            let oid = fields.next()?;
            let age = fields.next()?;
            let author = fields.next()?;
            let subject = fields.next()?;
            let (kind, name) = [
                (RefKind::Local, "refs/heads/"),
                (RefKind::Remote, "refs/remotes/"),
                (RefKind::Tag, "refs/tags/"),
            ]
            .into_iter()
            .find_map(|(kind, prefix)| Some((kind, full.strip_prefix(prefix)?)))?;
            let detail = [head.trim(), tracking, age, author, oid, subject]
                .into_iter()
                .filter(|field| !field.is_empty())
                .collect::<Vec<_>>()
                .join(" · ");
            Some(GitRef {
                kind,
                name: name.into(),
                full: full.into(),
                detail,
            })
        })
        .collect()
}

#[derive(Clone, Copy)]
enum Choice {
    Checkout,
    StartBranch,
    Detach,
    Merge,
    Rebase,
    DeleteBranch,
    DeleteTag,
    StashApply,
    StashPop,
    StashDrop,
    RemoveRemote,
    SetUpstream,
    AddWorktree,
    OpenWorktree,
    RemoveWorktree,
    FileHistory,
}

#[derive(Clone, Copy)]
enum Form {
    Branch,
    Tag,
    Rename,
    Stash,
    Remote,
    Commit,
    DropStash,
    Worktree,
}

#[derive(Clone)]
enum RowAction {
    Select(Choice),
    Prompt(Form, Option<String>),
    Target(Choice, GitRef),
    Run(Vec<String>),
    Open(PathBuf),
    ShowFile {
        hash: String,
        short: String,
        path: String,
    },
}

struct Row {
    label: String,
    detail: String,
    action: RowAction,
}

pub(crate) struct Picker {
    repo: PathBuf,
    generation: u64,
    request: u64,
    title: &'static str,
    rows: Vec<Row>,
    form: Option<(Form, Option<String>)>,
    pending: bool,
    error: Option<String>,
}

impl Picker {
    pub(crate) fn hits(&self, query: &str) -> Vec<usize> {
        let query = query.trim().to_lowercase();
        self.rows
            .iter()
            .enumerate()
            .filter_map(|(index, row)| {
                let label = row.label.to_lowercase();
                (query.split_whitespace().all(|part| label.contains(part))).then_some(index)
            })
            .collect()
    }
}

fn form_title(form: Form) -> &'static str {
    match form {
        Form::Branch => "Enter the new branch name",
        Form::Tag => "Enter the new tag name",
        Form::Rename => "Enter the new name for the current branch",
        Form::Stash => "Enter a stash message or press Enter to use the default",
        Form::Remote => "Enter a remote name followed by its URL",
        Form::Commit => "Enter a commit hash to cherry-pick",
        Form::DropStash => "Type drop to permanently delete this stash",
        Form::Worktree => "Enter a folder path for the new worktree",
    }
}

fn form_args(form: Form, input: &str, base: Option<&str>) -> Result<Vec<String>, &'static str> {
    let args: Vec<&str> = match form {
        Form::Branch | Form::Rename | Form::Tag if input.is_empty() => {
            return Err("Enter a name to continue.")
        }
        Form::Branch => {
            let mut args = vec!["switch", "-c", input, "--"];
            args.extend(base);
            args
        }
        Form::Tag => vec!["tag", "--", input],
        Form::Rename => vec!["branch", "-m", "--", input],
        Form::Stash if input.is_empty() => vec!["stash", "push", "--include-untracked"],
        Form::Stash => vec!["stash", "push", "--include-untracked", "-m", input],
        Form::Remote => {
            let (name, url) = input
                .split_once(char::is_whitespace)
                .filter(|(_, url)| !url.trim().is_empty())
                .ok_or("Enter both a remote name and URL.")?;
            vec!["remote", "add", "--", name, url.trim()]
        }
        Form::Commit
            if input.len() >= 7
                && input.len() <= 64
                && input.bytes().all(|b| b.is_ascii_hexdigit()) =>
        {
            vec!["cherry-pick", input]
        }
        Form::Commit => return Err("Enter a hexadecimal commit hash."),
        Form::DropStash if input.eq_ignore_ascii_case("drop") => {
            vec!["stash", "drop", base.ok_or("Select a stash first.")?]
        }
        Form::DropStash => return Err("Type drop to confirm."),
        Form::Worktree if input.is_empty() => return Err("Enter a folder path."),
        Form::Worktree => {
            let start = base.ok_or("Select a Git ref")?;
            let mut args = vec!["worktree", "add"];
            if start.starts_with("refs/") {
                args.push("--detach");
            }
            args.extend(["--", input, start]);
            args
        }
    };
    Ok(args.into_iter().map(str::to_string).collect())
}

fn checkout_args(choice: Choice, target: &GitRef) -> Vec<String> {
    let name = target.name.as_str();
    let full = target.full.as_str();
    let args = match choice {
        Choice::Checkout => match target.kind {
            RefKind::Local => vec!["switch", "--", name],
            RefKind::Remote => vec!["switch", "--track", "--", full],
            RefKind::Tag => vec!["switch", "--detach", "--", full],
        },
        Choice::Detach => vec!["switch", "--detach", "--", full],
        Choice::Merge => vec!["merge", "--no-edit", "--", full],
        Choice::Rebase => vec!["rebase", "--", full],
        Choice::DeleteBranch => vec!["branch", "-d", "--", name],
        Choice::DeleteTag => vec!["tag", "-d", "--", name],
        Choice::SetUpstream => vec!["branch", "--set-upstream-to", full],
        _ => return Vec::new(),
    };
    args.into_iter().map(str::to_string).collect()
}

fn menu_row(label: &'static str, action: RowAction) -> Row {
    Row {
        label: tr(label).into(),
        detail: String::new(),
        action,
    }
}

fn manage_rows() -> Vec<Row> {
    let select = |label, choice| menu_row(label, RowAction::Select(choice));
    let form = |label, form| menu_row(label, RowAction::Prompt(form, None));
    let run = |label, args: &[&str]| {
        menu_row(
            label,
            RowAction::Run(args.iter().map(|s| s.to_string()).collect()),
        )
    };
    vec![
        select("Checkout Branch or Tag", Choice::Checkout),
        form("Rename Current Branch", Form::Rename),
        select("Delete Merged Branch", Choice::DeleteBranch),
        select("Merge into Current Branch", Choice::Merge),
        select("Rebase Current Branch", Choice::Rebase),
        run("Abort Merge", &["merge", "--abort"]),
        run("Abort Rebase", &["rebase", "--abort"]),
        run(
            "Continue Rebase",
            &["-c", "core.editor=true", "rebase", "--continue"],
        ),
        run("Abort Cherry-pick", &["cherry-pick", "--abort"]),
        run(
            "Continue Cherry-pick",
            &["-c", "core.editor=true", "cherry-pick", "--continue"],
        ),
        form("Cherry-pick Commit", Form::Commit),
        form("Create Tag", Form::Tag),
        select("Delete Local Tag", Choice::DeleteTag),
        form("Stash Changes (Include Untracked)", Form::Stash),
        select("Apply Stash", Choice::StashApply),
        select("Pop Stash", Choice::StashPop),
        select("Drop Stash", Choice::StashDrop),
        form("Add Remote", Form::Remote),
        select("Remove Remote", Choice::RemoveRemote),
        select("Set Branch Upstream", Choice::SetUpstream),
        select("Create Worktree", Choice::AddWorktree),
        select("Open Worktree", Choice::OpenWorktree),
        select("Remove Worktree", Choice::RemoveWorktree),
    ]
}

fn load_choices(
    repo: &Path,
    wsl: Option<&crate::remote::WslTarget>,
    choice: Choice,
    file: Option<&str>,
) -> Result<Vec<Row>, String> {
    let git = |args: &[&str]| run_step(repo, &git_step(args), wsl);
    if let Some(path) = file {
        let log = git(&[
            "--literal-pathspecs",
            "log",
            "-n",
            "200",
            "--format=%H%x1f%h%x1f%s%x1f%an%x1f%ar%x1f%D%x1f%P",
            "-z",
            "--",
            path,
        ])?;
        return Ok(crate::scm_git::parse_log(&log)
            .into_iter()
            .map(|commit| Row {
                label: commit.subject,
                detail: format!("{} · {} · {}", commit.short, commit.author, commit.when),
                action: RowAction::ShowFile {
                    hash: commit.hash,
                    short: commit.short,
                    path: path.into(),
                },
            })
            .collect());
    }
    if matches!(choice, Choice::OpenWorktree | Choice::RemoveWorktree) {
        return Ok(git(&["worktree", "list", "--porcelain", "-z"])?
            .split("\0\0")
            .filter_map(|record| {
                let mut fields = record.split('\0');
                let raw = fields.next()?.strip_prefix("worktree ")?;
                let path = wsl.map_or_else(|| PathBuf::from(raw), |target| target.unc_path(raw));
                if path == repo {
                    return None;
                }
                let detail = fields
                    .find_map(|field| field.strip_prefix("branch refs/heads/"))
                    .unwrap_or("");
                let action = match choice {
                    Choice::RemoveWorktree => RowAction::Run(vec![
                        "worktree".into(),
                        "remove".into(),
                        "--".into(),
                        raw.into(),
                    ]),
                    _ => RowAction::Open(path.clone()),
                };
                Some(Row {
                    label: path.to_string_lossy().into_owned(),
                    detail: detail.into(),
                    action,
                })
            })
            .collect());
    }
    if matches!(
        choice,
        Choice::StashApply | Choice::StashPop | Choice::StashDrop
    ) {
        return Ok(git(&["stash", "list", "--format=%gd%x00%gs"])?
            .lines()
            .filter_map(|row| {
                let (id, detail) = row.split_once('\0')?;
                let action = match choice {
                    Choice::StashDrop => RowAction::Prompt(Form::DropStash, Some(id.into())),
                    Choice::StashPop => {
                        RowAction::Run(vec!["stash".into(), "pop".into(), id.into()])
                    }
                    _ => RowAction::Run(vec!["stash".into(), "apply".into(), id.into()]),
                };
                Some(Row {
                    label: id.into(),
                    detail: detail.into(),
                    action,
                })
            })
            .collect());
    }
    if matches!(choice, Choice::RemoveRemote) {
        return Ok(git(&["remote"])?
            .lines()
            .map(|name| Row {
                label: name.into(),
                detail: String::new(),
                action: RowAction::Run(vec![
                    "remote".into(),
                    "remove".into(),
                    "--".into(),
                    name.into(),
                ]),
            })
            .collect());
    }
    let format = format!("--format={REF_FORMAT}");
    let refs = git(&[
        "for-each-ref",
        "--sort=-creatordate",
        &format,
        "refs/heads",
        "refs/remotes",
        "refs/tags",
    ])?;
    let mut rows = Vec::new();
    if matches!(choice, Choice::Checkout) {
        rows.extend([
            menu_row("Create new branch…", RowAction::Prompt(Form::Branch, None)),
            menu_row(
                "Create new branch from…",
                RowAction::Select(Choice::StartBranch),
            ),
            menu_row("Checkout detached…", RowAction::Select(Choice::Detach)),
        ]);
    }
    let mut refs = parse_refs(&refs);
    refs.sort_by_key(|entry| match entry.kind {
        RefKind::Local => 0,
        RefKind::Remote => 1,
        RefKind::Tag => 2,
    });
    rows.extend(
        refs.into_iter()
            .filter(|entry| match choice {
                Choice::DeleteBranch => entry.kind == RefKind::Local,
                Choice::DeleteTag => entry.kind == RefKind::Tag,
                Choice::SetUpstream => entry.kind == RefKind::Remote,
                _ => true,
            })
            .map(|entry| Row {
                label: entry.name.clone(),
                detail: entry.detail.clone(),
                action: RowAction::Target(choice, entry),
            }),
    );
    Ok(rows)
}

impl Workbench {
    pub(crate) fn on_scm_checkout(
        &mut self,
        _: &ScmCheckout,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.scm_pick(Some(Choice::Checkout), None, w, cx);
    }

    pub(crate) fn on_scm_manage(&mut self, _: &ScmManage, w: &mut Window, cx: &mut Context<Self>) {
        self.scm_pick(None, None, w, cx);
    }

    pub(crate) fn on_scm_file_history(
        &mut self,
        _: &ScmFileHistory,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let path = self
            .active
            .and_then(|index| self.docs.get(index))
            .and_then(|doc| {
                doc.diff
                    .as_ref()
                    .and_then(|diff| diff.target.as_deref())
                    .or(Some(&doc.path))
            })
            .and_then(|path| path.strip_prefix(self.scm.repo.as_ref()?).ok())
            .map(|path| path.to_string_lossy().replace('\\', "/"));
        let Some(path) = path.filter(|p| !p.is_empty()) else {
            self.status = tr("Open a file in this Git repository first.").into();
            cx.notify();
            return;
        };
        self.scm_pick(Some(Choice::FileHistory), Some(path), w, cx);
    }

    fn scm_show_picker(&mut self, picker: Picker, w: &mut Window, cx: &mut Context<Self>) {
        let title = tr(picker.title);
        self.quick.git_picker = Some(picker);
        self.quick.open = true;
        self.quick.input.update(cx, |input, cx| {
            input.set_value("", w, cx);
            input.set_placeholder(title, w, cx);
        });
        self.quick.input.focus_handle(cx).focus(w, cx);
        self.quick_filter(cx);
    }

    fn scm_pick(
        &mut self,
        choice: Option<Choice>,
        file: Option<String>,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(repo) = self.scm.repo.clone() else {
            self.status = tr("Open a Git repository first.").into();
            cx.notify();
            return;
        };
        let generation = self.scm.workspace_generation;
        let request = PICKER_REQUEST.fetch_add(1, Ordering::Relaxed);
        let title = match choice {
            None => "Choose a Git operation",
            Some(Choice::Checkout) => "Select a branch or tag to checkout",
            Some(Choice::StartBranch) => "Select the starting branch or tag",
            Some(Choice::Detach) => "Select a ref for detached checkout",
            Some(Choice::StashApply | Choice::StashPop | Choice::StashDrop) => "Select a stash",
            Some(Choice::RemoveRemote) => "Select a remote to remove",
            Some(Choice::OpenWorktree | Choice::RemoveWorktree) => "Select a worktree",
            Some(Choice::FileHistory) => "Select a commit to view the file diff",
            _ => "Select a Git ref",
        };
        self.scm_show_picker(
            Picker {
                repo: repo.clone(),
                generation,
                request,
                title,
                rows: if choice.is_none() {
                    manage_rows()
                } else {
                    Vec::new()
                },
                form: None,
                pending: choice.is_some(),
                error: None,
            },
            w,
            cx,
        );
        let Some(choice) = choice else { return };
        let wsl = self.wsl.clone();
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    load_choices(&repo, wsl.as_ref(), choice, file.as_deref())
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                if this.scm.workspace_generation != generation {
                    return;
                }
                if let Some(picker) = &mut this.quick.git_picker {
                    if picker.request == request && picker.pending {
                        picker.pending = false;
                        match result {
                            Ok(rows) => picker.rows = rows,
                            Err(error) => picker.error = Some(error),
                        }
                        this.quick_filter(cx);
                    }
                }
            });
        })
        .detach();
    }

    pub(crate) fn scm_accept_picker(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        let Some(picker) = &self.quick.git_picker else {
            return;
        };
        if self.scm.repo.as_ref() != Some(&picker.repo)
            || self.scm.workspace_generation != picker.generation
        {
            self.quick_close(w, cx);
            return;
        }
        let action = if let Some((form, base)) = &picker.form {
            let input = self.quick.input.read(cx).value().to_string();
            match form_args(*form, input.trim(), base.as_deref()) {
                Ok(args) => RowAction::Run(args),
                Err(message) => {
                    if let Some(picker) = &mut self.quick.git_picker {
                        picker.error = Some(tr(message).into());
                    }
                    cx.notify();
                    return;
                }
            }
        } else {
            let Some(row) = self
                .quick
                .hits
                .get(self.quick.sel)
                .and_then(|i| picker.rows.get(*i))
            else {
                return;
            };
            row.action.clone()
        };
        match action {
            RowAction::Select(choice) => self.scm_pick(Some(choice), None, w, cx),
            RowAction::Prompt(form, base) => self.scm_prompt(form, base, w, cx),
            RowAction::Target(Choice::StartBranch, target) => {
                self.scm_prompt(Form::Branch, Some(target.full), w, cx)
            }
            RowAction::Target(Choice::AddWorktree, target) => {
                let start = if target.kind == RefKind::Local {
                    target.name
                } else {
                    target.full
                };
                self.scm_prompt(Form::Worktree, Some(start), w, cx)
            }
            RowAction::Target(choice, target) => {
                self.scm_run_picker(checkout_args(choice, &target), w, cx)
            }
            RowAction::Run(args) => self.scm_run_picker(args, w, cx),
            RowAction::Open(path) => {
                self.quick_close(w, cx);
                self.open_folder(path, cx);
            }
            RowAction::ShowFile { hash, short, path } => {
                self.quick_close(w, cx);
                self.scm_show_commit_file(hash, short, path, w, cx);
            }
        }
    }

    fn scm_prompt(
        &mut self,
        form: Form,
        base: Option<String>,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(mut picker) = self.quick.git_picker.take() else {
            return;
        };
        picker.title = form_title(form);
        picker.rows.clear();
        picker.error = None;
        picker.pending = false;
        picker.form = Some((form, base));
        self.scm_show_picker(picker, w, cx);
    }

    fn scm_run_picker(&mut self, args: Vec<String>, w: &mut Window, cx: &mut Context<Self>) {
        let Some(picker) = &self.quick.git_picker else {
            return;
        };
        let changes_files = match args.first().map(String::as_str) {
            Some("stash") => args.get(1).is_some_and(|arg| arg != "drop"),
            Some("switch" | "merge" | "rebase" | "cherry-pick" | "-c") => true,
            _ => false,
        };
        if changes_files
            && self
                .docs
                .iter()
                .any(|doc| doc.dirty && doc.path.starts_with(&picker.repo))
        {
            if let Some(picker) = &mut self.quick.git_picker {
                picker.error =
                    Some(tr("Save unsaved editors before changing Git branches or files.").into());
            }
            cx.notify();
            return;
        }
        self.quick_close(w, cx);
        self.view = crate::workbench::SideView::Scm;
        self.sidebar_open = true;
        let step = if changes_files {
            Step::WorkspaceGit(args)
        } else {
            Step::Git(args, None)
        };
        self.scm_run(tr("Running Git…"), vec![step], cx, |this, _, _| {
            this.status = tr("Git operation completed.").into();
        });
    }

    pub(crate) fn scm_picker_rows(
        &self,
        picker: &Picker,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let mut rows = Vec::new();
        if let Some(message) = &picker.error {
            rows.push(
                div()
                    .px_3()
                    .py_1()
                    .text_sm()
                    .text_color(c(theme::DANGER))
                    .child(message.clone())
                    .into_any_element(),
            );
        } else if picker.pending || picker.form.is_none() && self.quick.hits.is_empty() {
            let message = if picker.pending {
                tr("Loading Git refs…")
            } else {
                tr("No matching items.")
            };
            rows.push(
                div()
                    .px_3()
                    .py_1()
                    .text_sm()
                    .text_color(c(theme::FG_MUTED))
                    .child(message)
                    .into_any_element(),
            );
        }
        for (row, &i) in self.quick.hits.iter().enumerate() {
            let entry = &picker.rows[i];
            let icon = match &entry.action {
                RowAction::Target(_, entry) => match entry.kind {
                    RefKind::Tag => IconName::Tag,
                    _ => IconName::GitBranch,
                },
                RowAction::Prompt(Form::Branch, _) | RowAction::Select(Choice::StartBranch) => {
                    IconName::Plus
                }
                _ => IconName::ChevronRight,
            };
            rows.push(
                h_flex()
                    .id(("quick-git-row", row))
                    .mx_1()
                    .px_2()
                    .py_1()
                    .gap_2()
                    .min_w_0()
                    .cursor_pointer()
                    .when(row == self.quick.sel, |h| h.bg(c(theme::SELECT)))
                    .hover(|h| h.bg(c(theme::BG_HOVER)))
                    .child(Icon::new(icon).size(theme::u(14.)).flex_none())
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .child(div().truncate().text_sm().child(entry.label.clone()))
                            .when(!entry.detail.is_empty(), |v| {
                                v.child(
                                    div()
                                        .truncate()
                                        .text_xs()
                                        .text_color(c(theme::FG_MUTED))
                                        .child(entry.detail.clone()),
                                )
                            }),
                    )
                    .on_click(cx.listener(move |this, _, w, cx| {
                        this.quick.sel = row;
                        this.scm_accept_picker(w, cx);
                    }))
                    .into_any_element(),
            );
        }
        rows
    }
}

#[cfg(test)]
mod tests {
    use super::{checkout_args, form_args, parse_refs, Choice, Form, RefKind};

    #[test]
    fn parse_refs_keeps_local_remote_and_tags_distinct() {
        let line = |name, symbolic| {
            [
                name,
                symbolic,
                "*",
                "<>",
                "aa00123",
                "1 day ago",
                "Ada",
                "Fix picker",
            ]
            .join("\0")
        };
        let refs = parse_refs(
            &[
                line("refs/heads/main", ""),
                line("refs/remotes/origin/HEAD", "refs/remotes/origin/main"),
                line("refs/remotes/origin/main", ""),
                line("refs/tags/main", ""),
            ]
            .join("\n"),
        );
        assert_eq!(
            refs.iter().map(|r| r.kind).collect::<Vec<_>>(),
            [RefKind::Local, RefKind::Remote, RefKind::Tag]
        );
        assert_eq!(
            checkout_args(Choice::Checkout, &refs[0]),
            ["switch", "--", "main"]
        );
        assert_eq!(
            checkout_args(Choice::Checkout, &refs[1]),
            ["switch", "--track", "--", "refs/remotes/origin/main"]
        );
        assert_eq!(
            checkout_args(Choice::Checkout, &refs[2]),
            ["switch", "--detach", "--", "refs/tags/main"]
        );
    }

    #[test]
    fn picker_forms_keep_names_and_urls_as_arguments() {
        assert!(form_args(Form::Remote, "origin", None).is_err());
        assert!(form_args(Form::Commit, "--abort", None).is_err());
        assert!(form_args(Form::DropStash, "", Some("stash@{0}")).is_err());
        assert_eq!(
            form_args(Form::Branch, "test/foo", Some("refs/tags/start")).unwrap(),
            ["switch", "-c", "test/foo", "--", "refs/tags/start"]
        );
        assert_eq!(
            form_args(Form::Stash, "work in progress", None)
                .unwrap()
                .last()
                .unwrap(),
            "work in progress"
        );
    }
}
