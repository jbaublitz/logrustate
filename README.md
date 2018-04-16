# logrustate
logrotate reimplementation using inotify for event based log rotation

## Usage
Simply point logrustate at your logs using the following invocation:

```
logrustate -f ONE_LOG_FILE -f ANOTHER_LOG_FILE -f AND_ANOTHER -s SIZE_OF_PRESERVED_LOGS -n NUMBER_OF_OLD_LOGS
```

Two important notes:
* Logs are `mmap`ed to the process's address space and chunks of size `SIZE_OF_PRESERVED_LOGS` are
taken off of the top while the remaining data is pushed to the beginning of the file using
`fallocate`. This absolutely requires `O_APPEND` opened log files or there will be
undefined behavior and potential data loss otherwise. This is usually
how logs are opened but make sure you are using this option for your logs files before using
this tool.
* Due to limitations in the `fallocate` syscall, log size *must* be a multiple of 4096.
