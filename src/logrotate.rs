use std::io::{self,Write};
use std::{ptr,slice,str};
use std::error::Error;
use std::fmt::{self,Display};
use std::ffi::CString;
use std::fs::{self,File};
use std::path::Path;

use libc;

pub enum LogMode {
    External,
    Piped,
    Managed,
}

#[derive(Debug)]
pub enum LogHandleError {
    Utf8(str::Utf8Error),
    Syscall(i32, String),
    IO(io::Error),
}

impl From<i32> for LogHandleError {
    fn from(ecode: i32) -> Self {
        let cstr = unsafe { CString::from_raw(libc::strerror(ecode)) };
        let string = cstr.into_string().unwrap_or("Error parsing errno string".to_string());
        LogHandleError::Syscall(ecode, string)
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

impl Display for LogHandleError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.description())
    }
}

impl Error for LogHandleError {
    fn description(&self) -> &str {
        match *self {
            LogHandleError::Utf8(ref e) => e.description(),
            LogHandleError::Syscall(_, ref string) => string.as_str(),
            LogHandleError::IO(ref e) => e.description(),
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
        for v in 0..self.old_log_cap {
            let path = format!("{}.{}", logfile_base, v);
            self.old_logs.push(path);
        }
    }

    fn shift_logs(&mut self, logfile_base: &str) -> Result<String, LogHandleError> {
        let (mut files, count) = self.old_logs.iter()
                .fold((Vec::new(), 0), |(mut vec, num), val| {
            if Path::new(val).exists() {
                vec.push(val);
                (vec, num + 1)
            } else {
                (vec, num)
            }
        });
        let mut oldest_file = String::new();
        if count == self.old_log_cap {
            oldest_file = files.remove(0).to_owned();
            fs::remove_file(&oldest_file)?;
        }
        let new_files = (0..count).fold(Vec::new(), |mut vec, c| {
            vec.push(format!("{}.{}", logfile_base, c));
            vec
        });
        let from_to_iter = files.into_iter().zip(new_files.iter());
        for (from, to) in from_to_iter {
            if from != to {
                assert!(!Path::new(to).exists());
                fs::rename(from, to)?;
            }
        }
        Ok(oldest_file)
    }
}

pub struct LogState<'a> {
    logfile: &'a str,
    mmap: Option<&'a [u8]>,
    old_logs: OldLogState,
    buf_size: usize,
}

impl<'a> LogState<'a> {
    pub fn new(logfile: &'a str, old_log_num: usize, buf_size: usize) -> Self {
        LogState { logfile,
                   old_logs: OldLogState::new(logfile, old_log_num),
                   mmap: None, buf_size }
    }

    pub fn handle_log(&mut self, mode: LogMode) -> Result<(), LogHandleError> {
        match mode {
            LogMode::External => self.handle_external_log()?,
            LogMode::Piped => self.handle_piped_log()?,
            LogMode::Managed => self.handle_managed_log()?,
        };
        Ok(())
    }

    fn handle_external_log(&mut self) -> Result<(), LogHandleError> {
        let file_size = fs::metadata(self.logfile)?.len();
        if let None = self.mmap {
            self.mmap = Some(unsafe {
                let fd = match libc::open(self.logfile as *const _ as *const i8, libc::O_RDONLY) {
                    i if i < 0 => Err(LogHandleError::from(i)),
                    i => Ok(i),
                }?;
                let mmap_ptr = libc::mmap(ptr::null_mut(), self.buf_size, libc::PROT_READ,
                                          libc::MAP_SHARED, fd, 0);
                slice::from_raw_parts(mmap_ptr as *const u8, self.buf_size)
            });
            if let Some(mmap) = self.mmap {
                if file_size > self.buf_size as u64 {
                    self.logrotate(mmap)?
                }
            }
        }
        Ok(())
    }

    fn handle_piped_log(&mut self) -> Result<(), LogHandleError> {
        Ok(())
    }

    fn handle_managed_log(&mut self) -> Result<(), LogHandleError> {
        Ok(())
    }

    fn strip_log_head(&self, path: Option<&str>, head: &[u8])
                      -> Result<(), LogHandleError> {
        if let Some(p) = path {
            let mut f = File::open(p)?;
            f.write_all(head)?;
        }
        Ok(())
    }

    fn logrotate(&mut self, head: &[u8]) -> Result<(), LogHandleError> {
        self.old_logs.shift_logs(self.logfile)?;
        self.strip_log_head(self.old_logs.old_logs.first().map(|val| val.as_str()), head)?;
        Ok(())
    }
}

