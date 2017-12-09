use std::io;
use std::{ptr,slice,str};
use std::error::Error;
use std::fmt::{self,Display};
use std::ffi::CString;

use libc;

pub enum LogMode {
    External(usize),
    Piped(usize),
    Managed(usize),
}

#[derive(Debug)]
pub enum LogHandleError {
    Utf8(str::Utf8Error),
    Syscall(i32, String),
}

impl LogHandleError {
    pub fn new_syscall(ecode: i32) -> Self {
        let cstr = unsafe { CString::from_raw(libc::strerror(ecode)) };
        let string = cstr.into_string().unwrap_or("Error parsing errno string".to_string());
        LogHandleError::Syscall(ecode, string)
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
            LogHandleError::Syscall(i, ref string) => string.as_str(),
        }
    }
}

pub fn handle_log(logfile: &str, mode: LogMode) -> Result<(), LogHandleError> {
    match mode {
        LogMode::External(buf_size) => try!(handle_external_log(logfile, buf_size)),
        LogMode::Piped(buf_size) => try!(handle_piped_log(logfile, buf_size)),
        LogMode::Managed(buf_size) => try!(handle_managed_log(logfile, buf_size)),
    };
    Ok(())
}

fn handle_external_log(logfile: &str, buf_size: usize) -> Result<(), LogHandleError> {
    let mmap_buf = unsafe {
        let fd = try!(match libc::open(logfile as *const _ as *const i8, libc::O_RDONLY) {
            i if i < 0 => Err(LogHandleError::new_syscall(i)),
            i => Ok(i),
        });
        let mmap_ptr = libc::mmap(ptr::null_mut(), buf_size, libc::PROT_READ,
                                  libc::MAP_SHARED, fd, 0);
        slice::from_raw_parts(mmap_ptr as *const u8, buf_size)
    };
    let mmap_str = try!(str::from_utf8(mmap_buf));
    let newlines = mmap_str.split("\n").count();
    Ok(())
}

fn handle_piped_log(logfile: &str, buf_size: usize) -> Result<(), LogHandleError> {
    Ok(())
}

fn handle_managed_log(logfile: &str, buf_size: usize) -> Result<(), LogHandleError> {
    Ok(())
}
