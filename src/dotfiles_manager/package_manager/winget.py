import subprocess
from typing import List
from semantic_version import Version
from .interface import PackageManager, PackageNotFound, PackageManagerNotFound, PackageSpec, PackageInstallError, PackageInstallBunchError

class WingetManager(PackageManager):
    id = "winget"

    def __init__(self, command: str = "winget"):
        self.command = command

    def versions(self, package: str) -> List[Version]:
        try:
            result = subprocess.run([
                self.command, "show", "--id", package, "--versions"
            ], capture_output=True, text=True, check=True)
            versions = []
            # Version
            # -------
            # 0.1.1
            # 0.1.2
            
            lines = result.stdout.splitlines()
            capture = 0
            for line in lines:
                line = line.strip()
                if line.startswith("Version") or line.startswith("-------"):
                    capture += 1
                    continue
                if capture == 2:
                    versions.append(Version.coerce(line))

            return versions

        except FileNotFoundError:
            raise PackageManagerNotFound(self.command)
        except subprocess.CalledProcessError as e:
            output = (e.stderr or "") + (e.stdout or "")
            if "No package found matching input criteria" in output or "No installed package found" in output:
                raise PackageNotFound(package, self.command)
            raise

    def install(self, spec):
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

        already_installed = []
        to_install = []
        for s, pkgid in zip(specs, pkgids):
            result = subprocess.run([
                self.command, "list", "--id", pkgid
            ], capture_output=True, text=True)
            if result.returncode == 0 and pkgid in result.stdout:
                already_installed.append(s)
            else:
                to_install.append((s, pkgid))

        failed = []
        successful = already_installed.copy()
        if to_install:
            for s, pkgid in to_install:
                proc = subprocess.run([
                    self.command, "install", "--id", pkgid, "--accept-source-agreements", "--accept-package-agreements", "--silent"
                ], capture_output=True, text=True)
                # Check if installed after install attempt
                result = subprocess.run([
                    self.command, "list", "--id", pkgid
                ], capture_output=True, text=True)
                if result.returncode == 0 and pkgid in result.stdout:
                    successful.append(s)
                else:
                    failed.append(s)

        if failed:
            if isinstance(spec, list):
                raise PackageInstallBunchError(failed, self.command)
            else:
                raise PackageInstallError(failed[0], self.command)
        return None
