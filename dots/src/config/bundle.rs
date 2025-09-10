// bundle module for config
use semver;
use crate::config::root::PackageManager;
use std::path::PathBuf;

pub fn example_bundle() {
    println!("This is the bundle config module");
}

#[derive(Debug, Clone)]
pub struct Bundle {
    pub name: String,
    pub dependencies: Vec<Dependency>,
    pub dotfiles: Vec<Dotfile>,
}

#[derive(Debug, Clone)]
pub struct Dependency {
    pub name: String,
    pub version: Option<semver::VersionReq>,
    pub manager: Option<PackageManager>,
}

#[derive(Debug, Clone)]
pub struct Dotfile {
    pub source: DotfileSource,
    pub target: DotfileTarget,
    pub aliases: Vec<Alias>,
    pub shell_additions: Vec<ShellAdition>,
}

#[derive(Debug, Clone)]
pub enum DotfileSource {
    // Git(String), TODO
    Local(PathBuf)
}

#[derive(Debug, Clone)]
pub enum DotfileTarget {
    Copy(PathBuf),
    Config(PathBuf),
    Home(PathBuf),
}

#[derive(Debug, Clone)]
pub struct Alias(pub String, pub String);

#[derive(Debug, Clone)]
pub struct ShellAdition {
    pub shell: ShellRc,
    pub position: ShellPosition,
    pub content: String,
}

#[derive(Debug, Clone)]
pub enum ShellRc {
    Bash,
    Zsh,
    Fish,
}

#[derive(Debug, Clone)]
pub enum ShellPosition {
    Start,
    End,
    Random
}

