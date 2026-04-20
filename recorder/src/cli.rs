use clap::{CommandFactory, FromArgMatches, Parser, Subcommand};
use std::ffi::OsStr;
use std::path::{Path, PathBuf};

#[derive(Parser)]
#[command(version)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

impl Cli {
    pub fn parse_with_runtime_bin_name() -> Self {
        let mut command = Self::command();
        if let Some(bin_name) = current_bin_name() {
            let bin_name: &'static str = Box::leak(bin_name.into_boxed_str());
            command = command.name(bin_name).bin_name(bin_name);
        }

        let matches = command.get_matches();
        Self::from_arg_matches(&matches).unwrap_or_else(|err| err.exit())
    }
}

fn current_bin_name() -> Option<String> {
    std::env::args_os().next().and_then(|arg0| {
        Path::new(&arg0)
            .file_name()
            .and_then(OsStr::to_str)
            .map(str::to_owned)
    })
}

#[derive(Subcommand)]
pub enum Command {
    /// Start a recording session. Writes session info to stdout as JSON.
    Start {
        /// Override default output directory (~/.domino/recordings).
        #[arg(long)]
        out_dir: Option<PathBuf>,
    },
    /// Stop the currently active recording session.
    Stop,
    /// Print active session info as JSON, or "{}" if none.
    Status,
    /// Print diagnostic info about permissions, devices, OS version.
    Doctor,
}
