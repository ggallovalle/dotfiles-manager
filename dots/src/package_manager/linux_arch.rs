use std::{
    path::{Path, PathBuf},
    process::Command,
};

use crate::package_manager::{CommandRunner, ManagerIdentifier, Manifest, PackageManager};

pub struct ArchPacman {
    bin: PathBuf,
}

impl ArchPacman {
    pub fn new(bin: PathBuf) -> Self {
        Self { bin }
    }

    pub fn pacman() -> Self {
        Self::default()
    }

    pub fn paru() -> Self {
        Self { bin: PathBuf::from("/usr/bin/paru") }
    }

    pub fn yay() -> Self {
        Self { bin: PathBuf::from("/usr/bin/yay") }
    }
}

impl Default for ArchPacman {
    fn default() -> Self {
        Self { bin: PathBuf::from("/usr/sbin/pacman") }
    }
}

impl PackageManager for ArchPacman {
    fn bin(&self) -> &Path {
        &self.bin
    }

    fn doctor(&self, runner: &dyn CommandRunner, package: &str) -> Option<Manifest> {
        let bin = self.bin();
        let mut command = Command::new(bin);
        command.arg("-Q").arg(package);
        let (code, stdout, _stderr) = runner.execute(command);
        // stdout: <package> <version>
        // stderr: error: package '<package>' was not found
        if code == 0
            && let Some(((name, version))) = stdout.lines().nth(0).and_then(|v| v.split_once(' '))
        {
            Some(Manifest {
                // pacman might found it under a different name
                // e.g. rustc is provided by rustup
                id: name.to_string(),
                name: package.to_string(),
                manager: ManagerIdentifier::ArchPacman,
                version: Some(version.into()),
                available_versions: None,
                provides: vec![],
            })
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::package_manager::mocked::*;
    use crate::package_manager::*;

    #[test]
    fn test_pacman_doctor() {
        // Given
        let pm = crate::package_manager::linux_arch::ArchPacman::pacman();
        let mut runner = MockedCommandRunner::default();
        let star = &semver::VersionReq::STAR;
        let star_custom = &semver::VersionReq::parse("*").unwrap();

        // When
        runner.add_success("bash 5.1.16-1\n");
        let manifest = pm.doctor(&runner, "bash").unwrap();
        let version_req = &semver::VersionReq::parse(">=5").unwrap();
        assert!(manifest.matches_version(&version_req),);

        // runner.add_failure("error: package 'nonexistent-package' was not found\n");
        let manifest = pm.doctor(&runner, "nonexistent-package");
        assert!(manifest.is_none());

        runner.add_success("rustup gibrish-version\n");
        let manifest = pm.doctor(&runner, "rust");
        let manifest = manifest.unwrap();
        assert_eq!(manifest.version(), "gibrish-version");
        assert_eq!(manifest.id, "rustup");
        assert_eq!(manifest.name, "rust");
        assert!(manifest.matches_version(star),);
        assert!(manifest.matches_version(star_custom),);
        assert!(!manifest.matches_version(version_req),);
    }
}
