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
    pub priority: Option<u32>,
}
