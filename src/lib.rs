use std::{
    io::{BufRead, BufReader, Read, Write},
    os::windows::process::CommandExt,
    process::Command,
    thread,
};

use conpty::Process;
use interprocess::os::windows::named_pipe::{DuplexPipeStream, pipe_mode};

pub use interprocess;
use windows::Win32::{
    Foundation::CloseHandle,
    System::Threading::{
        OpenProcess, PROCESS_CREATION_FLAGS, PROCESS_QUERY_INFORMATION, PROCESS_SET_INFORMATION,
        SetPriorityClass,
    },
};

use crate::cli::Args;

pub mod cli;
pub fn run(args: Args) -> std::io::Result<()> {
    let stdin_pipe: DuplexPipeStream<pipe_mode::Bytes> =
        DuplexPipeStream::connect_by_path(format!(r"\\.\pipe\{}", args.name_pipe_stdin))?;

    let mut stdout_pipe: DuplexPipeStream<pipe_mode::Bytes> =
        DuplexPipeStream::connect_by_path(format!(r"\\.\pipe\{}", args.name_pipe_stdout))?;

    let mut cmd = Command::new(&args.program_path);
    cmd.args(&args.program_args);
    if let Some(flags) = args.creation_flags {
        cmd.creation_flags(flags);
    }
    let mut child = Process::spawn(cmd)?;
    if let Some(priority) = args.priority {
        set_process_priority(child.pid(), PROCESS_CREATION_FLAGS(priority))?;
    }

    let process_stdin = child.input().unwrap();
    let process_stdout = child.output().unwrap();

    thread::spawn(move || {
        bridge_input(stdin_pipe, process_stdin);
    });

    thread::spawn(move || {
        bridge_output(process_stdout, &mut stdout_pipe);
    });

    let status = child.wait(None)?;

    std::process::exit(status as i32);
}

fn bridge_input(mut from: DuplexPipeStream<pipe_mode::Bytes>, mut to: impl Write) {
    let mut buf = [0u8; 4096];
    loop {
        match from.read(&mut buf) {
            Ok(0) | Err(_) => break,
            Ok(n) => {
                if to.write_all(&buf[..n]).is_err() {
                    break;
                }
                let _ = to.flush();
            }
        }
    }
}

fn bridge_output(from: impl Read, to: &mut impl Write) {
    let reader = BufReader::new(from);
    for line in reader.lines() {
        match line {
            Ok(line) => {
                if writeln!(to, "{}", line).is_err() {
                    break;
                }
                let _ = to.flush();
            }
            Err(_) => break,
        }
    }
}

fn set_process_priority(pid: u32, priority: PROCESS_CREATION_FLAGS) -> windows::core::Result<()> {
    unsafe {
        let handle = OpenProcess(
            PROCESS_SET_INFORMATION | PROCESS_QUERY_INFORMATION,
            false,
            pid,
        )?;
        if handle.is_invalid() {
            return Err(windows::core::Error::from_win32());
        }

        let success = SetPriorityClass(handle, priority);
        CloseHandle(handle)?;
        if success.is_ok() {
            return Err(windows::core::Error::from_win32());
        }

        Ok(())
    }
}
