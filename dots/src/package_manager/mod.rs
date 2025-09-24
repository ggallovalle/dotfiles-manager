use std::path::{Path, PathBuf};

use indexmap::IndexMap;
use strum::{self, VariantNames};
use which::which;

mod linux_arch;
mod mocked;

#[derive(
    Debug, Clone, Hash, PartialEq, Eq, strum::EnumString, strum::VariantNames, strum::Display,
)]
pub enum ManagerIdentifier {
    #[strum(serialize = "pacman")]
    ArchPacman,
    #[strum(serialize = "yay")]
    ArchYay,
    #[strum(serialize = "paru")]
    ArchParu,
    // #[strum(serialize = "apt")]
    // DebianApt,
    // #[strum(serialize = "brew")]
    // MacBrew,
    // #[strum(serialize = "choco")]
    // WindowsChoco,
    // #[strum(serialize = "winget")]
    // WindowsWinget,
    // #[strum(serialize = "cargo")]
    // RustCargo,
}

impl ManagerIdentifier {
    pub fn which(&self) -> Option<PathBuf> {
        let command = self.to_string();
        which(command).ok()
    }
}

#[derive(Debug, Clone)]
pub struct Manifest {
    pub name: String,
    pub id: String,
    pub manager: ManagerIdentifier,
    pub version: Option<ManifestVersion>,
    pub available_versions: Option<Vec<String>>,
    pub provides: Vec<String>,
}

impl Manifest {
    pub fn matches_version(&self, req: &semver::VersionReq) -> bool {
        if let Some(installed) = &self.version {
            if req == &semver::VersionReq::STAR {
                return true;
            }

            if let ManifestVersion::Semver(ver) = installed {
                return req.matches(&ver);
            }
        }
        false
    }

    pub fn version(&self) -> String {
        if let Some(installed) = &self.version {
            return installed.to_string();
        }
        "not installed".to_string()
    }
}

#[derive(Debug, Clone, strum::Display)]
pub enum ManifestVersion {
    #[strum(to_string = "{0}")]
    Semver(semver::Version),
    #[strum(to_string = "{0}")]
    Unkown(String),
}

impl From<&str> for ManifestVersion {
    fn from(value: &str) -> Self {
        semver::Version::parse(value).ok().map_or_else(
            || ManifestVersion::Unkown(value.to_owned()),
            // NOTE: prerelease complicates matching VersionReq
            |v| ManifestVersion::Semver(semver::Version::new(v.major, v.minor, v.patch)),
        )
    }
}

pub trait PackageManager {
    fn bin(&self) -> &Path;

    fn doctor(&self, runner: &dyn CommandRunner, package: &str) -> Option<Manifest>;
}

pub trait CommandRunner {
    fn execute(&self, command: std::process::Command) -> (i32, String, String);
}

pub struct SystemCommandRunner;

impl Default for SystemCommandRunner {
    fn default() -> Self {
        Self {}
    }
}

impl CommandRunner for SystemCommandRunner {
    fn execute(&self, mut command: std::process::Command) -> (i32, String, String) {
        let output = command.output();
        // dbg!(&command, &output);
        match output {
            Ok(output) => (
                output.status.code().unwrap_or(-1),
                String::from_utf8_lossy(&output.stdout).to_string(),
                String::from_utf8_lossy(&output.stderr).to_string(),
            ),
            Err(e) => (-1, "".to_string(), format!("Failed to execute command: {}", e)),
        }
    }
}

