use std::{
    ffi::OsString,
    io::{BufRead, BufReader, Read, Write},
    os::windows::process::CommandExt,
    process::Command,
    sync::mpsc,
    thread,
    time::Duration,
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

const OUTPUT_RELAY_TIMEOUT: Duration = Duration::from_secs(5);

pub mod cli;
pub fn run(args: Args) -> std::io::Result<u32> {
    let stdin_pipe: DuplexPipeStream<pipe_mode::Bytes> =
        DuplexPipeStream::connect_by_path(format!(r"\\.\pipe\{}", args.name_pipe_stdin))?;

    let mut stdout_pipe: DuplexPipeStream<pipe_mode::Bytes> =
        DuplexPipeStream::connect_by_path(format!(r"\\.\pipe\{}", args.name_pipe_stdout))?;

    let mut child = match spawn(&args) {
        Ok(child) => child,
        Err(e) => {
            // The parent reads only the pipes.
            let _ = writeln!(stdout_pipe, "Error: {}: {e}", args.program_path.display());
            // Waits for the parent to read it: the parent drops a connection
            // whose client exited before being accepted.
            let _ = stdout_pipe.flush();
            return Err(e);
        }
    };

    let process_stdin = child.input().unwrap();
    let process_stdout = child.output().unwrap();

    thread::spawn(move || {
        bridge_input(stdin_pipe, process_stdin);
    });

    let (output_done, output_relayed) = mpsc::channel();
    thread::spawn(move || {
        bridge_output(process_stdout, &mut stdout_pipe);
        let _ = output_done.send(());
    });

    let code = child.wait(None)?;
    // Closing the pseudo console ends its output once read, and each line is
    // flushed until the parent read it.
    drop(child);
    // Bounded, as a parent that stopped reading would keep this process running.
    let _ = output_relayed.recv_timeout(OUTPUT_RELAY_TIMEOUT);
    Ok(code)
}

fn spawn(args: &Args) -> std::io::Result<Process> {
    let child = Process::spawn(command(args))?;
    if let Some(priority) = args.priority {
        set_process_priority(child.pid(), PROCESS_CREATION_FLAGS(priority))?;
    }
    Ok(child)
}

// conpty joins the program and the arguments into the command line as they are.
fn command(args: &Args) -> Command {
    // Paths can not contain quotes.
    let mut program = OsString::from("\"");
    program.push(&args.program_path);
    program.push("\"");
    let mut cmd = Command::new(program);
    cmd.args(
        args.program_args
            .iter()
            .map(|arg| shell_escape::windows::escape(arg.into()).into_owned()),
    );
    if let Some(flags) = args.creation_flags {
        cmd.creation_flags(flags);
    }
    cmd
}

fn bridge_input(mut from: impl Read, mut to: impl Write) {
    let mut buf = [0u8; 4096];
    loop {
        match from.read(&mut buf) {
            Ok(0) | Err(_) => break,
            Ok(n) => {
                if to.write_all(&buf[..n]).is_err() {
                    return;
                }
                let _ = to.flush();
            }
        }
    }
    // Ctrl+C, as nothing else could stop the program once the parent is gone.
    let _ = to.write_all(&[0x03]);
    let _ = to.flush();
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
        if success.is_err() {
            return Err(windows::core::Error::from_win32());
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_keeps_arguments_whole() {
        let args = Args {
            name_pipe_stdin: String::new(),
            name_pipe_stdout: String::new(),
            program_path: r"C:\Users\Jo Doe\xmrig.exe".into(),
            program_args: ["--rig-id=a b", "--threads", r#"x"y"#, r"C:\my logs\", ""]
                .map(String::from)
                .to_vec(),
            creation_flags: None,
            priority: None,
        };
        let cmd = command(&args);
        assert_eq!(cmd.get_program(), r#""C:\Users\Jo Doe\xmrig.exe""#);
        assert_eq!(
            cmd.get_args().collect::<Vec<_>>(),
            [
                r#""--rig-id=a b""#,
                "--threads",
                r#""x\"y""#,
                r#""C:\my logs\\""#,
                r#""""#
            ]
        );
    }

    #[test]
    fn a_parent_gone_stops_the_program() {
        let mut program_input = Vec::new();
        bridge_input(&b"v\r\n"[..], &mut program_input);
        assert_eq!(program_input, b"v\r\n\x03");
    }

    #[test]
    fn a_program_gone_gets_no_ctrl_c() {
        // Fails every write, counting them.
        struct Gone(usize);
        impl Write for Gone {
            fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
                self.0 += 1;
                Err(std::io::ErrorKind::BrokenPipe.into())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        let mut program = Gone(0);
        bridge_input(&b"v\r\n"[..], &mut program);
        assert_eq!(program.0, 1);
    }
}
