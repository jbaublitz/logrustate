use std::sync::mpsc::channel;

use notify::{self,INotifyWatcher,Watcher,RecursiveMode,RawEvent};
use notify::op::{WRITE,CLOSE_WRITE};

use logrotate::{handle_log,LogMode};

pub fn watch_files(files: &[&'static str]) -> notify::Result<()> {
    let (tx, rx) = channel();

    let mut watcher = try!(INotifyWatcher::new_raw(tx));
    
    files.iter().for_each(|filename| {
        watcher.watch(filename, RecursiveMode::Recursive).unwrap_or_else(|e| {
            println!("Failed to create watcher for {}: {}", filename, e);
        });
    });

    loop {
        match rx.recv() {
            Ok(RawEvent { path: Some(path), op: Ok(op), cookie: _ }) => {
                if op == WRITE || op == CLOSE_WRITE {
                    let path_str = match path.to_str() {
                        Some(p) => p,
                        None => { continue; },
                    };
                    match handle_log(path_str, LogMode::External(100)) {
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
