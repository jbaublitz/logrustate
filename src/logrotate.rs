use std::io::{self,Write};
use std::{ptr,slice,str};
use std::error::Error;
use std::fmt::{self,Display};
use std::ffi::CString;
use std::fs::{self,File};
use std::path::Path;
use std::collections::HashMap;

use libc;
use nix;

#[derive(Debug)]
pub enum LogHandleError {
    Utf8(str::Utf8Error),
    Syscall(i32, String),
    IO(io::Error),
}

impl LogHandleError {
    fn from_errno() -> Self {
        let ecode = nix::errno::errno();
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
    num_old_logs: usize, 
}

impl OldLogState {
    pub fn new(num_old_logs: usize) -> Self {
        OldLogState{ num_old_logs }
    }

    fn shift_logs(&mut self, logfile_base: &str, file_size: u64, buf_size: usize)
                  -> Result<(u64, usize), LogHandleError> {
        let mut chunks = file_size as usize / buf_size;
        let mut num_discarded = 0;
        if chunks > self.num_old_logs {
            println!("Some logs will be discarded - consider upping either \
                     buffer size or number of old logs");
            num_discarded = chunks - self.num_old_logs;
            chunks = self.num_old_logs;
        }
        let existing_files = (0..self.num_old_logs).fold(Vec::new(), |mut acc, item| {
            let path = format!("{}.{}", logfile_base, item);
            if Path::new(&path).exists() {
                acc.push(path);
            }
            acc
        });
        println!("Found the following old logs: {:?}", existing_files);
        let num_files_rm = if existing_files.len() == self.num_old_logs {
            chunks
        } else {
            0
        };
        let mut existing_files_enum = existing_files.iter().enumerate().rev();
        for (_, file) in existing_files_enum.by_ref().take(num_files_rm) {
            println!("Removing the following file: {}", file);
            fs::remove_file(file)?
        }
        for (i, file) in existing_files_enum {
            let new_file = format!("{}.{}", logfile_base, i + 1);
            println!("Renaming file {} to {}", file, new_file);
            fs::rename(file, new_file)?
        }
        if num_discarded > 0 {
            Ok((self.num_old_logs as u64, num_discarded))
        } else {
            Ok((num_files_rm as u64, 0))
        }
    }
}

pub struct LogState<'a> {
    mmaps: HashMap<String, (u32, &'a [u8])>,
    old_logs: OldLogState,
    buf_size: usize,
}

impl<'a> LogState<'a> {
    pub fn new(old_log_num: usize, buf_size: usize) -> Self {
        LogState { old_logs: OldLogState::new(old_log_num),
                   mmaps: HashMap::new(), buf_size }
    }

    pub fn handle_log(&mut self, logfile: &str) -> Result<(), LogHandleError> {
        let mmap_open = self.mmaps.contains_key(logfile);
        if !mmap_open {
            unsafe {
                match libc::open(logfile as *const _ as *const i8, libc::O_RDWR) {
                    i if i < 0 => Err(LogHandleError::from_errno()),
                    i => {
                        let mmap_ptr = libc::mmap(ptr::null_mut(), self.buf_size, libc::PROT_READ,
                                                  libc::MAP_SHARED, i, 0);
                        self.mmaps.insert(logfile.to_owned(),
                                          (i as u32,
                                           slice::from_raw_parts(
                                               mmap_ptr as *const u8, self.buf_size)
                                           ));
                        Ok(())
                    },
                }?;
            };
        }
        let current_size = fs::metadata(logfile)?.len();
        if current_size > self.buf_size as u64 {
            self.logrotate(logfile, current_size)?
        }
        Ok(())
    }

    fn strip_log_head(&self, logfile_base: &str, end_num: u64, num_drop: usize)
                      -> Result<(), LogHandleError> {
        if let Some(&(fd, mmap)) = self.mmaps.get(logfile_base) {
            if num_drop > 0 {
                if unsafe { libc::fallocate(fd as libc::c_int, libc::FALLOC_FL_COLLAPSE_RANGE,
                                            0, (self.buf_size * num_drop) as libc::c_long) } < 0 {
                    return Err(LogHandleError::from_errno());
                }
            }
            for file_num in 0..end_num {
                let mut f = File::create(&format!("{}.{}", logfile_base, file_num))?;
                f.write_all(mmap)?;
                if unsafe { libc::fallocate(fd as libc::c_int, libc::FALLOC_FL_COLLAPSE_RANGE,
                                            0, self.buf_size as libc::c_long) } < 0 {
                    return Err(LogHandleError::from_errno());
                }
            }
        }

        Ok(())
    }

    fn logrotate(&mut self, path: &str, current_size: u64) -> Result<(), LogHandleError> {
        let (end_num, num_drop) = self.old_logs.shift_logs(path, current_size, self.buf_size)?;
        self.strip_log_head(path, end_num, num_drop)?;
        Ok(())
    }
}

impl<'a> Drop for LogState<'a> {
    fn drop(&mut self) {
        self.mmaps.drain().for_each(|(_, mmap_vals)| {
            let (fd, mmap): (u32, &[u8]) = mmap_vals;
            unsafe { libc::close(fd as libc::c_int) };
            let mut mmap_vec = mmap.to_vec();
            let mmap_len: usize = mmap.len();
            unsafe { libc::munmap(mmap_vec.as_mut_slice() as *mut _ as *mut libc::c_void, mmap_len) };
        })
    }
}
