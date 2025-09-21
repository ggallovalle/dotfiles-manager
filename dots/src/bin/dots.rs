use clap::{CommandFactory, Parser, Subcommand};
use clap_complete;
use dots::Dots;
use miette;
use std::path::PathBuf;

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
    println!("{:#?}", args);
    // println!("cwd: {:?}", std::env::current_dir().unwrap());
    let mut dots = Dots::create(args.config, args.dry_run, args.bundles, args.verbose)?;
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
