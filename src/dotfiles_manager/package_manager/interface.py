from dataclasses import dataclass
from typing import Protocol


from semantic_version import Version, Spec


@dataclass
class PackageSpec:
    id: str
    version: Spec | None = None


class PackageManager(Protocol):
    id: str

    def versions(self, package: str) -> list[Version]: ...

    def install(self, spec: PackageSpec | str | list[PackageSpec | str]):
        None


class PackageNotFound(Exception):
    package: str
    manager: str

    def __init__(self, package: str, manager: str) -> None:
        self.package = package
        self.manager = manager
        super().__init__(f"Package '{package}' not found in manager '{manager}'")


class PackageManagerNotFound(Exception):
    manager: str

    def __init__(self, manager: str) -> None:
        self.manager = manager
        super().__init__(f"Package manager '{manager}' not found")


class PackageInstallError(Exception):
    manager: str
    spec: PackageSpec

    def __init__(self, spec: PackageSpec, manager: str) -> None:
        self.spec = spec
        self.manager = manager
        super().__init__(f"Manager '{manager}' failed to install package '{spec}'")


class PackageInstallBunchError(Exception):
    manager: str
    specs: list[PackageSpec | str]

    def __init__(self, specs: list[PackageSpec | str], manager: str) -> None:
        self.specs = specs
        self.manager = manager
        super().__init__(f"Manager '{manager}' failed to install packages '{specs}'")
