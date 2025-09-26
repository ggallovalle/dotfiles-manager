use clap::{CommandFactory, Parser, Subcommand};
use clap_complete;
use dots::Dots;
use miette;
use std::path::PathBuf;
use tracing;

#[derive(Parser, Debug)]
#[command(
    version,
    name = "dots",
    about = "A dotfiles manager",
    author = "Gerson G. <ggallovalle@gmail.com>"
)]
struct Cli {
    /// Select which bundles to operate on (, delimited)
    #[clap(long, value_name = "BUNDLE", value_delimiter = ',', global = true)]
    bundles: Vec<String>,

    /// Path to the config file
    #[clap(short, long, default_value = "dotfiles.kdl", value_name = "FILE", global = true)]
    config: PathBuf,

    /// Dry run mode
    #[clap(long, action, global = true)]
    dry_run: bool,

    /// How verbose the output should be
    #[arg(short = 'v', action = clap::ArgAction::Count, global=true)]
    verbose: u8,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Imply `dependencies doctor` and `dotfiles doctor`
    Doctor,
    /// Imply `dependencies install` and `dotfiles install`
    Install,
    /// Imply `dependencies uninstall` and `dotfiles uninstall`
    Uninstall,
    /// Manage dependencies like in package managers
    Dependencies {
        #[command(subcommand)]
        command: NamespaceCommand,
    },
    /// Manage dotfiles like symlinks, templates and shell additions
    Dotfiles {
        #[command(subcommand)]
        command: NamespaceCommand,
    },

    /// Generate shell completions
    GenerateCompletions {
        /// Type of completions to generate
        #[clap(name = "type", value_enum)]
        shell: clap_complete::Shell,
    },
}

impl Commands {
    fn as_str(&self) -> &'static str {
        match self {
            Commands::Doctor => "doctor",
            Commands::Install => "install",
            Commands::Uninstall => "uninstall",
            Commands::Dependencies { command } => match command {
                NamespaceCommand::Doctor => "dependencies doctor",
                NamespaceCommand::Install => "dependencies install",
                NamespaceCommand::Uninstall => "dependencies uninstall",
            },
            Commands::Dotfiles { command } => match command {
                NamespaceCommand::Doctor => "dotfiles doctor",
                NamespaceCommand::Install => "dotfiles install",
                NamespaceCommand::Uninstall => "dotfiles uninstall",
            },
            Commands::GenerateCompletions { .. } => "generate-completions",
        }
    }
}

#[derive(Subcommand, Debug)]
pub enum NamespaceCommand {
    /// Test if everything is set up correctly
    Doctor,
    /// Install missing items
    Install,
    /// Uninstall items
    Uninstall,
}

fn main() -> miette::Result<()> {
    let args = Cli::parse();
    let verbosity: dots::Verbosity = args.verbose.into();
    init_tracing(&verbosity);

    let span = tracing::span!(
        tracing::Level::INFO,
        "cli",
        cli.bundles = tracing::field::valuable(&args.bundles),
        cli.config = args.config.to_str(),
        cli.dry_run = args.dry_run,
        cli.verbose = verbosity.as_str(),
        cli.command = args.command.as_str(),
        cwd = std::env::current_dir().unwrap().to_str(),
        args = std::env::args().collect::<Vec<_>>().join(" ")
    );
    let _span_guard = span.enter();

    tracing::debug!("starting dots");
    let mut dots = Dots::create(args.config, args.dry_run, args.bundles, verbosity)?;
    match args.command {
        Commands::Doctor => {
            dots.dependencies_doctor()?;
            dots.dotfiles_doctor()?;
        }
        Commands::Install => {
            dots.dependencies_install()?;
            dots.dotfiles_install()?;
        }
        Commands::Uninstall => {
            dots.dependencies_uninstall()?;
            dots.dotfiles_uninstall()?;
        }
        Commands::Dependencies { command } => match command {
            NamespaceCommand::Doctor => {
                dots.dependencies_doctor()?;
            }
            NamespaceCommand::Install => {
                dots.dependencies_install()?;
            }
            NamespaceCommand::Uninstall => {
                dots.dependencies_uninstall()?;
            }
        },
        Commands::Dotfiles { command } => match command {
            NamespaceCommand::Doctor => {
                dots.dotfiles_doctor()?;
            }
            NamespaceCommand::Install => {
                dots.dotfiles_install()?;
            }
            NamespaceCommand::Uninstall => {
                dots.dotfiles_uninstall()?;
            }
        },
        Commands::GenerateCompletions { shell } => {
            let mut cmd = Cli::command();
            let name = cmd.get_name().to_string();
            clap_complete::generate(shell, &mut cmd, name, &mut std::io::stdout());
        }
    }

    Ok(())
}

fn init_tracing(verbosity: &dots::Verbosity) {
    use tracing_appender::rolling::{RollingFileAppender, Rotation};
    use tracing_subscriber::fmt::writer::BoxMakeWriter;
    use tracing_subscriber::{prelude::*, registry::Registry};
    let file_appender = {
        let mut data_home = dirs_next::data_dir().unwrap();
        data_home.push("dots");
        RollingFileAppender::builder()
            .rotation(Rotation::MINUTELY)
            .filename_suffix("dots.log")
            .max_log_files(5)
            .build(data_home)
            .unwrap()
    };

    let console_layer = {
        if matches!(verbosity, dots::Verbosity::Quiet) {
            None
        } else {
            let to_stdout = matches!(verbosity, dots::Verbosity::Verbose);
            let stderr_layer = tracing_subscriber::fmt::Layer::default()
                .with_ansi(to_stdout)
                .with_thread_ids(to_stdout)
                .with_thread_names(to_stdout)
                .with_writer(if to_stdout {
                    BoxMakeWriter::new(std::io::stdout)
                } else {
                    BoxMakeWriter::new(std::io::stderr)
                });

            Some(stderr_layer)
        }
    };

    let file_layer = tracing_subscriber::fmt::Layer::default()
        .json()
        .with_current_span(false)
        .fmt_fields(tracing_subscriber::fmt::format::JsonFields::default())
        .with_writer(file_appender);

    Registry::default().with(file_layer).with(console_layer).init();
}
