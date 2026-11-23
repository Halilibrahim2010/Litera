use notify::{Config, Event, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::{Path, PathBuf};
use std::sync::mpsc::Sender;
use std::time::Duration;

pub struct FsWatcher {
    watcher: Option<RecommendedWatcher>,
    current_path: Option<PathBuf>,
}

impl FsWatcher {
    pub fn new() -> Self {
        Self {
            watcher: None,
            current_path: None,
        }
    }

    pub fn watch(&mut self, path: &Path, tx: Sender<PathBuf>) -> Result<(), notify::Error> {
        let target = path.to_path_buf();
        self.current_path = Some(target.clone());

        let event_tx = tx.clone();
        let watch_path = target.clone();

        let mut watcher = RecommendedWatcher::new(
            move |res: Result<Event, notify::Error>| {
                if let Ok(event) = res {
                    use notify::EventKind;
                    match event.kind {
                        EventKind::Create(_)
                        | EventKind::Modify(_)
                        | EventKind::Remove(_) => {
                            let _ = event_tx.send(watch_path.clone());
                        }
                        _ => {}
                    }
                }
            },
            Config::default().with_poll_interval(Duration::from_millis(500)),
        )?;

        watcher.watch(&target, RecursiveMode::NonRecursive)?;
        self.watcher = Some(watcher);

        Ok(())
    }

    pub fn current_path(&self) -> Option<&Path> {
        self.current_path.as_deref()
    }
}
