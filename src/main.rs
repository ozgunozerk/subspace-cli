//! CLI application for farming
//! brings `farmer` and `node` together

#![deny(missing_docs, clippy::unwrap_used)]
#![feature(concat_idents)]

mod commands;
mod config;
mod summary;
mod utils;

#[cfg(test)]
mod tests;

use std::io::{self, Write};

use clap::{Parser, Subcommand};
use color_eyre::eyre::{Context, Report};
use color_eyre::Help;
use crossterm::event::{Event, KeyCode};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use crossterm::{cursor, execute};
use owo_colors::OwoColorize;
use strum::IntoEnumIterator;
use strum_macros::EnumIter;
use tracing::instrument;

use crate::commands::farm::farm;
use crate::commands::info::info;
use crate::commands::init::init;
use crate::commands::wipe::wipe_config;
use crate::utils::{get_user_input, open_log_dir, support_message, yes_or_no_parser};

#[cfg(all(
    target_arch = "x86_64",
    target_vendor = "unknown",
    target_os = "linux",
    target_env = "gnu"
))]
#[global_allocator]
static GLOBAL: jemallocator::Jemalloc = jemallocator::Jemalloc;

#[derive(Debug, Parser)]
#[command(subcommand_required = false)]
#[command(name = "subspace")]
#[command(about = "Subspace CLI", long_about = None)]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

/// Available commands for the CLI
#[derive(Debug, Subcommand, EnumIter)]
enum Commands {
    #[command(about = "initializes the config file required for the farming")]
    Init,
    #[command(about = "starting the farming process (along with node in the background)")]
    Farm {
        #[arg(short, long, action)]
        verbose: bool,
        #[arg(short, long, action)]
        executor: bool,
        #[arg(long, action)]
        no_rotation: bool,
    },
    #[command(about = "wipes the node and farm instance (along with your plots)")]
    Wipe {
        #[arg(long, action)]
        farmer: bool,
        #[arg(long, action)]
        node: bool,
    },
    #[command(about = "displays info about the farmer instance (i.e. total amount of rewards, \
                       and status of initial plotting)")]
    Info,
    OpenLogs,
}

#[tokio::main]
#[instrument]
async fn main() -> Result<(), Report> {
    let args = Cli::parse();
    match args.command {
        Some(Commands::Info) => {
            info().await.suggestion(support_message())?;
        }
        Some(Commands::Init) => {
            init().suggestion(support_message())?;
        }
        Some(Commands::Farm { verbose, executor, no_rotation }) => {
            farm(verbose, executor, no_rotation).await.suggestion(support_message())?;
        }
        Some(Commands::Wipe { farmer, node }) => {
            wipe_config(farmer, node).await.suggestion(support_message())?;
        }
        Some(Commands::OpenLogs) => {
            open_log_dir().suggestion(support_message())?;
        }
        None => arrow_key_mode().await.suggestion(support_message())?,
    }

    Ok(())
}

#[instrument]
async fn arrow_key_mode() -> Result<(), Report> {
    let mut stdout = io::stdout();

    // Options to be displayed
    let options = Commands::iter().map(|command| command.to_string()).collect::<Vec<_>>();

    // Selected option index
    let mut selected = 0;

    // get the current location of the cursor
    let position = cursor::position()?.1;

    enable_raw_mode()?;

    // Print options to the terminal
    print_options(&mut stdout, &options, selected, position)?;

    // Process input events
    loop {
        if let Event::Key(event) = crossterm::event::read()? {
            match event.code {
                KeyCode::Up | KeyCode::Char('k') => {
                    // Move selection up
                    if selected > 0 {
                        selected -= 1;
                        print_options(&mut stdout, &options, selected, position)?;
                    }
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    // Move selection down
                    if selected < options.len() - 1 {
                        selected += 1;
                        print_options(&mut stdout, &options, selected, position)?;
                    }
                }
                KeyCode::Enter => {
                    break;
                }
                KeyCode::Char('c')
                    if event.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) =>
                {
                    return Ok(());
                }
                _ => {}
            }
        }
    }

    disable_raw_mode()?;

    // Move the cursor two lines below the options
    execute!(stdout, cursor::MoveTo(0, position + options.len() as u16 + 6))?;

    match selected {
        0 => {
            init().suggestion(support_message())?;
        }
        1 => {
            let prompt = "Do you want to initialize farmer in verbose mode? [y/n]: ";
            let verbose =
                get_user_input(prompt, None, yes_or_no_parser).context("prompt failed")?;

            let prompt = "Do you want to be an executor? [y/n]: ";
            let executor =
                get_user_input(prompt, None, yes_or_no_parser).context("prompt failed")?;

            let prompt = "Do you want to disable rotation for logs? [y/n]: ";
            let no_rotation =
                get_user_input(prompt, None, yes_or_no_parser).context("prompt failed")?;

            farm(verbose, executor, no_rotation).await.suggestion(support_message())?;
        }
        2 => {
            wipe_config(false, false).await.suggestion(support_message())?;
        }
        3 => {
            info().await.suggestion(support_message())?;
        }
        4 => {
            open_log_dir().suggestion(support_message())?;
        }
        _ => {
            unreachable!("this number must stay in [0-4]")
        }
    }

    Ok(())
}

// Helper function to print options to the terminal
fn print_options(
    stdout: &mut io::Stdout,
    options: &[String],
    selected: usize,
    position: u16,
) -> io::Result<()> {
    execute!(stdout, cursor::MoveTo(1, position + 2), cursor::SavePosition)?;
    writeln!(stdout, "Please select an option below using arrow keys (or `j` and `k`):\n",)?;

    // Print options to the terminal
    for (i, option) in options.iter().enumerate() {
        if i == selected {
            let output = format!(" > {} ", option);
            writeln!(stdout, "{} {}", cursor::MoveTo(1, i as u16 + position + 4), output.green())?;
        } else {
            let output = format!("  {} ", option);
            writeln!(stdout, "{} {}", cursor::MoveTo(1, i as u16 + position + 4), output)?;
        }
    }
    writeln!(stdout, "\n\r")?;
    stdout.flush()?;

    execute!(stdout, cursor::RestorePosition)?;

    Ok(())
}

impl std::fmt::Display for Commands {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match *self {
            Commands::Farm { verbose: _, executor: _, no_rotation: _ } => write!(f, "farm"),
            Commands::Wipe { farmer: _, node: _ } => write!(f, "wipe"),
            Commands::Info => write!(f, "info"),
            Commands::Init => write!(f, "init"),
            Commands::OpenLogs => write!(f, "open logs directory"),
        }
    }
}
