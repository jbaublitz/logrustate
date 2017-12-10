use std::io::{self,Write};
use std::{ptr,slice,str};
use std::error::Error;
use std::fmt::{self,Display};
use std::ffi::CString;
use std::fs::{self,File};
use std::path::Path;

use time;
use libc;
use nom;

struct ParseState {
    head: String,
    head_size: usize,
    line_count: usize,
}

enum NewlineResult {
    Less,
    Head(String, usize),
}

named_args!(count_newlines(lines: usize) <NewlineResult>, map!(fold_many1!(
    take_until_and_consume!("\n"), ParseState { head: String::new(), line_count: 0, head_size: 0 },
    |mut acc: ParseState, string: &[u8]| {
        acc.head_size += string.len();
        acc.head.push_str(String::from_utf8_lossy(string).as_ref());
        acc.line_count += 1;
        acc
    }
), |v| {
    match v {
        ParseState { head, head_size, line_count } => {
            if line_count > lines {
                NewlineResult::Head(head, head_size)
            } else {
                NewlineResult::Less
            }
        }
    }
}));

pub enum LogMode {
    External(usize),
    Piped(usize),
    Managed(usize),
}

#[derive(Debug)]
pub enum LogHandleError {
    Utf8(str::Utf8Error),
    Syscall(i32, String),
    IO(io::Error),
    Parse(nom::ErrorKind),
    Format(time::ParseError),
}

impl From<i32> for LogHandleError {
    fn from(ecode: i32) -> Self {
        let cstr = unsafe { CString::from_raw(libc::strerror(ecode)) };
        let string = cstr.into_string().unwrap_or("Error parsing errno string".to_string());
        LogHandleError::Syscall(ecode, string)
    }
}

impl From<time::ParseError> for LogHandleError {
    fn from(e: time::ParseError) -> Self {
        LogHandleError::Format(e)
    }
}

impl From<io::Error> for LogHandleError {
    fn from(e: io::Error) -> Self {
        LogHandleError::IO(e)
    }
}

impl From<str::Utf8Error> for LogHandleError {
    fn from(e: str::Utf8Error) -> Self {
        LogHandleError::Utf8(e)
    }
}

impl From<nom::ErrorKind> for LogHandleError {
    fn from(e: nom::ErrorKind) -> Self {
        LogHandleError::Parse(e)
    }
}

impl Display for LogHandleError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.description())
    }
}

impl Error for LogHandleError {
    fn description(&self) -> &str {
        match *self {
            LogHandleError::Utf8(ref e) => e.description(),
            LogHandleError::Format(ref e) => e.description(),
            LogHandleError::Syscall(_, ref string) => string.as_str(),
            LogHandleError::IO(ref e) => e.description(),
            LogHandleError::Parse(ref e) => e.description(),
        }
    }
}

pub struct OldLogState {
    old_logs: Vec<String>,
    old_log_cap: usize,
}

impl OldLogState {
    pub fn new(logfile_base: &str, num_old_logs: usize) -> Self {
        let mut state = OldLogState{ old_logs: Vec::new(),
                                     old_log_cap: num_old_logs, };
        state.populate_old_logs(logfile_base);
        state
    }

    fn populate_old_logs(&mut self, logfile_base: &str) {
        (0..self.old_log_cap).for_each(|v| {
            let path = format!("{}.{}", logfile_base, v);
            self.old_logs.push(path);
        });
    }
}

pub struct LogState<'a> {
    logfile: &'a str,
    mmap: Option<&'a [u8]>,
    old_logs: OldLogState,
    file_sz_lmt: usize,
}

impl<'a> LogState<'a> {
    pub fn new(logfile: &'a str, old_log_num: usize, file_sz_lmt: usize) -> Self {
        LogState { logfile,
                   old_logs: OldLogState::new(logfile, old_log_num),
                   mmap: None, file_sz_lmt }
    }

    pub fn handle_log(&mut self, mode: LogMode) -> Result<(), LogHandleError> {
        match mode {
            LogMode::External(buf_size) => self.handle_external_log(buf_size)?,
            LogMode::Piped(buf_size) => self.handle_piped_log(buf_size)?,
            LogMode::Managed(buf_size) => self.handle_managed_log(buf_size)?,
        };
        Ok(())
    }

    fn handle_external_log(&mut self, num_lines: usize)
                           -> Result<(), LogHandleError> {
        if let None = self.mmap {
            let file_size = match fs::metadata(self.logfile)?.len() {
                i if i > self.file_sz_lmt as u64 => self.file_sz_lmt as u64,
                i => i,
            };
            self.mmap = Some(unsafe {
                let fd = match libc::open(self.logfile as *const _ as *const i8, libc::O_RDONLY) {
                    i if i < 0 => Err(LogHandleError::from(i)),
                    i => Ok(i),
                }?;
                let mmap_ptr = libc::mmap(ptr::null_mut(), file_size as usize, libc::PROT_READ,
                                          libc::MAP_SHARED, fd, 0);
                slice::from_raw_parts(mmap_ptr as *const u8, file_size as usize)
            });
            if let Some(mmap) = self.mmap {
                let newline_result = count_newlines(mmap, num_lines).to_result()?;
                match newline_result {
                    NewlineResult::Less => (),
                    NewlineResult::Head(h, hsz) => {
                        self.logrotate(h, hsz)?
                    }
                };
            }
        }
        Ok(())
    }

    fn handle_piped_log(&mut self, buf_size: usize) -> Result<(), LogHandleError> {
        Ok(())
    }

    fn handle_managed_log(&mut self, buf_size: usize) -> Result<(), LogHandleError> {
        Ok(())
    }

    fn strip_log_head(&self, path: &str, head: &String, head_size: usize) -> Result<(), LogHandleError> {
        let mut f = File::open(path)?;
        f.write_all(head.as_bytes())?;
        Ok(())
    }

    fn logrotate(&self, head: String, head_size: usize) -> Result<(), LogHandleError> {
        let mut found_log_slot = false;
        for (i, path) in self.old_logs.old_logs.iter().enumerate() {
            let exists = Path::new(path).exists();
            if !exists {
                if i != 0 {
                    for path in self.old_logs.old_logs.iter().take(i) {

                    }
                }
                match self.strip_log_head(path, &head, head_size) {
                    Ok(()) => { found_log_slot = true; break; },
                    Err(e) => {
                        println!("Failed to strip head of file {}: {}", path, e);
                        continue;
                    },
                };
            }
        };
        if !found_log_slot {
            fs::remove_file(self.old_logs.old_logs.last().unwrap())?;
            
        }
        Ok(())
    }
}

