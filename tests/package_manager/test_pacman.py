import pytest
import shutil
from semantic_version import Version
from dotfiles_manager.package_manager.pacman import PacmanManager
from dotfiles_manager.package_manager.interface import (
    PackageNotFound,
    PackageManagerNotFound,
    PackageInstallBunchError,
    PackageSpec,
    PackageManager,
)



@pytest.fixture(scope="session")
def pacman_available() -> bool:
    return shutil.which("pacman") is not None

@pytest.fixture(scope="session")
def paru_available() -> bool:
    return shutil.which("paru") is not None

@pytest.fixture(scope="session")
def pacman_manager(pacman_available) -> PackageManager:
    if not pacman_available:
        pytest.skip("pacman is not installed")
    return PacmanManager()

@pytest.fixture(scope="session")
def paru_manager(paru_available) -> PackageManager:
    if not paru_available:
        pytest.skip("paru is not installed")
    return PacmanManager(command="paru")


def test_package_manager_not_found():
    manager: PackageManager = PacmanManager(command="notarealmanager123")
    with pytest.raises(PackageManagerNotFound):
        manager.versions("bash")


def test_versions_existing_package(pacman_manager: PackageManager):
    versions = pacman_manager.versions("bash")  # 'bash' is almost always present
    assert isinstance(versions, list)
    assert versions
    assert all(isinstance(v, Version) for v in versions)


def test_versions_existing_package_paru(paru_manager: PackageManager):
    versions = paru_manager.versions("bash")
    assert isinstance(versions, list)
    assert versions
    assert all(isinstance(v, Version) for v in versions)


def test_versions_nonexistent_package(pacman_manager: PackageManager):
    with pytest.raises(PackageNotFound):
        pacman_manager.versions("thispackagedoesnotexist12345")


def test_versions_nonexistent_package_paru(paru_manager: PackageManager):
    with pytest.raises(PackageNotFound):
        paru_manager.versions("thispackagedoesnotexist12345")


def test_install_bundle_all_fail(pacman_manager: PackageManager):
    bundle = ["thispackagedoesnotexist12345", PackageSpec("anotherfakepkg98765")]
    with pytest.raises(PackageInstallBunchError) as excinfo:
        pacman_manager.install(bundle)
    e = excinfo.value
    failed_ids = [s if isinstance(s, str) else getattr(s, "id", None) for s in e.specs]
    assert "thispackagedoesnotexist12345" in failed_ids
    assert "anotherfakepkg98765" in failed_ids


def test_install_bundle_one_fail_one_success(pacman_manager: PackageManager):
    bundle = [PackageSpec("bash"), "thispackagedoesnotexist12345"]
    with pytest.raises(PackageInstallBunchError) as excinfo:
        pacman_manager.install(bundle)
    e = excinfo.value
    failed_ids = [s if isinstance(s, str) else getattr(s, "id", None) for s in e.specs]
    assert "thispackagedoesnotexist12345" in failed_ids
    # Only failed ids are available; bash should not be in failed_ids
    assert "bash" not in failed_ids
