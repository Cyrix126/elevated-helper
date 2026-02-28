use std::{
    io::{BufRead, BufReader, Read, Write},
    process::Command,
    thread,
};

use conpty::Process;
use interprocess::os::windows::named_pipe::{DuplexPipeStream, pipe_mode};

pub use interprocess;

use crate::cli::Args;

pub mod cli;
pub fn run(args: Args) -> std::io::Result<()> {
    let stdin_pipe: DuplexPipeStream<pipe_mode::Bytes> =
        DuplexPipeStream::connect_by_path(format!(r"\\.\pipe\{}", args.name_pipe_stdin))?;

    let mut stdout_pipe: DuplexPipeStream<pipe_mode::Bytes> =
        DuplexPipeStream::connect_by_path(format!(r"\\.\pipe\{}", args.name_pipe_stdout))?;

    let mut cmd = Command::new(&args.program_path);
    cmd.args(&args.program_args);
    let mut child = Process::spawn(cmd)?;

    let process_stdin = child.input().unwrap();
    let process_stdout = child.output().unwrap();

    let stdin_handle = thread::spawn(move || {
        bridge_input(stdin_pipe, process_stdin);
    });

    let stdout_handle = thread::spawn(move || {
        bridge_output(process_stdout, &mut stdout_pipe);
    });

    let status = child.wait(None)?;

    stdin_handle.join().ok();
    stdout_handle.join().ok();

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
