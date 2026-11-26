use crate::workbench::Workbench;
use futures::{channel::mpsc, select_biased, FutureExt, StreamExt};
use gpui_kit::component::notify::{self, Event, EventKind, RecursiveMode, Watcher};
use gpui_kit::{AppContext, Context, Task};
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};

#[derive(Default)]
pub(crate) struct ScmWatch {
    pub paths: Vec<PathBuf>,
    task: Option<Task<()>>,
    pub unavailable: bool,
}

fn relevant(event: &Event) -> bool {
    !matches!(event.kind, EventKind::Access(_))
        && (event.paths.is_empty()
            || event.paths.iter().any(|path| {
                let mut parts = path
                    .components()
                    .skip_while(|part| part.as_os_str() != ".git");
                parts.next();
                !parts.next().is_some_and(|part| {
                    matches!(part.as_os_str().to_str(), Some("objects" | "index.lock"))
                })
            }))
}

fn start(
    paths: &[PathBuf],
    mut sender: mpsc::Sender<()>,
) -> notify::Result<notify::RecommendedWatcher> {
    let mut watcher = notify::RecommendedWatcher::new(
        move |event: notify::Result<Event>| {
            if event.as_ref().map_or(true, relevant) {
                let _ = sender.try_send(());
            }
        },
        notify::Config::default().with_follow_symlinks(false),
    )?;
    for path in paths {
        watcher.watch(path, RecursiveMode::Recursive)?;
    }
    Ok(watcher)
}

impl Workbench {
    pub(crate) fn scm_watch(&mut self, paths: Vec<PathBuf>, cx: &mut Context<Self>) {
        if self.scm.watch.paths == paths {
            return;
        }
        self.scm.watch.paths = paths.clone();
        self.scm.watch.unavailable = false;
        let generation = self.scm.workspace_generation;
        self.scm.watch.task = Some(cx.spawn(async move |this, cx| {
            let (send, mut events) = mpsc::channel(1);
            let watcher = cx
                .background_spawn(async move { start(&paths, send) })
                .await;
            let Ok(_watcher) = watcher else {
                let _ = this.update(cx, |this, cx| {
                    if this.scm.workspace_generation == generation {
                        this.scm.watch.unavailable = true;
                        cx.notify();
                    }
                });
                return;
            };
            if this.update(cx, |this, cx| this.scm_refresh(cx)).is_err() {
                return;
            }
            while events.next().await.is_some() {
                let deadline = Instant::now() + Duration::from_secs(1);
                loop {
                    let remaining = deadline.saturating_duration_since(Instant::now());
                    let wait = cx
                        .background_executor()
                        .timer(remaining.min(Duration::from_millis(220)))
                        .fuse();
                    let next = events.next().fuse();
                    futures::pin_mut!(wait, next);
                    select_biased! {
                        _ = wait => break,
                        event = next => if event.is_none() { return; },
                    }
                }
                if this.update(cx, |this, cx| this.scm_refresh(cx)).is_err() {
                    break;
                }
            }
        }));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn watcher_ignores_git_object_traffic_but_tracks_index_and_files() {
        let event = |path: &str| Event::new(EventKind::Any).add_path(PathBuf::from(path));
        assert!(relevant(&event("src/main.rs")));
        assert!(relevant(&event(".git/index")));
        assert!(!relevant(&event(".git/objects/pack")));
        assert!(relevant(&event("objects/pack")));
    }

    #[test]
    fn watcher_reports_outside_edits() {
        let path = std::env::temp_dir().join(format!("fluxcode-watch-{}", std::process::id()));
        std::fs::create_dir(&path).unwrap();
        let (send, mut events) = mpsc::channel(1);
        let watcher = start(std::slice::from_ref(&path), send).unwrap();
        std::fs::write(path.join("external.txt"), "change").unwrap();
        let deadline = Instant::now() + Duration::from_secs(2);
        let observed = loop {
            if matches!(events.next().now_or_never(), Some(Some(()))) {
                break true;
            }
            if Instant::now() >= deadline {
                break false;
            }
            std::thread::sleep(Duration::from_millis(10));
        };
        drop(watcher);
        std::fs::remove_file(path.join("external.txt")).unwrap();
        std::fs::remove_dir(path).unwrap();
        assert!(observed);
    }
}
