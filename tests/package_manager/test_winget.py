import pytest
import shutil
from semantic_version import Version
from dotfiles_manager.package_manager.winget import WingetManager
from dotfiles_manager.package_manager.interface import (
    PackageNotFound,
    PackageInstallBunchError,
    PackageSpec,
    PackageManager,
)

@pytest.fixture(scope="session")
def winget_available() -> str | None:
    is_exe = shutil.which("winget.exe") is not None
    is_normal = shutil.which("winget") is not None
    if is_exe:
        return "winget.exe"
    elif is_normal:
        return "winget"
    else:
        return None

@pytest.fixture(scope="session")
def winget_manager(winget_available) -> PackageManager:
    if not winget_available:
        pytest.skip("winget is not installed")
    return WingetManager(winget_available)

def test_versions_existing_package(winget_manager: PackageManager):
    versions = winget_manager.versions("Microsoft.WindowsTerminal")
    assert isinstance(versions, list)
    assert all(isinstance(v, Version) for v in versions)

def test_versions_nonexistent_package(winget_manager: PackageManager):
    with pytest.raises(PackageNotFound):
        winget_manager.versions("thispackagedoesnotexist12345")

def test_install_bundle_all_fail(winget_manager: PackageManager):
    bundle = ["thispackagedoesnotexist12345", PackageSpec("anotherfakepkg98765")]
    with pytest.raises(PackageInstallBunchError) as excinfo:
        winget_manager.install(bundle)
    e = excinfo.value
    failed_ids = [s if isinstance(s, str) else getattr(s, "id", None) for s in e.specs]
    assert "thispackagedoesnotexist12345" in failed_ids
    assert "anotherfakepkg98765" in failed_ids

def test_install_bundle_one_fail_one_success(winget_manager: PackageManager):
    bundle = [PackageSpec("Microsoft.WindowsTerminal"), "thispackagedoesnotexist12345"]
    with pytest.raises(PackageInstallBunchError) as excinfo:
        winget_manager.install(bundle)
    e = excinfo.value
    failed_ids = [s if isinstance(s, str) else getattr(s, "id", None) for s in e.specs]
    assert "thispackagedoesnotexist12345" in failed_ids
    assert "Microsoft.WindowsTerminal" not in failed_ids
