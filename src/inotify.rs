use std::sync::mpsc::channel;

use notify::{self,INotifyWatcher,Watcher,RecursiveMode,RawEvent};
use notify::op::{WRITE};

use logrotate::LogState;

pub fn watch_files(files: &[String], num_logs: usize, log_size: usize) -> notify::Result<()> {
    let (tx, rx) = channel();

    let mut watcher = INotifyWatcher::new_raw(tx)?;
    
    let mut failures = 0;
    files.iter().for_each(|filename| {
        watcher.watch(filename, RecursiveMode::Recursive).unwrap_or_else(|e| {
            println!("Failed to create watcher for {}: {}", filename, e);
            failures += 1;
        });
    });
    if failures == files.len() {
        return Err(notify::Error::PathNotFound);
    }

    let mut logstate = LogState::new(num_logs, log_size);
    loop {
        match rx.recv() {
            Ok(RawEvent { path: Some(path), op: Ok(op), cookie: _ }) => {
                if op == WRITE {
                    let path_str = match path.to_str() {
                        Some(p) => p,
                        None => { continue; },
                    };
                    match logstate.handle_log(path_str) {
                        Err(e) => { println!("Error handling log: {}", e) },
                        _ => (),
                    };
                }
            },
            _ => continue,
        }
    }
}
