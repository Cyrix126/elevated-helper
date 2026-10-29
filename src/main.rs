//! Helper binary - runs elevated and bridges I/O.

use clap::Parser;
use elevated_helper::{cli::Args, run};

fn main() {
    let args = Args::parse();

    match run(args) {
        Ok(code) => std::process::exit(code as i32),
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
}
