use std::{
    io::{BufRead, BufReader, Read, Write},
    process::{Command, Stdio},
    thread,
};

use interprocess::{
    TryClone,
    os::windows::named_pipe::{DuplexPipeStream, pipe_mode},
};

pub use interprocess;

use crate::cli::Args;

pub mod cli;
pub fn run(args: Args) -> std::io::Result<()> {
    let stdin_pipe: DuplexPipeStream<pipe_mode::Bytes> =
        DuplexPipeStream::connect_by_path(format!(r"\\.\pipe\{}", args.name_pipe_stdin))?;

    let stdout_pipe: DuplexPipeStream<pipe_mode::Bytes> =
        DuplexPipeStream::connect_by_path(format!(r"\\.\pipe\{}", args.name_pipe_stdout))?;

    let mut child = Command::new(&args.program_path)
        .args(&args.program_args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    let process_stdin = child.stdin.take().unwrap();
    let process_stdout = child.stdout.take().unwrap();
    let process_stderr = child.stderr.take().unwrap();

    let stdin_handle = thread::spawn(move || {
        bridge_input(stdin_pipe, process_stdin);
    });

    let mut stdout_pipe_clone = stdout_pipe.try_clone().unwrap();
    let stdout_handle = thread::spawn(move || {
        bridge_output(process_stdout, &mut stdout_pipe_clone);
    });

    let mut stderr_pipe = stdout_pipe;
    let stderr_handle = thread::spawn(move || {
        bridge_output(process_stderr, &mut stderr_pipe);
    });

    let status = child.wait()?;

    stdin_handle.join().ok();
    stdout_handle.join().ok();
    stderr_handle.join().ok();

    std::process::exit(status.code().unwrap_or(1));
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
