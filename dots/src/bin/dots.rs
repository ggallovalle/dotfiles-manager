use clap::{CommandFactory, Parser, Subcommand};
use clap_complete;
use dots::Dots;
use miette;
use scopeguard;
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

    /// If an existing destination file is found, overwrite it
    #[clap(long, action, global = true)]
    force: bool,

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
    let verbosity = if args.dry_run {
        Some(tracing::Level::DEBUG)
    } else {
        match args.verbose {
            0 => None,
            1 => Some(tracing::Level::INFO),
            2 => Some(tracing::Level::DEBUG),
            _ => Some(tracing::Level::TRACE),
        }
    };
    let _guard_tracing = init_tracing(&verbosity);

    let span = tracing::span!(
        tracing::Level::DEBUG,
        "cli",
        cli.bundles = tracing::field::valuable(&args.bundles),
        cli.config = args.config.to_str(),
        cli.dry_run = args.dry_run,
        cli.verbosity = tracing::field::debug(&verbosity),
        cli.command = args.command.as_str(),
        cli.force = args.force,
        cwd = std::env::current_dir().unwrap().to_str(),
        args = std::env::args().collect::<Vec<_>>().join(" ")
    );
    let _span_guard = span.enter();

    tracing::debug!("starting dots");
    match args.command {
        Commands::GenerateCompletions { shell } => {
            let mut cmd = Cli::command();
            let name = cmd.get_name().to_string();
            clap_complete::generate(shell, &mut cmd, name, &mut std::io::stdout());
        }
        _ => {
            if verbosity.is_none() {
                let mut latest_log = get_logs_dir();
                latest_log.push("dots-latest.log");
                eprintln!("see the latest log file at '{}' for details", latest_log.display());
            }
            let mut dots = Dots::create(args.config, args.dry_run, args.force, args.bundles)
                .inspect_err(trace_dots_error)?;
            let span = tracing::span!(
                tracing::Level::DEBUG,
                "config",
                config.dotfiles_dir = dots.config.dotfiles_dir.to_str(),
                config.env = tracing::field::valuable(&dots.config.env.keys().collect::<Vec<_>>()),
                config.bundles =
                    tracing::field::valuable(&dots.config.bundles.keys().collect::<Vec<_>>()),
            );
            let _span_guard = span.enter();
            execute(args.command, &mut dots).inspect_err(trace_dots_error)?;
        }
    }

    Ok(())
}

fn trace_dots_error(e: &dots::DotsError) {
    match e {
        dots::DotsError::Settings(inner) => {
            tracing::error!(
                diagnostics = tracing::field::valuable(&inner.diagnostics_jsonable()),
                "{}",
                e
            );
        }
        _ => {
            tracing::error!("{}", e);
        }
    }
}

fn execute(command: Commands, dots: &mut Dots) -> Result<(), dots::DotsError> {
    match command {
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
        _ => { /* handled in main */ }
    }
    Ok(())
}

fn get_data_dir() -> PathBuf {
    let mut data_home = dirs_next::data_dir().unwrap();
    data_home.push("dots");
    data_home
}

fn get_logs_dir() -> PathBuf {
    let mut logs_dir = get_data_dir();
    logs_dir.push("logs");
    logs_dir
}

fn init_tracing(verbosity: &Option<tracing::Level>) -> scopeguard::ScopeGuard<(), impl FnOnce(())> {
    use tracing_appender::rolling::{RollingFileAppender, Rotation};
    use tracing_subscriber::fmt::writer::BoxMakeWriter;
    use tracing_subscriber::prelude::*;

    let file_appender = {
        RollingFileAppender::builder()
            .rotation(Rotation::MINUTELY)
            .filename_suffix("dots.log")
            .max_log_files(5)
            .build(get_logs_dir())
            .unwrap()
    };

    let (latest_appender, latest_appender_guard) = {
        let mut latest_log = get_logs_dir();
        latest_log.push("dots-latest.log");
        let file = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(latest_log)
            .unwrap();
        tracing_appender::non_blocking(file)
    };

    let console_layer = match verbosity {
        None => None,
        Some(_) => {
            let stderr_layer = tracing_subscriber::fmt::Layer::default()
                .with_writer(BoxMakeWriter::new(std::io::stderr));
            Some(stderr_layer)
        }
    };

    let env_filter = tracing_subscriber::filter::EnvFilter::from_default_env().add_directive(
        format!("dots={}", verbosity.unwrap_or(tracing::Level::WARN)).parse().unwrap(),
    );

    let file_layer = tracing_subscriber::fmt::Layer::default()
        .json()
        .with_current_span(false)
        .fmt_fields(tracing_subscriber::fmt::format::JsonFields::default())
        .with_writer(file_appender.and(latest_appender));

    tracing_subscriber::registry().with(env_filter).with(file_layer).with(console_layer).init();

    let guard = scopeguard::guard((), move |_| {
        drop(latest_appender_guard);
    });
    guard
}
