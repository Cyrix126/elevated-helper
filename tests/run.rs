use std::{
    io::{ErrorKind, Read},
    path::PathBuf,
    process::{Child, Command},
    thread::sleep,
    time::Duration,
};

use elevated_helper::interprocess::os::windows::named_pipe::{
    DuplexPipeStream, PipeListener, PipeListenerOptions, PipeMode, pipe_mode::Bytes,
};

// Runs the helper as its parent does, returning the output pipe content and the exit code.
fn run_helper(program: &str) -> (String, Option<i32>) {
    let name = |end: &str| format!("elevated_helper_test_{}_{end}", std::process::id());
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
        .args(["--program-path", program])
        .spawn()
        .unwrap();
    // The parent may accept the pipes after a start error.
    sleep(Duration::from_millis(200));
    let _stdin = accept(&stdin, &mut helper);
    let mut stdout = accept(&stdout, &mut helper);
    let mut output = Vec::new();
    let mut buf = [0; 4096];
    // The pipe ends with the helper.
    while let Ok(n @ 1..) = stdout.read(&mut buf) {
        output.extend_from_slice(&buf[..n]);
    }
    let code = helper.wait().unwrap().code();
    (String::from_utf8_lossy(&output).into_owned(), code)
}

fn accept(listener: &PipeListener<Bytes, Bytes>, helper: &mut Child) -> DuplexPipeStream<Bytes> {
    loop {
        match listener.accept() {
            Ok(pipe) => {
                pipe.set_nonblocking(false).unwrap();
                return pipe;
            }
            Err(e) if e.kind() == ErrorKind::WouldBlock => {
                if let Some(status) = helper.try_wait().unwrap() {
                    panic!("the helper exited before connecting: {status}");
                }
                sleep(Duration::from_millis(10));
            }
            Err(e) => panic!("{e}"),
        }
    }
}

#[test]
fn start_error_reaches_the_output_pipe() {
    let (output, code) = run_helper(r"C:\missing\xmrig.exe");
    assert!(
        output.starts_with(r"Error: C:\missing\xmrig.exe: "),
        "{output:?}"
    );
    assert_eq!(code, Some(1));
}
