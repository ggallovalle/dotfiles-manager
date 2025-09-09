
import subprocess
from typing import List
from semantic_version import Version
from .interface import PackageInstallError, PackageManager, PackageNotFound, PackageManagerNotFound, PackageSpec



class PackageManagerHelper:
    def __init__(self, command: str = "pacman"):
        self.command = command

    def run_versions(self, package: str) -> List[Version]:
        try:
            result = subprocess.run([
                self.command, "-Si", package
            ], capture_output=True, text=True, check=True)
            versions = []
            for line in result.stdout.splitlines():
                if line.startswith("Version"):
                    version_str = line.split(":", 1)[1].strip()
                    try:
                        versions.append(Version(version_str))
                    except ValueError:
                        continue
            return versions
        except FileNotFoundError:
            raise PackageManagerNotFound(self.command)
        except subprocess.CalledProcessError as e:
            output = (e.stderr or "") + (e.stdout or "")
            if "error: package '" in output and "was not found" in output:
                return []
            raise

    def install(self, spec):
        # Accepts PackageSpec, str, or list of those
        if isinstance(spec, str):
            pkgids = [spec]
            specs = [spec]
        elif isinstance(spec, PackageSpec):
            pkgids = [spec.id]
            specs = [spec]
        elif isinstance(spec, list):
            pkgids = [s.id if isinstance(s, PackageSpec) else s for s in spec]
            specs = [s if isinstance(s, PackageSpec) else PackageSpec(s) for s in spec]
        else:
            raise ValueError("Unsupported spec type")

        # Check which are already installed
        already_installed = []
        to_install = []
        for s, pkgid in zip(specs, pkgids):
            result = subprocess.run([
                self.command, "-Q", pkgid
            ], capture_output=True, text=True)
            if result.returncode == 0:
                already_installed.append(s)
            else:
                to_install.append((s, pkgid))

        failed = []
        successful = already_installed.copy()
        # Try to install all missing at once
        if to_install:
            pkgid_list = [pkgid for _, pkgid in to_install]
            proc = subprocess.run([
                self.command, "-S", "--noconfirm", *pkgid_list
            ], capture_output=True, text=True)
            # Check which succeeded
            for s, pkgid in to_install:
                # Check if installed after install attempt
                result = subprocess.run([
                    self.command, "-Q", pkgid
                ], capture_output=True, text=True)
                if result.returncode == 0:
                    successful.append(s)
                else:
                    failed.append(s)

        if failed:
            from .interface import PackageInstallBunchError
            if isinstance(spec, list):
                raise PackageInstallBunchError(failed, self.command)
            else:
                raise PackageInstallError(failed[0], self.command)
        return None



class PacmanManager(PackageManager):
    id = "pacman"

    def __init__(self, command: str = "pacman"):
        self.helper = PackageManagerHelper(command)
        self.command = command

    def versions(self, package: str) -> List[Version]:
        versions = self.helper.run_versions(package)
        if not versions:
            raise PackageNotFound(package, self.id)
        return versions

    def install(self, spec):
        return self.helper.install(spec)
