use std::path::PathBuf;

use clap::Parser;

#[derive(Parser)]
#[command(name = "elevated-helper")]
#[command(about = "Bridges I/O between named pipes and a child process")]
pub struct Args {
    #[arg(long)]
    pub name_pipe_stdin: String,

    #[arg(long)]
    pub name_pipe_stdout: String,

    #[arg(long)]
    pub program_path: PathBuf,

    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub program_args: Vec<String>,
    #[arg(long)]
    pub creation_flags: Option<u32>,
    /// Priority can not be set by creation flags because Windows will always set elevated
    /// processes to at least normal priority at their start.
    /// The priority must be set after the process has been started.
    #[arg(long)]
    pub priority: Option<u32>,
}

#[cfg(test)]
mod tests {
    use super::Args;
    use clap::Parser;

    #[test]
    fn priority_is_an_option() {
        let args = Args::try_parse_from([
            "elevated-helper",
            "--name-pipe-stdin",
            "stdin",
            "--name-pipe-stdout",
            "stdout",
            "--program-path",
            "xmrig.exe",
            "--priority",
            "16384",
            "--",
            "--threads",
            "6",
        ])
        .unwrap();
        assert_eq!(args.priority, Some(16384));
        assert_eq!(args.program_args, ["--threads", "6"]);
    }
}
