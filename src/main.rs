//! Helper binary - runs elevated and bridges I/O.

use clap::Parser;
use elevated_helper::{cli::Args, run};

fn main() {
    let args = Args::parse();

    if let Err(e) = run(args) {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}
