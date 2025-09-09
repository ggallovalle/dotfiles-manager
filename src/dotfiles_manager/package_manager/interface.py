from typing import Protocol


class PackageManager(Protocol):
    id: str

    def versions(self, package: str) -> list[str]: ...


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
