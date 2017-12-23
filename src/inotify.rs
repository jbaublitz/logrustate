use std::sync::mpsc::channel;

use notify::{self,INotifyWatcher,Watcher,RecursiveMode,RawEvent};
use notify::op::{WRITE};

use logrotate::LogState;

pub fn watch_files(files: &[&'static str]) -> notify::Result<()> {
    let (tx, rx) = channel();

    let mut watcher = INotifyWatcher::new_raw(tx)?;
    
    files.iter().for_each(|filename| {
        watcher.watch(filename, RecursiveMode::Recursive).unwrap_or_else(|e| {
            println!("Failed to create watcher for {}: {}", filename, e);
        });
    });

    let mut logstate = LogState::new(5, 4096);
    loop {
        match rx.recv() {
            Ok(RawEvent { path: Some(path), op: Ok(op), cookie: _ }) => {
                if op == WRITE {
                    let path_str = match path.to_str() {
                        Some(p) => p,
                        None => { continue; },
                    };
                    match logstate.handle_external_log(path_str) {
                        Err(e) => { println!("Error handling log: {}", e) },
                        _ => (),
                    };
                }
            },
            Ok(RawEvent { path: _, op: _, cookie: _}) => { continue; },
            Err(_) => { continue; },
        }
    }
}
