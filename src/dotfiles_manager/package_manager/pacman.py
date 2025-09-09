
import subprocess
from typing import List
from semantic_version import Version
from .interface import PackageManager, PackageNotFound, PackageManagerNotFound



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
                        # If version string is not valid, skip it
                        continue
            return versions
        except FileNotFoundError:
            raise PackageManagerNotFound(self.command)
        except subprocess.CalledProcessError as e:
            # Check stderr or stdout for package not found message
            output = (e.stderr or "") + (e.stdout or "")
            if "error: package '" in output and "was not found" in output:
                return []
            raise


class PacmanManager(PackageManager):
    id = "pacman"

    def __init__(self, command: str = "pacman"):
        self.helper = PackageManagerHelper(command)

    def versions(self, package: str) -> List[Version]:
        versions = self.helper.run_versions(package)
        if not versions:
            raise PackageNotFound(package, self.id)
        return versions
