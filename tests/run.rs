use std::{
    io::{ErrorKind, Read},
    path::PathBuf,
    process::{Child, Command},
    sync::atomic::{AtomicUsize, Ordering},
    thread::sleep,
    time::Duration,
};

use elevated_helper::interprocess::os::windows::named_pipe::{
    DuplexPipeStream, PipeListener, PipeListenerOptions, PipeMode, pipe_mode::Bytes,
};

// Runs the helper as its parent does, returning the output pipe content and the exit code.
fn run_helper(program: &str, args: &[&str]) -> (String, Option<i32>) {
    static RUNS: AtomicUsize = AtomicUsize::new(0);
    let run = RUNS.fetch_add(1, Ordering::Relaxed);
    let name = |end: &str| format!("elevated_helper_test_{}_{run}_{end}", std::process::id());
    let listener = |name: &str| {
        PipeListenerOptions::new()
            .path(PathBuf::from(format!(r"\\.\pipe\{name}")))
            .mode(PipeMode::Bytes)
            .nonblocking(true)
            .create_duplex::<Bytes>()
            .unwrap()
    };
    let (stdin, stdout) = (listener(&name("stdin")), listener(&name("stdout")));
    let mut helper = Command::new(env!("CARGO_BIN_EXE_elevated-helper"))
        .args(["--name-pipe-stdin", &name("stdin")])
        .args(["--name-pipe-stdout", &name("stdout")])
        .args(["--program-path", program, "--"])
        .args(args)
        .spawn()
        .unwrap();
    // The parent may accept the pipes after a start error.
    sleep(Duration::from_millis(200));
    let mut output = Vec::new();
    // A helper with nothing to write may exit before the parent accepts.
    if let (Some(_stdin), Some(mut stdout)) =
        (accept(&stdin, &mut helper), accept(&stdout, &mut helper))
    {
        let mut buf = [0; 4096];
        // The pipe ends with the helper.
        while let Ok(n @ 1..) = stdout.read(&mut buf) {
            output.extend_from_slice(&buf[..n]);
        }
    }
    let code = helper.wait().unwrap().code();
    (String::from_utf8_lossy(&output).into_owned(), code)
}

fn accept(
    listener: &PipeListener<Bytes, Bytes>,
    helper: &mut Child,
) -> Option<DuplexPipeStream<Bytes>> {
    loop {
        match listener.accept() {
            Ok(pipe) => {
                pipe.set_nonblocking(false).unwrap();
                return Some(pipe);
            }
            Err(e) if e.kind() == ErrorKind::WouldBlock => {
                if helper.try_wait().unwrap().is_some() {
                    return None;
                }
                sleep(Duration::from_millis(10));
            }
            Err(e) => panic!("{e}"),
        }
    }
}

#[test]
fn start_error_reaches_the_output_pipe() {
    let (output, code) = run_helper(r"C:\missing\xmrig.exe", &[]);
    assert!(
        output.starts_with(r"Error: C:\missing\xmrig.exe: "),
        "{output:?}"
    );
    assert_eq!(code, Some(1));
}

#[test]
fn helper_exits_with_the_code_of_the_program() {
    let (_, code) = run_helper(r"C:\windows\system32\cmd.exe", &["/C", "exit 3"]);
    assert_eq!(code, Some(3));
}
