use clap::{CommandFactory, Parser, Subcommand};
use clap_complete;
use miette;
use std::path::PathBuf;
use dots::Dots;

#[derive(Parser, Debug)]
#[command(
    version,
    name = "dots",
    about = "A dotfiles manager",
    author = "Gerson G. <ggallovalle@gmail.com>"
)]
struct Cli {
    /// Path to the config file
    #[clap(short, long, default_value = "dotfiles.kdl", value_name = "FILE")]
    config: PathBuf,

    /// Dry run mode
    #[clap(long, action)]
    dry_run: bool,

    /// How verbose the output should be
    #[arg(short = 'v', action = clap::ArgAction::Count)]
    verbose: u8,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Imply `dependencies doctor` and `dotfiles doctor`
    Doctor {
        /// Select which bundles to operate on
        #[clap(value_name = "BUNDLE")]
        bundles: Vec<String>,
    },
    /// Imply `dependencies install` and `dotfiles install`
    Install {
        /// Select which bundles to operate on
        #[clap(value_name = "BUNDLE")]
        bundles: Vec<String>,
    },
    /// Imply `dependencies uninstall` and `dotfiles uninstall`
    Uninstall {
        /// Select which bundles to operate on
        #[clap(value_name = "BUNDLE")]
        bundles: Vec<String>,
    },
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

#[derive(Subcommand, Debug)]
pub enum NamespaceCommand {
    /// Test if everything is set up correctly
    Doctor { bundles: Vec<String> },
    /// Install missing items
    Install { bundles: Vec<String> },
    /// Uninstall items
    Uninstall { bundles: Vec<String> },
}

fn main() -> miette::Result<()> {
    let args = Cli::parse();
    println!("CLI args: {:#?}", args);
    match args.command {
        Commands::Doctor { bundles } => {
            let mut dots = Dots::create(args.config, args.dry_run, bundles, args.verbose)?;
            dots.dependencies_doctor()?;
            dots.dotfiles_doctor()?;
        },
        Commands::Install { bundles } => {
            let mut dots = Dots::create(args.config, args.dry_run, bundles, args.verbose)?;
            dots.dependencies_install()?;
            dots.dotfiles_install()?;
        },
        Commands::Uninstall { bundles } => {
            let mut dots = Dots::create(args.config, args.dry_run, bundles, args.verbose)?;
            dots.dependencies_uninstall()?;
            dots.dotfiles_uninstall()?;
        },
        Commands::Dependencies { command } => match command {
            NamespaceCommand::Doctor { bundles } => {
                let mut dots = Dots::create(args.config, args.dry_run, bundles, args.verbose)?;
                dots.dependencies_doctor()?;
            },
            NamespaceCommand::Install { bundles } => {
                let mut dots = Dots::create(args.config, args.dry_run, bundles, args.verbose)?;
                dots.dependencies_install()?;
            },
            NamespaceCommand::Uninstall { bundles } => {
                let mut dots = Dots::create(args.config, args.dry_run, bundles, args.verbose)?;
                dots.dependencies_uninstall()?;
            },
        },
        Commands::Dotfiles { command } => match command {
            NamespaceCommand::Doctor { bundles } => {
                let mut dots = Dots::create(args.config, args.dry_run, bundles, args.verbose)?;
                dots.dotfiles_doctor()?;
            },
            NamespaceCommand::Install { bundles } => {
                let mut dots = Dots::create(args.config, args.dry_run, bundles, args.verbose)?;
                dots.dotfiles_install()?;
            },
            NamespaceCommand::Uninstall { bundles } => {
                let mut dots = Dots::create(args.config, args.dry_run, bundles, args.verbose)?;
                dots.dotfiles_uninstall()?;
            },
        },
        Commands::GenerateCompletions { shell } => {
            let mut cmd = Cli::command();
            let name = cmd.get_name().to_string();
            clap_complete::generate(shell, &mut cmd, name, &mut std::io::stdout());
        },
    }

    Ok(())
}
