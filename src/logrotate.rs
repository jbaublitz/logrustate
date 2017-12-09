use std::io::{self,Write};
use std::{ptr,slice,str};
use std::error::Error;
use std::fmt::{self,Display};
use std::ffi::CString;
use std::fs::{self,File};

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
    External(usize, usize),
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

pub fn handle_log(logfile: &str, mode: LogMode) -> Result<(), LogHandleError> {
    match mode {
        LogMode::External(buf_size, file_sz_lmt) => handle_external_log(logfile, file_sz_lmt, buf_size)?,
        LogMode::Piped(buf_size) => handle_piped_log(logfile, buf_size)?,
        LogMode::Managed(buf_size) => handle_managed_log(logfile, buf_size)?,
    };
    Ok(())
}

fn handle_external_log(logfile: &str, file_sz_lmt: usize, num_lines: usize)
                       -> Result<(), LogHandleError> {
    let file_size = match fs::metadata(logfile)?.len() {
        i if i > file_sz_lmt as u64 => file_sz_lmt as u64,
        i => i,
    };
    let mmap_buf = unsafe {
        let fd = match libc::open(logfile as *const _ as *const i8, libc::O_RDONLY) {
            i if i < 0 => Err(LogHandleError::from(i)),
            i => Ok(i),
        }?;
        let mmap_ptr = libc::mmap(ptr::null_mut(), file_size as usize, libc::PROT_READ,
                                  libc::MAP_SHARED, fd, 0);
        slice::from_raw_parts(mmap_ptr as *const u8, file_size as usize)
    };
    let newline_result = count_newlines(mmap_buf, num_lines).to_result()?;
    match newline_result {
        NewlineResult::Less => (),
        NewlineResult::Head(h, hsz) => {
            strip_log_head(logfile, h, hsz)?
        }
    };
    Ok(())
}

fn strip_log_head(logfile: &str, head: String, head_size: usize) -> Result<(), LogHandleError> {
    let mut f = File::open(format!("{}.{}", logfile, time::now_utc().strftime("%s")?))?;
    f.write_all(head.as_bytes())?;
    Ok(())
}

fn handle_piped_log(logfile: &str, buf_size: usize) -> Result<(), LogHandleError> {
    Ok(())
}

fn handle_managed_log(logfile: &str, buf_size: usize) -> Result<(), LogHandleError> {
    Ok(())
}
