import pytest
import shutil
from dotfiles_manager.package_manager.pacman import PacmanManager
from dotfiles_manager.package_manager.interface import (
    PackageNotFound,
    PackageManagerNotFound,
)


@pytest.fixture(scope="session")
def pacman_available():
    return shutil.which("pacman") is not None


@pytest.fixture(scope="session")
def paru_available():
    return shutil.which("paru") is not None


def test_package_manager_not_found():
    manager = PacmanManager(command="notarealmanager123")
    with pytest.raises(PackageManagerNotFound):
        manager.versions("bash")


def test_versions_existing_package(pacman_available):
    if not pacman_available:
        pytest.skip("pacman is not installed")
    manager = PacmanManager()
    versions = manager.versions("bash")  # 'bash' is almost always present
    assert isinstance(versions, list)
    assert versions
    assert all(isinstance(v, str) for v in versions)


def test_versions_existing_package_paru(paru_available):
    if not paru_available:
        pytest.skip("paru is not installed")
    manager = PacmanManager(command="paru")
    versions = manager.versions("bash")
    assert isinstance(versions, list)
    assert versions
    assert all(isinstance(v, str) for v in versions)


def test_versions_nonexistent_package(pacman_available):
    if not pacman_available:
        pytest.skip("pacman is not installed")
    manager = PacmanManager()
    with pytest.raises(PackageNotFound):
        manager.versions("thispackagedoesnotexist12345")


def test_versions_nonexistent_package_paru(paru_available):
    if not paru_available:
        pytest.skip("paru is not installed")
    manager = PacmanManager(command="paru")
    with pytest.raises(PackageNotFound):
        manager.versions("thispackagedoesnotexist12345")
