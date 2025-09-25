use std::{
    fmt::Write,
    path::{Path, PathBuf},
    process::Command,
};

use crate::package_manager::{
    CommandRunner, InstallError, InstallStatus, ManagerIdentifier, Manifest, PackageManager,
};

pub struct ArchPacman {
    bin: PathBuf,
    whoami: ManagerIdentifier,
}

impl ArchPacman {
    pub fn new(whoami: ManagerIdentifier, bin: PathBuf) -> Self {
        Self { bin, whoami }
    }

    pub fn pacman() -> Self {
        Self::default()
    }

    pub fn paru() -> Self {
        Self { bin: PathBuf::from("/usr/bin/paru"), whoami: ManagerIdentifier::ArchParu }
    }

    pub fn yay() -> Self {
        Self { bin: PathBuf::from("/usr/bin/yay"), whoami: ManagerIdentifier::ArchYay }
    }

    pub fn install_script<W: Write>(
        &self,
        list_packages: &[&str],
        writer: &mut W,
    ) -> std::fmt::Result {
        writeln!(
            writer,
            "{} -S --noconfirm --needed --noprogressbar {}",
            self.bin.display(),
            list_packages.join(" ")
        )?;
        Ok(())
    }
}

impl Default for ArchPacman {
    fn default() -> Self {
        Self { bin: PathBuf::from("/usr/sbin/pacman"), whoami: ManagerIdentifier::ArchPacman }
    }
}

impl PackageManager for ArchPacman {
    fn bin(&self) -> &Path {
        &self.bin
    }

    fn whoami(&self) -> ManagerIdentifier {
        self.whoami.clone()
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

    fn install_version(
        &self,
        runner: &dyn CommandRunner,
        package: &str,
        _version: &semver::VersionReq,
    ) -> Result<InstallStatus, InstallError> {
        let bin = self.bin();
        let mut command = Command::new(bin);
        command.arg("-S").arg("--noconfirm").arg("--needed").arg(package);
        let (code, stdout, stderr) = runner.execute(command);

        match (code, stdout.as_str(), stderr.as_str()) {
            (0, _, err) if err.contains("skipping") => {
                Ok(InstallStatus::AlreadyInstalled(Manifest::new(self.whoami(), package)))
            }
            (0, out, _) if out.contains("New Version") => {
                Ok(InstallStatus::Installed(Manifest::new(self.whoami(), package)))
            }
            (c, _, err) if c != 0 && err.contains("could not find all required packages") => {
                Err(InstallError::NotFound(Manifest::new(self.whoami(), package)))
            }
            _ => Err(InstallError::Error { code, stdout, stderr }),
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

    #[test]
    // #[ignore]
    fn test_pacman_install() {
        let pm = crate::package_manager::linux_arch::ArchPacman::paru();
        let mut runner = MockedCommandRunner::default();

        // not found
        // $ paru -S unknown-package
        // Output {
        //     status: ExitStatus(
        //         unix_wait_status(
        //             256,
        //         ),
        //     ),
        //     stdout: ":: Resolving dependencies...\n",
        //     stderr: "error: could not find all required packages:\n    unknown-package (target)\n",
        // }
        runner.add_failure(
            "error: could not find all required packages:\n    unknown-package (target)\n",
        );
        let code = pm.install(&runner, "unknown-package");
        assert!(matches!(code, Err(InstallError::NotFound(_))));

        // already installed
        // $ paru -S fish
        // Output {
        //     status: ExitStatus(
        //         unix_wait_status(
        //             0,
        //         ),
        //     ),
        //     stdout: " there is nothing to do\n",
        //     stderr: "warning: fish-4.0.8-1 is up to date -- skipping\n",
        // }
        runner.add_warning("warning: fish-4.0.8-1 is up to date -- skipping\n");
        let code = pm.install(&runner, "fish");
        assert!(matches!(code, Ok(InstallStatus::AlreadyInstalled(_))));

        // success (status 0)
        // stdout
        // resolving dependencies...
        // looking for conflicting packages...

        // Package (1)  New Version  Net Change

        // extra/fish   4.0.8-1       21.94 MiB

        // Total Installed Size:  21.94 MiB
        runner.add_success("resolving dependencies...\nlooking for conflicting packages...\n\nPackage (1)  New Version  Net Change\n\nextra/fish   4.0.8-1       21.94 MiB\n\nTotal Installed Size:  21.94 MiB");
        let code = pm.install(&runner, "nushell");
        assert!(matches!(code, Ok(InstallStatus::Installed(_))));
    }
}
