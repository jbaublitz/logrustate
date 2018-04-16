//! # `logrustate`
//! Event driven logrotate implementation in Rust

#![deny(missing_docs)]

extern crate getopts;
extern crate libc;
extern crate nix;
extern crate notify;

mod inotify;
mod logrotate;

use std::env;
use std::process;

use getopts::{Fail,Options};

use inotify::watch_files;

struct LogrustateArgs {
    files: Vec<String>,
    num_old_logs: usize,
    log_size: usize,
}

fn parse_args() -> Result<LogrustateArgs, Fail> {
    let args = env::args();

    let mut opts = Options::new();
    opts.optmulti("f", "file", "Log file to watch", "FILE")
        .optopt("n", "num-old-logs", "Number of old logs to preserve", "NUM")
        .optopt("s", "log-size", "Size of preserved logs", "SIZE")
        .optflag("h", "help", "Help text");
    let matches = opts.parse(args)?;

    if matches.opt_present("h") {
        println!("{}", opts.usage("USAGE: logrustate [-f FILE1 -f FILE 2...] [-h]"));
        process::exit(0);
    }
    let num_old_logs = if let Some(nlogs) = matches.opt_str("n") {
        nlogs.parse::<usize>().unwrap_or_else(|_| {
            println!("Failed to parse -n argument - defaulting to 5");
            5
        })
    } else {
        println!("Defaulting to 5 old logs");
        5
    };
    let log_size = if let Some(logsize) = matches.opt_str("s") {
        let mut lsize = logsize.parse::<usize>().unwrap_or_else(|_| {
            println!("Failed to parse -s argument - defaulting to 4096");
            4096
        });
        if lsize % 4096 != 0 {
            println!("Log size must be a multiple of 4096 - \
                     increasing to the next multiple of 4096");
            lsize -= lsize % 4096;
            lsize += 4096;
        }
        lsize
    } else {
        println!("Defaulting to 4096");
        4096
    };
    let files = matches.opt_strs("f");
    Ok(LogrustateArgs { files, num_old_logs, log_size })
}

/// Main function
pub fn main() {
    let args = match parse_args() {
        Ok(args) => args,
        Err(e) => {
            println!("{}", e);
            process::exit(1);
        }
    };

    match watch_files(&args.files, args.num_old_logs, args.log_size) {
        Err(e) => {
            println!("Error watching files: {}", e);
            process::exit(1);
        },
        _ => (),
    }
}
