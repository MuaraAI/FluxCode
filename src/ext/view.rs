//! The Extensions view (left), an extension's page (editor area), the project tasks in
//! the Run view, and running them.
//!
//! Layout, after looking at how other tools do it: a list in the sidebar with a search
//! box and Installed / This folder / All (VS Code's Extensions view, Zed's Installed
//! filter, JetBrains' Marketplace and Installed tabs); a row is an icon, the name and
//! version, one line on what it does and an Install button (VS Code, Nova, Obsidian);
//! the details open in the editor area, since the sidebar is too narrow for them (VS
//! Code's extension editor); the page lists what the extension adds and needs (Zed's
//! "provides", Raycast's command list), with the folder's own projects first.

use super::{Filter, Manifest, Project, Task, Tool};
use crate::i18n::tr;
use crate::theme::{self, c};
use crate::panel::RunInfo;
use crate::workbench::*;
use gpui_kit::assets::IconName as Ico;
use gpui_kit::component::{
    button::{Button, ButtonVariants},
    h_flex,
    input::{Input, InputState},
    menu::{DropdownMenu, PopupMenu, PopupMenuItem},
    spinner::Spinner,
    text::TextView,
    tooltip::Tooltip,
    v_flex, Icon, Sizable,
};
use gpui_kit::{prelude::FluentBuilder, *};
use std::path::Path;
use std::time::Instant;

/// A manifest's icon name → a Lucide icon.
pub fn icon(name: &str) -> Ico {
    match name {
        "pickaxe" => Ico::Pickaxe,
        "box" => Ico::Box,
        "globe" => Ico::Globe,
        "coffee" => Ico::Coffee,
        "braces" => Ico::Braces,
        "cpu" => Ico::Cpu,
        "hexagon" => Ico::Hexagon,
        "terminal" => Ico::SquareTerminal,
        "hammer" => Ico::Hammer,
        "upload" => Ico::Upload,
        "server" => Ico::Server,
        "flask-conical" => Ico::FlaskConical,
        "trash" => Ico::Trash,
        "download" => Ico::Download,
        "eye" => Ico::Eye,
        "circle-check" => Ico::CircleCheck,
        "package" => Ico::Package,
        "play" => Ico::Play,
        "sparkles" => Ico::Sparkles,
        "settings" => Ico::Settings,
        "folder-plus" => Ico::FolderPlus,
        _ => Ico::Blocks,
    }
}

/// The square tile an extension's icon sits on.
fn tile(name: &str, size: f32) -> Div {
    div()
        .flex_none()
        .size(theme::u(size))
        .rounded(px(size * 0.22))
        .bg(c(theme::BG_HOVER))
        .border_1()
        .border_color(c(theme::BORDER))
        .flex()
        .items_center()
        .justify_center()
        .child(Icon::new(icon(name)).size(theme::u(size * 0.55)).text_color(c(theme::FG_STRONG)))
}

/// A section of the extension page: a title, maybe one line under it, then its content
/// (the caller adds it).
fn page_section(title: SharedString, note: Option<SharedString>) -> Div {
    v_flex()
        .w_full()
        .gap_2()
        .child(
            v_flex()
                .gap_0p5()
                .child(div().text_sm().font_weight(FontWeight::SEMIBOLD).text_color(c(theme::FG_STRONG)).child(title))
                .when_some(note, |v, n| v.child(div().text_xs().text_color(c(theme::FG_DIM)).child(n))),
        )
}

/// Tasks few people need every day: behind More instead of in the list.
fn rare(t: &Task) -> bool {
    matches!(t.id.as_str(), "clean" | "wrapper")
}

/// Rows under a project's main button before the rest go behind More.
const SHOWN: usize = 5;

/// A project's tasks laid out: the main one (the manifest's `primary`, else the first),
/// the ones shown as rows, and the ones behind More (rare ones and any past `SHOWN`).
pub(crate) fn arrange(tasks: &[Task]) -> (Option<usize>, Vec<usize>, Vec<usize>) {
    let main = tasks.iter().position(|t| t.primary).or(if tasks.is_empty() { None } else { Some(0) });
    let (mut rows, mut more) = (Vec::new(), Vec::new());
    for (i, t) in tasks.iter().enumerate() {
        if Some(i) == main {
            continue;
        }
        if rare(t) || rows.len() >= SHOWN {
            more.push(i);
        } else {
            rows.push(i);
        }
    }
    (main, rows, more)
}

/// What a task does, for its tooltip.
fn task_tip(t: &Task) -> SharedString {
    match (t.line.is_empty(), t.terminal) {
        (true, _) => tr("Done by FluxCode itself").into(),
        (false, true) => crate::trf!("Runs in the Terminal: {line}"; line => t.line).into(),
        (false, false) => t.line.clone().into(),
    }
}

/// The main action of a group in the Run view: a full-width button, with a spinner in
/// place of its icon while it runs. `accent`: the accent fill, for the one group the
/// view leads with; the others get a quieter filled button.
pub(crate) fn run_main_button(id: ElementId, ic: Ico, label: SharedString, running: bool, accent: bool, tip: SharedString) -> Stateful<Div> {
    let (fg, icon_fg) = if accent { (theme::ON_ACCENT, theme::ON_ACCENT) } else { (theme::FG_STRONG, theme::ACCENT_TEXT) };
    h_flex()
        .id(id)
        .flex_1()
        .min_w_0()
        .h(theme::u(30.))
        .px_3()
        .gap_2()
        .justify_center()
        .rounded(px(5.))
        .border_1()
        .when(accent, |d| d.bg(c(theme::ACCENT)).border_color(c(theme::ACCENT)).hover(|d| d.opacity(0.9)).active(|d| d.opacity(0.78)))
        .when(!accent, |d| d.bg(c(theme::BG_ELEVATED)).border_color(c(theme::BORDER_STRONG)).hover(|d| d.bg(c(theme::BG_HOVER))).active(|d| d.opacity(0.85)))
        .text_color(c(fg))
        .text_sm()
        .font_weight(FontWeight::MEDIUM)
        .cursor_pointer()
        .child(if running {
            Spinner::new().icon(Ico::LoaderCircle).xsmall().color(c(icon_fg)).into_any_element()
        } else {
            Icon::new(ic).size(theme::u(14.)).text_color(c(icon_fg)).into_any_element()
        })
        .child(div().min_w_0().truncate().child(label))
        .tooltip(move |w, cx| Tooltip::new(tip.clone()).build(w, cx))
}

/// A quiet task row in the Run view; the caller adds what sits at its right end.
pub(crate) fn run_row(id: ElementId, ic: Ico, label: SharedString, running: bool, tip: SharedString) -> Stateful<Div> {
    h_flex()
        .id(id)
        .w_full()
        .min_w_0()
        .h(theme::u(26.))
        .px_2()
        .gap_2()
        .rounded(px(4.))
        .cursor_pointer()
        .text_sm()
        .text_color(c(if running { theme::FG_STRONG } else { theme::FG }))
        .hover(|d| d.bg(c(theme::BG_HOVER)))
        .child(if running {
            Spinner::new().icon(Ico::LoaderCircle).xsmall().color(c(theme::ACCENT_TEXT)).into_any_element()
        } else {
            Icon::new(ic).size(theme::u(14.)).text_color(c(theme::FG_MUTED)).into_any_element()
        })
        .child(div().flex_1().min_w_0().truncate().child(label))
        .tooltip(move |w, cx| Tooltip::new(tip.clone()).build(w, cx))
}

impl super::Ext {
    pub fn new(window: &mut Window, cx: &mut Context<Workbench>) -> Self {
        Self {
            catalog: super::catalog(),
            found: Vec::new(),
            tasks: Vec::new(),
            query: cx.new(|cx| InputState::new(window, cx).placeholder(tr("Search extensions"))),
            new_name: cx.new(|cx| InputState::new(window, cx).placeholder(tr("Name"))),
            filter: Filter::All,
            page: None,
            needs: Default::default(),
            after: None,
            last_cwd: None,
            scanning: false,
            note: None,
            running: None,
            fold_scan: 0,
            fold_count: 0,
            fold_line: None,
        }
    }
}

impl Workbench {
    pub(crate) fn ext_installed(&self, id: &str) -> bool {
        self.settings.extensions.iter().any(|e| e == id)
    }

    fn ext_manifest(&self, id: &str) -> Option<&Manifest> {
        self.ext.catalog.iter().find(|m| m.id == id)
    }

    /// Find the projects each extension is for, off the UI thread.
    pub(crate) fn ext_scan(&mut self, cx: &mut Context<Self>) {
        let roots = self.roots();
        let files: Vec<String> = self.all_files.as_ref().clone();
        let catalog = self.ext.catalog.clone();
        self.ext.scanning = true;
        cx.spawn(async move |this, cx| {
            let (found, tasks) = cx
                .background_spawn(async move {
                    let found = super::detect(&catalog, &roots, &files);
                    let tasks: Vec<Vec<Task>> = found.iter().map(|p| catalog.iter().find(|m| m.id == p.ext).map(|m| super::tasks(m, p)).unwrap_or_default()).collect();
                    (found, tasks)
                })
                .await;
            _ = this.update(cx, |this, cx| {
                this.ext.found = found;
                this.ext.tasks = tasks;
                this.ext.scanning = false;
                cx.notify();
            });
        })
        .detach();
    }

    /// Work the tasks out again (a wrapper was added, node_modules appeared).
    fn ext_retask(&mut self, cx: &mut Context<Self>) {
        let found = self.ext.found.clone();
        let catalog = self.ext.catalog.clone();
        cx.spawn(async move |this, cx| {
            let tasks = cx
                .background_spawn(async move { found.iter().map(|p| catalog.iter().find(|m| m.id == p.ext).map(|m| super::tasks(m, p)).unwrap_or_default()).collect::<Vec<_>>() })
                .await;
            _ = this.update(cx, |this, cx| {
                if this.ext.tasks.len() == tasks.len() {
                    this.ext.tasks = tasks;
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn ext_install(&mut self, id: &str, cx: &mut Context<Self>) {
        if !self.ext_installed(id) {
            self.settings.extensions.push(id.to_string());
            self.settings.save();
        }
        self.ext.note = None;
        self.ext_check_needs(id, cx);
        cx.notify();
    }

    pub(crate) fn ext_uninstall(&mut self, id: &str, cx: &mut Context<Self>) {
        self.settings.extensions.retain(|e| e != id);
        self.settings.save();
        if let Some(m) = self.ext_manifest(id).cloned() {
            if !m.builtin() {
                super::remove_file(&m);
                self.ext.catalog.retain(|x| x.id != id);
                self.ext.page = None;
            }
        }
        cx.notify();
    }

    /// Install an extension from an extension.json the user picks.
    fn ext_install_from_file(&mut self, cx: &mut Context<Self>) {
        let paths = cx.prompt_for_paths(PathPromptOptions { files: true, directories: false, multiple: false, prompt: Some(tr("Install extension").into()) });
        cx.spawn(async move |this, cx| {
            let Ok(Ok(Some(paths))) = paths.await else { return };
            let Some(p) = paths.into_iter().next() else { return };
            let res = super::install_file(&p);
            _ = this.update(cx, |this, cx| {
                match res {
                    Ok(id) => {
                        this.ext.catalog = super::catalog();
                        this.ext_install(&id, cx);
                        this.ext.page = Some(id);
                        this.ext_scan(cx);
                    }
                    Err(e) => this.ext.note = Some(crate::trf!("Couldn't install it: {e}"; e => e)),
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn ext_open_page(&mut self, id: &str, cx: &mut Context<Self>) {
        self.ext.page = Some(id.to_string());
        self.settings_open = false;
        self.whats_new = None;
        self.collab.page = false;
        self.ext_check_needs(id, cx);
        cx.notify();
    }

    /// Look the extension's tools up (PATH and the JDK folders), off the UI thread.
    fn ext_check_needs(&mut self, id: &str, cx: &mut Context<Self>) {
        let Some(m) = self.ext_manifest(id).cloned() else { return };
        let projects: Vec<Project> = self.ext.found.iter().filter(|p| p.ext == id).cloned().collect();
        let key = id.to_string();
        cx.spawn(async move |this, cx| {
            let needs = cx.background_spawn(async move { super::check_needs(&m, &projects.iter().collect::<Vec<_>>()) }).await;
            _ = this.update(cx, |this, cx| {
                this.ext.needs.insert(key, needs);
                cx.notify();
            });
        })
        .detach();
    }

    /// What the agent is told about the folder's projects (see ws_context).
    pub(crate) fn ext_agent_notes(&self) -> Option<String> {
        super::agent_notes(&self.ext.catalog, &self.settings.extensions, &self.ext.found, &self.roots())
    }

    // ------------------------------------------------------------ running tasks

    /// Save the open files of a project, like Run's "save before run".
    fn ext_save_under(&mut self, dir: &Path, cx: &mut Context<Self>) {
        for d in self.docs.iter_mut().filter(|d| d.dirty && d.path.starts_with(dir)) {
            let text = d.editor.read(cx).value();
            if crate::core::write_text(&d.path, &text, d.crlf).is_ok() {
                d.dirty = false;
            }
        }
    }

    /// JAVA_HOME and PATH for a Java task: the JDK the project asks for.
    fn ext_java_env(project: &Project) -> Option<super::java::Jdk> {
        super::java::best_jdk(Some(project))
    }

    pub(crate) fn ext_run(&mut self, pix: usize, tix: usize, window: &mut Window, cx: &mut Context<Self>) {
        let (Some(project), Some(task)) = (self.ext.found.get(pix).cloned(), self.ext.tasks.get(pix).and_then(|t| t.get(tix)).cloned()) else { return };
        self.ext_save_under(&project.dir, cx);
        if !task.native.is_empty() {
            self.ext_native(&project, &task.native, Some(window), cx);
            return;
        }
        let jdk = if task.java { Self::ext_java_env(&project) } else { None };
        if task.java && jdk.is_none() {
            if let Some(w) = super::java::wanted(&project.dir) {
                self.out_lines.clear();
                push_line(&mut self.out_lines, &crate::trf!("{name} asks for Java {w}, which isn't installed here. Install it from the extension's page (Needs), then build again."; name => project.name, w => w), LineKind::Err);
                self.panel_open = true;
                self.panel_tab = PanelTab::Output;
                cx.notify();
                return;
            }
        }
        if task.terminal {
            self.ext_run_in_terminal(&project, &task.line, jdk.as_ref(), cx);
            return;
        }
        if let Some(id) = self.out_proc.take() {
            self.procs.kill(id);
        }
        self.out_lines.clear();
        let ext_name = self.ext_manifest(&project.ext).map(|m| m.name.clone()).unwrap_or_default();
        let mut head = Line::new(&task.line, LineKind::Cmd);
        head.meta = if task.java { format!("{} · FluxCompile {}", project.name, task.label.to_lowercase()) } else { format!("{} · {}", project.name, task.label) }.into();
        self.out_lines.push(head);
        if let Some(j) = &jdk {
            push_line(&mut self.out_lines, &crate::trf!("JDK {v}: {path}"; v => j.major, path => j.home.display()), LineKind::Dim);
        }
        let mut cmd = crate::proc::shell_command(&task.line);
        crate::github::apply_env(&mut cmd);
        if let Some(j) = &jdk {
            cmd.env("JAVA_HOME", &j.home);
            let path = std::env::var_os("PATH").unwrap_or_default();
            let mut parts = vec![j.home.join("bin")];
            parts.extend(std::env::split_paths(&path));
            if let Ok(joined) = std::env::join_paths(parts) {
                cmd.env("PATH", joined);
            }
        }
        match self.procs.spawn(cmd, Some(&project.dir)) {
            Ok(id) => {
                self.out_proc = Some(id);
                self.out_run = Some(RunInfo { started: Instant::now(), block: self.out_lines.len(), python: String::new(), cwd: project.dir.clone(), command: task.line.clone() });
                self.ext.after = (!task.then.is_empty()).then(|| (project.clone(), task.then.clone()));
                self.ext.last_cwd = Some(project.dir.clone());
                self.ext.running = Some((pix, task.id.clone()));
                self.ext.fold_scan = 0;
                self.ext.fold_count = 0;
                self.ext.fold_line = None;
            }
            Err(e) => push_line(&mut self.out_lines, &crate::trf!("Couldn't start '{program}': {e}"; program => task.line, e => e), LineKind::Err),
        }
        self.status = crate::trf!("{ext}: {task}"; ext => ext_name, task => task.label).into();
        self.panel_open = true;
        self.panel_tab = PanelTab::Output;
        cx.notify();
    }

    /// A long-running task (a dev server, `cargo run`, a Minecraft server) in the
    /// Terminal: it keeps going and reads what you type there.
    fn ext_run_in_terminal(&mut self, project: &Project, line: &str, jdk: Option<&super::java::Jdk>, cx: &mut Context<Self>) {
        self.panel_open = true;
        self.panel_tab = PanelTab::Terminal;
        if self.term_proc.is_some() {
            self.status = tr("The terminal is busy: stop what runs there first (Ctrl+C)").into();
            cx.notify();
            return;
        }
        self.term_cwd = project.dir.clone();
        if !self.term_lines.is_empty() {
            push_line(&mut self.term_lines, "", LineKind::Blank);
        }
        let mut prompt = Line::new(line, LineKind::Cmd);
        prompt.meta = crate::term::short_cwd(&self.term_cwd, self.root.as_deref()).into();
        close_line(&mut self.term_lines);
        self.term_lines.push(prompt);
        let mut cmd = crate::proc::shell_command(line);
        crate::github::apply_env(&mut cmd);
        if let Some(j) = jdk {
            cmd.env("JAVA_HOME", &j.home);
            let mut parts = vec![j.home.join("bin")];
            parts.extend(std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()));
            if let Ok(joined) = std::env::join_paths(parts) {
                cmd.env("PATH", joined);
            }
        }
        match self.procs.spawn(cmd, Some(&project.dir)) {
            Ok(id) => {
                self.term_proc = Some(id);
                self.term_run = Some(RunInfo { started: Instant::now(), block: self.term_lines.len(), python: String::new(), cwd: project.dir.clone(), command: line.to_string() });
            }
            Err(e) => push_line(&mut self.term_lines, &e, LineKind::Err),
        }
        self.term_scroll.scroll_to_bottom();
        cx.notify();
    }

    /// FluxCompile: fold Gradle's `> Task ...` lines into one progress line as they come.
    pub(crate) fn ext_fold(&mut self) {
        if self.ext.running.is_none() {
            return;
        }
        let Some(start) = self.out_run.as_ref().map(|r| r.block) else { return };
        let lines = &mut self.out_lines;
        let mut i = start.max(self.ext.fold_scan);
        let mut last: Option<String> = None;
        while i < lines.len() {
            if !lines[i].open && crate::ext::compile::is_task_line(&lines[i].text) {
                last = Some(crate::ext::compile::task_name(&lines[i].text).to_string());
                lines.remove(i);
                self.ext.fold_count += 1;
                continue;
            }
            i += 1;
        }
        if let Some(t) = last {
            let text = crate::trf!("▸ {n} Gradle tasks · {task}"; n => self.ext.fold_count, task => t);
            match self.ext.fold_line {
                Some(ix) if ix < lines.len() => lines[ix].text = text.into(),
                _ => {
                    let ix = start.min(lines.len());
                    lines.insert(ix, Line::new(&text, LineKind::Dim));
                    self.ext.fold_line = Some(ix);
                }
            }
        }
        // The last line may still grow; everything above it is done.
        self.ext.fold_scan = lines.len().saturating_sub(1).max(start);
    }

    /// The Output panel's last task ended: say what FluxCompile built, then run the
    /// follow-up step if it succeeded.
    pub(crate) fn ext_after_run(&mut self, code: Option<i32>, cx: &mut Context<Self>) {
        let running = self.ext.running.take();
        if let Some(ix) = self.ext.fold_line.take() {
            if ix < self.out_lines.len() {
                self.out_lines[ix].text = crate::trf!("▸ {n} Gradle tasks"; n => self.ext.fold_count).into();
            }
        }
        let after = self.ext.after.take();
        if code == Some(0) {
            let project = running.as_ref().and_then(|(pix, _)| self.ext.found.get(*pix).cloned());
            if let Some(p) = project.filter(|p| p.ext == "paper" || p.ext == "java-gradle" || p.ext == "java-maven") {
                let took = self.out_lines.iter().rev().find(|l| l.kind == LineKind::Status).map(|l| l.meta.to_string()).unwrap_or_default();
                if let Some(s) = crate::ext::compile::summary(&p, &took) {
                    let mut l = Line::new(&s, LineKind::Success);
                    l.meta = "FluxCompile".into();
                    close_line(&mut self.out_lines);
                    self.out_lines.push(l);
                }
            }
            if let Some((p, native)) = after {
                self.ext_native(&p, &native, None, cx);
            }
        }
        self.ext_retask(cx);
    }

    fn ext_lines(&mut self, meta: &str, title: &str, res: Result<Vec<String>, String>, cx: &mut Context<Self>) {
        let mut head = Line::new(title, LineKind::Cmd);
        head.meta = meta.to_string().into();
        if !self.out_lines.is_empty() {
            push_line(&mut self.out_lines, "", LineKind::Blank);
        }
        close_line(&mut self.out_lines);
        self.out_lines.push(head);
        match res {
            Ok(lines) => {
                for l in lines {
                    push_line(&mut self.out_lines, &l, LineKind::Success);
                }
            }
            Err(e) => push_line(&mut self.out_lines, &e, LineKind::Err),
        }
        self.panel_open = true;
        self.panel_tab = PanelTab::Output;
        self.out_scroll.scroll_to_bottom();
        cx.notify();
    }

    /// Steps FluxCode does itself.
    fn ext_native(&mut self, project: &Project, native: &str, window: Option<&mut Window>, cx: &mut Context<Self>) {
        let p = project.clone();
        let roots = self.roots();
        let who = project.name.clone();
        match native {
            "paper.deploy" => self.ext_background(format!("{who} · FluxCompile"), tr("Deploy to the test server").into(), cx, move || super::compile::deploy(&p, &roots)),
            "bedrock.deploy" => self.ext_background(who, tr("Deploy to Minecraft").into(), cx, move || super::bedrock::deploy(&p)),
            "bedrock.pack" => self.ext_background(who, tr("Pack the add-on").into(), cx, move || super::bedrock::pack(&p)),
            "bedrock.validate" => self.ext_background(who, tr("Check the manifests").into(), cx, move || {
                let list = super::bedrock::packs(&p.dir);
                if list.is_empty() {
                    return Err("No pack here (a folder with manifest.json).".into());
                }
                let problems = super::bedrock::validate(&list);
                if problems.is_empty() {
                    Ok(list.iter().map(|k| format!("{}: manifest.json is fine", k.dir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default())).collect())
                } else {
                    Err(problems
                        .iter()
                        .map(|(d, m)| format!("{}\\manifest.json: {m}", d.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()))
                        .collect::<Vec<_>>()
                        .join("\n"))
                }
            }),
            "gradle.wrapper" => {
                let dir = p.dir.clone();
                self.ext_background(who, tr("Add the Gradle wrapper").into(), cx, move || {
                    super::java::add_wrapper(&dir).map(|v| vec![format!("Added the wrapper for Gradle {v} (gradlew, gradlew.bat, gradle/wrapper). The first build downloads Gradle itself once.")])
                });
            }
            "paper.server" => match super::java::find_server(&p.dir, &roots).or_else(|| super::java::server_at(&p.dir)) {
                Some(server) => {
                    let eula = std::fs::read_to_string(server.dir.join("eula.txt")).unwrap_or_default();
                    if eula.contains("eula=false") {
                        self.ext_lines(&server.jar, &tr("Start server"), Err(tr("Accept Minecraft's EULA first: set eula=true in eula.txt (it opens now).").into()), cx);
                        if let Some(w) = window {
                            self.open_file(server.dir.join("eula.txt"), w, cx);
                        }
                        return;
                    }
                    let line = super::java::server_line(&server);
                    let sp = Project { ext: p.ext.clone(), dir: server.dir.clone(), name: server.jar.clone() };
                    self.ext_run_in_terminal(&sp, &line, None, cx);
                }
                None => self.ext_lines(&who, &tr("Start server"), Err(tr("No test server found: a folder with a plugins folder and a paper-*.jar, above the plugin or in the workspace.").into()), cx),
            },
            "preview.open" => {
                if let Some(w) = window {
                    self.open_file(p.dir.join("index.html"), w, cx);
                    self.on_toggle_preview(&TogglePreview, w, cx);
                }
            }
            other => self.ext_lines(&who, other, Err(format!("Unknown step {other}")), cx),
        }
    }

    fn ext_background(&mut self, meta: String, title: SharedString, cx: &mut Context<Self>, job: impl FnOnce() -> Result<Vec<String>, String> + Send + 'static) {
        cx.spawn(async move |this, cx| {
            let res = cx.background_spawn(async move { job() }).await;
            _ = this.update(cx, |this, cx| {
                this.ext_lines(&meta, &title, res, cx);
                this.ext_retask(cx);
            });
        })
        .detach();
    }

    /// New project from a template, in the workspace folder.
    fn ext_create(&mut self, template: &str, window: &mut Window, cx: &mut Context<Self>) {
        let Some(root) = self.root.clone() else {
            self.ext.note = Some(tr("Open a folder first: the new project goes into it.").into());
            return cx.notify();
        };
        let name = self.ext.new_name.read(cx).value().to_string();
        let roots = self.roots();
        let res = match template {
            "paper" => {
                // Inside a server's plugin-dev/projects when the workspace is a server like that.
                let server = super::java::server_at(&root).or_else(|| super::java::find_server(&root.join("x"), &roots));
                let parent = [root.join("plugin-dev").join("projects"), root.join("plugin-dev")].into_iter().find(|d| d.is_dir()).unwrap_or(root.clone());
                super::java::new_plugin(&parent, &name, server.as_ref())
            }
            "bedrock" => super::bedrock::new_addon(&root, &name),
            _ => Err("unknown template".into()),
        };
        match res {
            Ok(dir) => {
                self.ext.note = Some(crate::trf!("Made {dir}"; dir => dir.display()));
                self.ext.new_name.update(cx, |s, cx| s.set_value("", window, cx));
                if template == "paper" {
                    let d = dir.clone();
                    let who = dir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                    self.ext_background(who, tr("Add the Gradle wrapper").into(), cx, move || {
                        super::java::add_wrapper(&d).map(|v| vec![format!("Added the wrapper for Gradle {v}. Build it from the Run view.")])
                    });
                }
                let open = if template == "paper" { super::java::plugin_yml(&dir).map(|y| y.path) } else { Some(dir.join("BP").join("manifest.json")) };
                if let Some(f) = open {
                    self.open_file(f, window, cx);
                }
                self.ext_refresh_files(cx);
            }
            Err(e) => self.ext.note = Some(e),
        }
        cx.notify();
    }

    // ------------------------------------------------------------ the sidebar view

    pub(crate) fn render_ext_view(&self, cx: &mut Context<Self>) -> AnyElement {
        let q = self.ext.query.read(cx).value().to_lowercase();
        let here = |m: &Manifest| self.ext.found.iter().any(|p| p.ext == m.id);
        let count = |f: Filter| {
            self.ext.catalog.iter().filter(|m| match f {
                Filter::Installed => self.ext_installed(&m.id),
                Filter::Here => here(m),
                Filter::All => true,
            })
            .count()
        };
        let tab = |id: &'static str, label: SharedString, f: Filter, cx: &mut Context<Self>| {
            let on = self.ext.filter == f;
            div()
                .id(id)
                .px_2()
                .h(theme::u(22.))
                .flex()
                .items_center()
                .rounded(px(4.))
                .text_xs()
                .cursor_pointer()
                .text_color(c(if on { theme::FG_STRONG } else { theme::FG_MUTED }))
                .when(on, |d| d.bg(c(theme::BG_HOVER)))
                .hover(|d| d.text_color(c(theme::FG_STRONG)))
                .child(label)
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.ext.filter = f;
                    cx.notify();
                }))
        };
        let tabs = h_flex()
            .px_2()
            .gap_0p5()
            .flex_wrap()
            .child(tab("ext-f-all", format!("{} {}", tr("All"), count(Filter::All)).into(), Filter::All, cx))
            .child(tab("ext-f-inst", format!("{} {}", tr("Installed"), count(Filter::Installed)).into(), Filter::Installed, cx))
            .child(tab("ext-f-here", format!("{} {}", tr("This folder"), count(Filter::Here)).into(), Filter::Here, cx));
        let mut list = v_flex().id("ext-list").w_full().flex_1().min_h_0().overflow_y_scroll().pb_2();
        let mut shown = 0;
        for (i, m) in self.ext.catalog.iter().enumerate() {
            let installed = self.ext_installed(&m.id);
            let ok = match self.ext.filter {
                Filter::Installed => installed,
                Filter::Here => here(m),
                Filter::All => true,
            };
            let hay = format!("{} {} {} {}", m.name, m.summary, m.category, m.id).to_lowercase();
            if !ok || (!q.is_empty() && !q.split_whitespace().all(|w| hay.contains(w))) {
                continue;
            }
            shown += 1;
            let used: Vec<&str> = self.ext.found.iter().filter(|p| p.ext == m.id).map(|p| p.name.as_str()).take(2).collect();
            let selected = self.ext.page.as_deref() == Some(m.id.as_str());
            let (id_open, id_install) = (m.id.clone(), m.id.clone());
            let meta: SharedString = if used.is_empty() {
                format!("{} · {}", m.publisher, m.category).into()
            } else {
                crate::trf!("Used in {names}"; names => used.join(", ")).into()
            };
            list = list.child(
                h_flex()
                    .id(("ext-row", i))
                    .flex_none()
                    .w_full()
                    .min_w_0()
                    .overflow_hidden()
                    .px_3()
                    .py_2()
                    .gap_2p5()
                    .items_start()
                    .cursor_pointer()
                    .when(selected, |d| d.bg(c(theme::BG_HOVER)))
                    .hover(|d| d.bg(c(theme::BG_HOVER)))
                    .child(tile(&m.icon, 30.))
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .gap_0p5()
                            .child(
                                h_flex()
                                    .gap_1p5()
                                    .child(div().flex_shrink(1.).min_w_0().truncate().text_sm().font_weight(FontWeight::SEMIBOLD).text_color(c(theme::FG_STRONG)).child(m.name.clone()))
                                    .child(div().flex_none().text_xs().text_color(c(theme::FG_DIM)).child(m.version.clone())),
                            )
                            .child(div().w_full().text_xs().text_color(c(theme::FG_MUTED)).line_clamp(2).text_ellipsis().child(m.summary.clone()))
                            .child(div().truncate().text_xs().text_color(c(if used.is_empty() { theme::FG_DIM } else { theme::ACCENT_TEXT })).child(meta)),
                    )
                    .when(!installed, |h| {
                        h.child(Button::new(("ext-install", i)).label(tr("Install")).xsmall().primary().on_click(cx.listener(move |this, _, _, cx| {
                            cx.stop_propagation();
                            this.ext_install(&id_install, cx);
                        })))
                    })
                    .on_click(cx.listener(move |this, _, _, cx| this.ext_open_page(&id_open, cx))),
            );
        }
        if shown == 0 {
            list = list.child(div().px_3().py_4().text_xs().text_color(c(theme::FG_DIM)).child(match self.ext.filter {
                Filter::Here if self.root.is_none() => tr("Open a folder to see which extensions it uses."),
                Filter::Here => tr("No extension is for a project in this folder."),
                Filter::Installed => tr("Nothing installed yet: pick one under All."),
                Filter::All => tr("No extension matches."),
            }));
        }
        v_flex()
            .size_full()
            .child(
                h_flex()
                    .px_2()
                    .pt_1()
                    .pb_2()
                    .gap_1()
                    .child(div().flex_1().min_w_0().child(Input::new(&self.ext.query).small().cleanable(true)))
                    .child(
                        Button::new("ext-from-file")
                            .icon(Icon::new(Ico::FolderOpen))
                            .ghost()
                            .small()
                            .tooltip(tr("Install from a file (extension.json)"))
                            .on_click(cx.listener(|this, _, _, cx| this.ext_install_from_file(cx))),
                    ),
            )
            .child(tabs)
            .when_some(self.ext.note.clone(), |v, n| v.child(div().px_3().pt_2().text_xs().text_color(c(theme::FG_MUTED)).child(n)))
            .child(div().h(px(1.)).mt_2().bg(c(theme::BORDER)))
            .child(list)
            .into_any_element()
    }

    // ------------------------------------------------------------ the page

    pub(crate) fn render_ext_page(&self, id: &str, cx: &mut Context<Self>) -> AnyElement {
        let Some(m) = self.ext_manifest(id).cloned() else { return div().into_any_element() };
        let installed = self.ext_installed(&m.id);

        // Header: icon, name, one meta line and the summary, with the actions on the
        // right. On a narrow editor area the actions drop below the text.
        let mut meta: Vec<String> = Vec::new();
        if !m.category.is_empty() {
            meta.push(m.category.clone());
        }
        meta.push(format!("v{}", m.version));
        meta.push(if m.builtin() {
            tr("Built into FluxCode").to_string()
        } else if m.publisher.is_empty() {
            tr("Installed from a file").to_string()
        } else {
            crate::trf!("By {who}"; who => m.publisher)
        });
        let mid = m.id.clone();
        let actions = h_flex()
            .flex_none()
            .gap_1()
            .child(if installed {
                Button::new("ext-uninstall").label(tr("Uninstall")).small().on_click(cx.listener(move |this, _, _, cx| this.ext_uninstall(&mid, cx))).into_any_element()
            } else {
                Button::new("ext-install-page").label(tr("Install")).small().primary().on_click(cx.listener(move |this, _, _, cx| this.ext_install(&mid, cx))).into_any_element()
            })
            .child(Button::new("ext-close").icon(Icon::new(Ico::Close)).small().ghost().tooltip(tr("Close (Esc)")).on_click(cx.listener(|this, _, _, cx| {
                this.ext.page = None;
                cx.notify();
            })));
        // The title shares its line with the actions; when the two don't fit (the title
        // would get narrower than ~160 px) the actions move under it.
        let title = h_flex()
            .w_full()
            .flex_wrap()
            .gap_x_4()
            .gap_y_1()
            .child(div().flex_1().min_w(theme::u(160.)).truncate().text_xl().font_weight(FontWeight::SEMIBOLD).text_color(c(theme::FG_STRONG)).child(m.name.clone()))
            .child(actions);
        let meta_line = h_flex()
            .w_full()
            .gap_2()
            .text_xs()
            .text_color(c(theme::FG_DIM))
            .when(installed, |h| h.child(h_flex().flex_none().gap_1().text_color(c(theme::GREEN)).child(Icon::new(Ico::Check).size(theme::u(12.))).child(tr("Installed"))).child("·"))
            .child(div().flex_1().min_w_0().truncate().child(meta.join("  ·  ")));
        let head = h_flex()
            .w_full()
            .gap_4()
            .items_start()
            .child(tile(&m.icon, 48.))
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .gap_1()
                    .child(title)
                    .child(meta_line)
                    .child(div().pt_1p5().text_sm().text_color(c(theme::FG)).child(m.summary.clone())),
            );

        let mut body = v_flex().w_full().max_w(theme::u(760.)).gap(theme::u(28.)).child(head);

        // The folder's projects, with their main task and the rest in a menu.
        let mine: Vec<usize> = self.ext.found.iter().enumerate().filter(|(_, p)| p.ext == m.id).map(|(i, _)| i).collect();
        if !mine.is_empty() {
            let note: SharedString = if installed { tr("Their tasks are in the Run view too (Ctrl+Shift+D).").into() } else { tr("Install the extension to build and run them here.").into() };
            let mut list = v_flex().rounded(px(6.)).border_1().border_color(c(theme::BORDER)).overflow_hidden();
            for (k, pix) in mine.into_iter().enumerate() {
                list = list.child(self.render_project_card(pix, installed, k > 0, cx));
            }
            body = body.child(page_section(tr("In this folder").into(), Some(note)).child(list));
        }

        // New project.
        if !m.templates.is_empty() {
            let t = m.templates[0].clone();
            let (what, how): (SharedString, SharedString) = if t == "paper" {
                (tr("New Paper plugin").into(), tr("A Gradle project for your server's Paper version, with the Gradle wrapper.").into())
            } else {
                (tr("New add-on").into(), tr("A behavior pack and its resource pack, ready to deploy.").into())
            };
            let where_: SharedString = match &self.root {
                Some(r) => {
                    let parent = if t == "paper" { [r.join("plugin-dev").join("projects"), r.join("plugin-dev")].into_iter().find(|d| d.is_dir()).unwrap_or(r.clone()) } else { r.clone() };
                    let shown = r.parent().and_then(|top| parent.strip_prefix(top).ok()).map(|p| p.display().to_string()).unwrap_or_else(|| parent.display().to_string());
                    crate::trf!("Goes into {dir}"; dir => shown).into()
                }
                None => tr("Open a folder first: the new project goes into it.").into(),
            };
            let form = v_flex()
                .gap_1p5()
                .child(
                    h_flex()
                        .gap_2()
                        .child(div().flex_1().min_w_0().max_w(theme::u(360.)).child(Input::new(&self.ext.new_name).small()))
                        .child(Button::new("ext-create").label(tr("Create")).small().on_click(cx.listener(move |this, _, w, cx| this.ext_create(&t, w, cx)))),
                )
                .child(div().w_full().truncate().text_xs().text_color(c(theme::FG_DIM)).child(where_))
                .when_some(self.ext.note.clone(), |v, n| v.child(div().text_xs().text_color(c(theme::FG_MUTED)).child(n)));
            body = body.child(page_section(what, Some(how)).child(form));
        } else if let Some(n) = &self.ext.note {
            body = body.child(div().text_xs().text_color(c(theme::FG_MUTED)).child(n.clone()));
        }

        // The tools it needs.
        if !m.needs.is_empty() {
            let checked = self.ext.needs.get(&m.id);
            let mut rows = v_flex().rounded(px(6.)).border_1().border_color(c(theme::BORDER)).overflow_hidden();
            let list: Vec<(super::Need, Option<Tool>)> = match checked {
                Some(v) => v.iter().map(|(n, t)| (n.clone(), Some(t.clone()))).collect(),
                None => m.needs.iter().map(|n| (n.clone(), None)).collect(),
            };
            for (k, (need, state)) in list.into_iter().enumerate() {
                let (mark, detail): (AnyElement, SharedString) = match &state {
                    Some(Tool::Found(p)) => (Icon::new(Ico::CircleCheck).size(theme::u(14.)).text_color(c(theme::GREEN)).into_any_element(), p.clone().into()),
                    Some(Tool::Missing) => (Icon::new(Ico::CircleX).size(theme::u(14.)).text_color(c(theme::DANGER)).into_any_element(), tr("Not found").into()),
                    None => (Spinner::new().icon(Ico::LoaderCircle).xsmall().color(c(theme::FG_DIM)).into_any_element(), tr("Checking…").into()),
                };
                let missing = matches!(state, Some(Tool::Missing));
                let (winget, url) = (need.winget.clone(), need.url.clone());
                rows = rows.child(
                    h_flex()
                        .px_3()
                        .py_2()
                        .gap_x_2p5()
                        .gap_y_1()
                        .flex_wrap()
                        .when(k > 0, |h| h.border_t_1().border_color(c(theme::BORDER)))
                        .child(div().flex_none().child(mark))
                        .child(div().flex_none().text_sm().text_color(c(theme::FG_STRONG)).child(need.label.clone()))
                        .child(div().flex_1().min_w(theme::u(120.)).truncate().text_xs().text_color(c(theme::FG_DIM)).child(detail))
                        .when(missing && cfg!(windows) && !winget.is_empty(), |h| {
                            let w = winget.clone();
                            h.child(Button::new(("ext-need-get", k)).label(tr("Install")).xsmall().tooltip(format!("winget install -e --id {winget}")).on_click(cx.listener(move |this, _, _, cx| {
                                let line = format!("winget install -e --id {w} --accept-source-agreements --accept-package-agreements");
                                this.panel_open = true;
                                this.panel_tab = PanelTab::Terminal;
                                this.run_in_terminal(&line, cx);
                            })))
                        })
                        .when(missing && !url.is_empty(), |h| {
                            h.child(Button::new(("ext-need-url", k)).label(tr("Download page")).xsmall().ghost().on_click(move |_, _, cx| cx.open_url(&url)))
                        }),
                );
            }
            body = body.child(page_section(tr("Needs").into(), None).child(rows));
        }

        // What it adds.
        let mut tasks = v_flex().rounded(px(6.)).border_1().border_color(c(theme::BORDER)).overflow_hidden();
        for (k, t) in m.tasks.iter().enumerate() {
            let flux = t.run.contains("{fluxcompile}");
            let how: SharedString = if !t.run.is_empty() {
                t.run
                    .replace(" {fluxcompile}", "")
                    .replace("{gradle}", "gradlew")
                    .replace("{mvn}", "mvn")
                    .replace("{npm}", "npm")
                    .replace("{python}", "python")
                    .into()
            } else if t.native == "npm.scripts" {
                tr("One task per script in package.json").into()
            } else {
                tr("Done by FluxCode itself").into()
            };
            // Two lines at every width: the task, and under it what it runs.
            tasks = tasks.child(
                h_flex()
                    .px_3()
                    .py_2()
                    .gap_2p5()
                    .items_start()
                    .when(k > 0, |h| h.border_t_1().border_color(c(theme::BORDER)))
                    .child(div().flex_none().pt(px(3.)).child(Icon::new(icon(&t.icon)).size(theme::u(14.)).text_color(c(theme::FG_MUTED))))
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .child(div().text_sm().text_color(c(theme::FG_STRONG)).child(t.label.clone()))
                            .child(div().text_xs().font_family("Cascadia Code").text_color(c(theme::FG_DIM)).child(how)),
                    )
                    .when(flux, |h| {
                        h.child(
                            div()
                                .id(("ext-flux", k))
                                .flex_none()
                                .px_1p5()
                                .rounded(px(4.))
                                .border_1()
                                .border_color(c(theme::BORDER))
                                .text_xs()
                                .text_color(c(theme::FG_MUTED))
                                .child("FluxCompile")
                                .tooltip(|w, cx| Tooltip::new(crate::trf!("FluxCompile runs it with {what}"; what => crate::ext::compile::ABOUT)).build(w, cx)),
                        )
                    }),
            );
        }
        body = body.child(page_section(tr("Tasks").into(), Some(tr("What it adds to the Run view.").into())).child(tasks));
        if !m.about.is_empty() {
            body = body.child(page_section(tr("About").into(), None).child(div().text_sm().text_color(c(theme::FG)).child(TextView::markdown("ext-about", m.about.clone()))));
        }
        v_flex()
            .id("ext-page")
            .key_context("ExtPage")
            .size_full()
            .items_center()
            .px(theme::u(24.))
            .pt(theme::u(28.))
            .pb(theme::u(40.))
            .overflow_y_scroll()
            .bg(c(theme::BG_EDITOR))
            .on_key_down(cx.listener(|this, e: &KeyDownEvent, _, cx| {
                if e.keystroke.key == "escape" {
                    this.ext.page = None;
                    cx.notify();
                }
            }))
            .child(body)
            .into_any_element()
    }

    /// One project on the page: its name and folder, its main task as a button and the
    /// other tasks in a menu next to it (a row of every task wrapped into a cloud).
    fn render_project_card(&self, pix: usize, installed: bool, divided: bool, cx: &mut Context<Self>) -> AnyElement {
        let p = &self.ext.found[pix];
        let rel = self.rel_display(&p.dir.to_string_lossy());
        let tasks: &[Task] = self.ext.tasks.get(pix).map(|v| v.as_slice()).unwrap_or(&[]);
        let (main, rows, more) = arrange(tasks);
        let running_id = self.ext.running.as_ref().filter(|(rp, _)| *rp == pix).map(|(_, id)| id.clone());
        let running_label = running_id.as_ref().and_then(|id| tasks.iter().find(|t| &t.id == id)).map(|t| t.label.clone());
        let line: SharedString = match &running_label {
            Some(l) => crate::trf!("Running {task}…"; task => l).into(),
            None if rel.is_empty() => tr("The folder itself").into(),
            None => rel.into(),
        };
        let mut right = h_flex().flex_none().gap_1();
        if installed {
            if let Some(mi) = main {
                let t = &tasks[mi];
                let on = running_id.as_deref() == Some(t.id.as_str());
                let b = Button::new(("ext-main", pix)).icon(Icon::new(icon(&t.icon))).label(t.label.clone()).small();
                // The accent once on the page: the first project's main task.
                let b = if divided { b } else { b.primary() };
                right = right.child(
                    b.loading(on)
                        .tooltip(task_tip(t))
                        .on_click(cx.listener(move |this, _, w, cx| this.ext_run(pix, mi, w, cx))),
                );
            }
            let others: Vec<usize> = rows.into_iter().chain(more).collect();
            if !others.is_empty() {
                right = right.child(self.tasks_menu(("ext-more", pix).into(), pix, &others, None, false, cx));
            }
        }
        h_flex()
            .px_3()
            .py_2p5()
            .gap_3()
            .when(divided, |h| h.border_t_1().border_color(c(theme::BORDER)))
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .child(div().truncate().text_sm().font_weight(FontWeight::SEMIBOLD).text_color(c(theme::FG_STRONG)).child(p.name.clone()))
                    .child(div().truncate().text_xs().text_color(c(if running_label.is_some() { theme::ACCENT_TEXT } else { theme::FG_DIM })).child(line)),
            )
            .child(right)
            .into_any_element()
    }

    /// A menu of a project's tasks (and, when `page` names the extension, its page).
    fn tasks_menu(&self, id: ElementId, pix: usize, which: &[usize], page: Option<(String, String)>, labelled: bool, cx: &mut Context<Self>) -> AnyElement {
        let tasks: &[Task] = self.ext.tasks.get(pix).map(|v| v.as_slice()).unwrap_or(&[]);
        let items: Vec<(usize, String, String)> = which.iter().filter_map(|&i| tasks.get(i).map(|t| (i, t.label.clone(), t.icon.clone()))).collect();
        let weak = cx.entity().downgrade();
        let b = Button::new(id).icon(Icon::new(Ico::Ellipsis)).ghost();
        let b = if labelled { b.label(tr("More")).xsmall().px_2().h(theme::u(26.)) } else { b.small().tooltip(tr("More tasks")) };
        b.dropdown_menu(move |mut menu: PopupMenu, _, _| {
            menu = menu.min_w(px(200.));
            for (ti, label, ic) in &items {
                let (w, ti) = (weak.clone(), *ti);
                menu = menu.item(PopupMenuItem::new(label.clone()).icon(Icon::new(icon(ic))).on_click(move |_, win, cx| {
                    _ = w.update(cx, |this, cx| this.ext_run(pix, ti, win, cx));
                }));
            }
            if let Some((ext, name)) = &page {
                let (w, ext) = (weak.clone(), ext.clone());
                menu = menu.separator().item(PopupMenuItem::new(crate::trf!("About {ext}"; ext => name)).icon(Icon::new(Ico::Info)).on_click(move |_, _, cx| {
                    _ = w.update(cx, |this, cx| this.ext_open_page(&ext, cx));
                }));
            }
            menu
        })
        .into_any_element()
    }

    // ------------------------------------------------------------ the Run view

    /// The project the Run view leads with (its main button gets the accent): the one
    /// holding the active file (the innermost), else the first. Only projects with more
    /// than one task count: a lone shortcut like Preview index.html never leads. The
    /// bool: the active file is inside it.
    pub(crate) fn ext_lead(&self) -> (Option<usize>, bool) {
        let active = self.active.and_then(|a| self.docs.get(a)).map(|d| d.path.clone());
        let ok = |i: usize, p: &Project| self.ext_installed(&p.ext) && self.ext.tasks.get(i).is_some_and(|t| t.len() > 1);
        let holding = active.and_then(|f| {
            self.ext.found.iter().enumerate().filter(|(i, p)| ok(*i, p) && f.starts_with(&p.dir)).max_by_key(|(_, p)| p.dir.components().count()).map(|(i, _)| i)
        });
        match holding {
            Some(i) => (Some(i), true),
            None => (self.ext.found.iter().enumerate().find(|(i, p)| ok(*i, p)).map(|(i, _)| i), false),
        }
    }

    /// Stop what runs in the Output panel (a task or the file), not the Terminal.
    pub(crate) fn run_stop(&self, id: impl Into<ElementId>, size: f32, cx: &mut Context<Self>) -> Stateful<Div> {
        div()
            .id(id)
            .flex_none()
            .size(theme::u(size))
            .rounded(px(5.))
            .border_1()
            .border_color(c(theme::BORDER_STRONG))
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .hover(|d| d.bg(c(theme::BG_HOVER)))
            .child(Icon::new(Ico::Square).size(theme::u(size * 0.42)).text_color(c(theme::DANGER)))
            .tooltip(|w, cx| Tooltip::new(tr("Stop")).build(w, cx))
            .on_click(cx.listener(|this, _, _, cx| {
                cx.stop_propagation();
                if let Some(id) = this.out_proc {
                    this.procs.kill(id);
                }
                cx.notify();
            }))
    }

    /// The folder's projects and their tasks, at the top of the Run view: per project
    /// a header, its main task as a full-width button, the next few as quiet rows and
    /// the rare ones (Clean, Add Gradle wrapper) behind More. Projects an extension
    /// would build but isn't installed for get one line each below.
    /// `lead`: the project whose main button gets the accent (None: the active file
    /// leads instead, see `render_run_view`).
    pub(crate) fn render_ext_run(&self, lead: Option<usize>, cx: &mut Context<Self>) -> Option<AnyElement> {
        if self.ext.found.is_empty() {
            return None;
        }
        let mut col = v_flex().w_full().gap(theme::u(20.));
        let mut any = false;
        for (pix, p) in self.ext.found.iter().enumerate() {
            if !self.ext_installed(&p.ext) {
                continue;
            }
            any = true;
            let m = self.ext_manifest(&p.ext);
            let ext_name = m.map(|m| m.name.clone()).unwrap_or_default();
            let tasks: &[Task] = self.ext.tasks.get(pix).map(|v| v.as_slice()).unwrap_or(&[]);
            let (main, rows, more) = arrange(tasks);
            let running = |t: &Task| self.ext.running.as_ref().is_some_and(|(rp, rt)| *rp == pix && *rt == t.id);

            let rel = self.rel_display(&p.dir.to_string_lossy());
            let tip: SharedString = if rel.is_empty() { format!("{} · {ext_name}", p.name) } else { format!("{} · {rel}", p.name) }.into();
            let ext_id = p.ext.clone();
            let head = h_flex()
                .id(("run-proj", pix))
                .w_full()
                .min_w_0()
                .gap_2p5()
                .cursor_pointer()
                .child(tile(m.map(|m| m.icon.as_str()).unwrap_or(""), 28.))
                .child(
                    v_flex()
                        .flex_1()
                        .min_w_0()
                        .child(div().truncate().text_sm().font_weight(FontWeight::SEMIBOLD).text_color(c(theme::FG_STRONG)).child(p.name.clone()))
                        .child(div().truncate().text_xs().text_color(c(theme::FG_DIM)).child(ext_name.clone())),
                )
                .tooltip(move |w, cx| Tooltip::new(tip.clone()).build(w, cx))
                .on_click(cx.listener(move |this, _, _, cx| this.ext_open_page(&ext_id, cx)));

            let mut group = v_flex().w_full().gap_1().child(head);
            if let Some(mi) = main {
                let t = &tasks[mi];
                let on = running(t);
                group = group.child(
                    h_flex()
                        .w_full()
                        .pt_1p5()
                        .pb_0p5()
                        .gap_1p5()
                        .child(run_main_button(("run-main", pix).into(), icon(&t.icon), t.label.clone().into(), on, lead == Some(pix), task_tip(t)).on_click(cx.listener(move |this, _, w, cx| this.ext_run(pix, mi, w, cx))))
                        .when(on, |h| h.child(self.run_stop(("run-main-stop", pix), 30., cx))),
                );
            }
            for ti in rows {
                let t = &tasks[ti];
                let on = running(t);
                let row = run_row(("run-task", pix * 100 + ti).into(), icon(&t.icon), t.label.clone().into(), on, task_tip(t))
                    .on_click(cx.listener(move |this, _, w, cx| this.ext_run(pix, ti, w, cx)))
                    .when(on, |r| r.child(self.run_stop(("run-task-stop", pix * 100 + ti), 20., cx)))
                    .when(!on && t.terminal, |r| r.child(Icon::new(Ico::SquareTerminal).size(theme::u(12.)).text_color(c(theme::FG_DIM))));
                group = group.child(row);
            }
            if !more.is_empty() {
                let running_hidden = more.iter().any(|&i| running(&tasks[i]));
                group = group.child(
                    h_flex()
                        .w_full()
                        .gap_1()
                        .child(self.tasks_menu(("run-more", pix).into(), pix, &more, Some((p.ext.clone(), ext_name.clone())), true, cx))
                        .when(running_hidden, |h| {
                            let label = more.iter().map(|&i| &tasks[i]).find(|t| running(t)).map(|t| t.label.clone()).unwrap_or_default();
                            h.child(div().flex_1().min_w_0().truncate().text_xs().text_color(c(theme::ACCENT_TEXT)).child(crate::trf!("Running {task}…"; task => label)))
                                .child(self.run_stop(("run-more-stop", pix), 20., cx))
                        }),
                );
            }
            col = col.child(group);
        }

        // Projects an extension would build, not installed yet: one calm line each.
        let mut offered: Vec<&str> = Vec::new();
        let mut offers = v_flex().w_full().gap_0p5();
        for p in &self.ext.found {
            if self.ext_installed(&p.ext) || offered.contains(&p.ext.as_str()) {
                continue;
            }
            offered.push(&p.ext);
            let Some(m) = self.ext_manifest(&p.ext) else { continue };
            let (id_a, id_b) = (m.id.clone(), m.id.clone());
            offers = offers.child(
                h_flex()
                    .id(SharedString::from(format!("run-offer-{}", m.id)))
                    .w_full()
                    .min_w_0()
                    .h(theme::u(28.))
                    .px_2()
                    .gap_2()
                    .rounded(px(4.))
                    .cursor_pointer()
                    .hover(|d| d.bg(c(theme::BG_HOVER)))
                    .child(Icon::new(icon(&m.icon)).size(theme::u(14.)).text_color(c(theme::FG_MUTED)))
                    .child(
                        h_flex()
                            .flex_1()
                            .min_w_0()
                            .gap_1p5()
                            .child(div().flex_shrink(1.).min_w_0().truncate().text_sm().text_color(c(theme::FG)).child(m.name.clone()))
                            .child(div().flex_1().min_w_0().truncate().text_xs().text_color(c(theme::FG_DIM)).child(crate::trf!("for {name}"; name => p.name))),
                    )
                    .child(Button::new(SharedString::from(format!("run-offer-i-{}", m.id))).label(tr("Install")).xsmall().on_click(cx.listener(move |this, _, _, cx| {
                        cx.stop_propagation();
                        this.ext_install(&id_a, cx);
                    })))
                    .on_click(cx.listener(move |this, _, _, cx| this.ext_open_page(&id_b, cx))),
            );
        }
        if !offered.is_empty() {
            any = true;
            col = col.child(v_flex().w_full().gap_1().child(div().px_2().text_xs().font_weight(FontWeight::MEDIUM).text_color(c(theme::FG_MUTED)).child(tr("Suggested extensions"))).child(offers));
        }
        any.then(|| col.into_any_element())
    }

    /// Re-read the folder after a new project was written (the tree, then the scan).
    fn ext_refresh_files(&mut self, cx: &mut Context<Self>) {
        self.refresh_tree(cx);
    }
}

#[cfg(test)]
mod tests {
    // Not `super::*`: the gpui prelude there has a `test` macro that shadows `#[test]`.
    use super::arrange;
    use crate::ext::Task;

    fn t(id: &str, primary: bool) -> Task {
        Task { id: id.into(), label: id.into(), icon: String::new(), line: String::new(), native: String::new(), then: String::new(), java: false, primary, terminal: false }
    }

    #[test]
    fn main_task_rows_and_more() {
        let paper = [t("build", true), t("deploy", false), t("copy", false), t("server", false), t("test", false), t("clean", false), t("wrapper", false)];
        assert_eq!(arrange(&paper), (Some(0), vec![1, 2, 3, 4], vec![5, 6]));
        // No primary: the first task leads; past SHOWN rows the rest go behind More.
        let many: Vec<Task> = (0..9).map(|i| t(&format!("s{i}"), false)).collect();
        assert_eq!(arrange(&many), (Some(0), vec![1, 2, 3, 4, 5], vec![6, 7, 8]));
        assert_eq!(arrange(&[]), (None, vec![], vec![]));
    }
}
