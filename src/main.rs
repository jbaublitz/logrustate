extern crate notify;
extern crate libc;

mod inotify;
mod logrotate;

use std::process;

use inotify::watch_files;

pub fn main() {
    match watch_files(&["test.log"]) {
        Err(e) => {
            println!("Error watching files: {}", e);
            process::exit(1);
        },
        _ => (),
    }
}