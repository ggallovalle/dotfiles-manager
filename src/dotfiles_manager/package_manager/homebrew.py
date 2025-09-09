import subprocess
from typing import List
from semantic_version import Version
from .interface import PackageManager, PackageNotFound, PackageManagerNotFound, PackageSpec, PackageInstallError, PackageInstallBunchError

class HomebrewManager(PackageManager):
    id = "brew"

    def __init__(self, command: str = "brew"):
        self.command = command

    def versions(self, package: str) -> List[Version]:
        try:
            result = subprocess.run([
                self.command, "info", "--json=v2", package
            ], capture_output=True, text=True, check=True)
            import json
            info = json.loads(result.stdout)
            versions = []
            for formula in info.get("formulae", []):
                v = formula.get("versions", {}).get("stable")
                if v:
                    try:
                        versions.append(Version(v))
                    except ValueError:
                        continue
            return versions
        except FileNotFoundError:
            raise PackageManagerNotFound(self.command)
        except subprocess.CalledProcessError as e:
            output = (e.stderr or "") + (e.stdout or "")
            if f"Error: No available formula with the name \"{package}\"" in output:
                return []
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
                self.command, "list", pkgid
            ], capture_output=True, text=True)
            if result.returncode == 0:
                already_installed.append(s)
            else:
                to_install.append((s, pkgid))

        failed = []
        successful = already_installed.copy()
        if to_install:
            pkgid_list = [pkgid for _, pkgid in to_install]
            proc = subprocess.run([
                self.command, "install", *pkgid_list
            ], capture_output=True, text=True)
            for s, pkgid in to_install:
                result = subprocess.run([
                    self.command, "list", pkgid
                ], capture_output=True, text=True)
                if result.returncode == 0:
                    successful.append(s)
                else:
                    failed.append(s)

        if failed:
            if isinstance(spec, list):
                raise PackageInstallBunchError(failed, self.command)
            else:
                raise PackageInstallError(failed[0], self.command)
        return None
