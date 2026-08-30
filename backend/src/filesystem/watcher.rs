use crate::events::{DomainEvent, EventJournal};
use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub struct FileSystemWatcher {
    _watcher: RecommendedWatcher,
    root_path: PathBuf,
    connection_id: String,
}

impl FileSystemWatcher {
    pub fn new<P: AsRef<Path>>(
        connection_id: String,
        root_path: P,
        event_journal: Arc<EventJournal>,
    ) -> anyhow::Result<Self> {
        let root = root_path.as_ref().to_path_buf();
        let root_clone = root.clone();
        let conn_id = connection_id.clone();

        let mut watcher = RecommendedWatcher::new(
            move |res: Result<Event, notify::Error>| {
                if let Ok(event) = res {
                    let action = match event.kind {
                        EventKind::Create(_) => "create",
                        EventKind::Modify(_) => "modify",
                        EventKind::Remove(_) => "delete",
                        _ => return,
                    };

                    for path in event.paths {
                        // Compute relative VFS path
                        let relative_str = if let Ok(rel) = path.strip_prefix(&root_clone) {
                            format!("/{}", rel.to_string_lossy().replace('\\', "/"))
                        } else {
                            format!("/{}", path.to_string_lossy().replace('\\', "/"))
                        };

                        let ws_event = DomainEvent::file_change(&conn_id, &relative_str, action);
                        let journal = event_journal.clone();
                        tokio::spawn(async move {
                            let _ = journal.append(ws_event, None).await;
                        });
                    }
                }
            },
            Config::default(),
        )?;

        watcher.watch(&root, RecursiveMode::Recursive)?;

        Ok(Self {
            _watcher: watcher,
            root_path: root,
            connection_id,
        })
    }

    pub fn root_path(&self) -> &Path {
        &self.root_path
    }

    pub fn connection_id(&self) -> &str {
        &self.connection_id
    }
}
