import pytest
import shutil
from semantic_version import Version
from dotfiles_manager.package_manager.homebrew import HomebrewManager
from dotfiles_manager.package_manager.interface import (
    PackageNotFound,
    PackageManagerNotFound,
    PackageInstallBunchError,
    PackageSpec,
    PackageManager,
)

@pytest.fixture(scope="session")
def brew_available() -> bool:
    return shutil.which("brew") is not None

@pytest.fixture(scope="session")
def brew_manager(brew_available) -> PackageManager:
    if not brew_available:
        pytest.skip("brew is not installed")
    return HomebrewManager()

def test_versions_existing_package(brew_manager: PackageManager):
    versions = brew_manager.versions("bash")
    assert isinstance(versions, list)
    assert versions
    assert all(isinstance(v, Version) for v in versions)

def test_versions_nonexistent_package(brew_manager: PackageManager):
    with pytest.raises(PackageNotFound):
        brew_manager.versions("thispackagedoesnotexist12345")

def test_install_bundle_all_fail(brew_manager: PackageManager):
    bundle = ["thispackagedoesnotexist12345", PackageSpec("anotherfakepkg98765")]
    with pytest.raises(PackageInstallBunchError) as excinfo:
        brew_manager.install(bundle)
    e = excinfo.value
    failed_ids = [s if isinstance(s, str) else getattr(s, "id", None) for s in e.specs]
    assert "thispackagedoesnotexist12345" in failed_ids
    assert "anotherfakepkg98765" in failed_ids

def test_install_bundle_one_fail_one_success(brew_manager: PackageManager):
    bundle = [PackageSpec("bash"), "thispackagedoesnotexist12345"]
    with pytest.raises(PackageInstallBunchError) as excinfo:
        brew_manager.install(bundle)
    e = excinfo.value
    failed_ids = [s if isinstance(s, str) else getattr(s, "id", None) for s in e.specs]
    assert "thispackagedoesnotexist12345" in failed_ids
    assert "bash" not in failed_ids
