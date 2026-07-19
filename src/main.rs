pub mod calculator;
pub mod cli;
pub mod rates;
pub mod region;
pub mod tui;

use clap::Parser;
use std::process;

fn main() {
    // If no command-line arguments are provided, start the TUI
    if std::env::args().len() == 1 {
        if let Err(e) = tui::run() {
            eprintln!("TUI Error: {}", e);
            process::exit(1);
        }
    } else {
        // Otherwise, run the non-interactive CLI
        let args = cli::CliArgs::parse();
        let exit_code = cli::run(args);
        process::exit(exit_code);
    }
}
