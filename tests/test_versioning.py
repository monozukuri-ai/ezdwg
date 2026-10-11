"""Release tooling tests; the helper uses Python 3.11 independently of the API."""

import shutil
import subprocess
from pathlib import Path

import pytest

pytest.importorskip("tomllib", reason="release tooling requires Python 3.11+")

from scripts import bump_version as release


@pytest.fixture
def checkout(tmp_path: Path) -> Path:
    for name in (
        "Cargo.toml",
        "Cargo.lock",
        "uv.lock",
        "pyproject.toml",
        "crates/ezdwg-core/Cargo.toml",
        "crates/ezdwg-python/Cargo.toml",
    ):
        target = tmp_path / name
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(release.ROOT / name, target)
    return tmp_path


def test_repository_versions_are_consistent() -> None:
    version = release.check_versions(release.ROOT)
    assert release.check_versions(release.ROOT, f"v{version}") == version


@pytest.mark.parametrize(
    ("cargo", "python"),
    [
        ("1.2.3", "1.2.3"),
        ("1.2.3-alpha.1", "1.2.3a1"),
        ("1.2.3-beta.2", "1.2.3b2"),
        ("1.2.3-rc.3", "1.2.3rc3"),
    ],
)
def test_python_version(cargo: str, python: str) -> None:
    assert release.python_version(cargo) == python


@pytest.mark.parametrize(
    "version", ["v1.2.3", "1.2", "01.2.3", "1.2.3-rc.01", "1.2.3\n"]
)
def test_invalid_version_leaves_files_unchanged(checkout: Path, version: str) -> None:
    paths = list(checkout.rglob("*.*"))
    before = {path: path.read_bytes() for path in paths}
    with pytest.raises(ValueError, match="use X.Y.Z"):
        release.bump_version(checkout, version)
    assert {path: path.read_bytes() for path in paths} == before


def test_rejects_mismatched_tag(checkout: Path) -> None:
    with pytest.raises(ValueError, match="does not match"):
        release.check_versions(checkout, "v99.0.0")


def test_rejects_stale_cargo_lock(checkout: Path) -> None:
    manifest = checkout / "Cargo.toml"
    old = release.check_versions(checkout)
    manifest.write_text(manifest.read_text().replace(f'"{old}"', '"99.0.0"'))
    with pytest.raises(ValueError, match="Cargo.lock is out of date"):
        release.check_versions(checkout)


def test_rejects_stale_uv_lock(checkout: Path) -> None:
    lock = checkout / "uv.lock"
    lock.write_text(
        lock.read_text().replace(
            'name = "ezdwg"\n', 'name = "ezdwg"\nversion = "99.0.0"\n'
        )
    )
    with pytest.raises(ValueError, match="outdated ezdwg version"):
        release.check_versions(checkout)


@pytest.mark.parametrize("path", ["pyproject.toml", "crates/ezdwg-core/Cargo.toml"])
def test_rejects_duplicate_version_source(checkout: Path, path: str) -> None:
    manifest = checkout / path
    text = manifest.read_text().replace('dynamic = ["version"]', 'version = "99.0.0"')
    manifest.write_text(text.replace("version.workspace = true", 'version = "99.0.0"'))
    with pytest.raises(ValueError, match="dynamic version|inherit version"):
        release.check_versions(checkout)


@pytest.mark.parametrize("failing_tool", ["cargo", "uv"])
def test_failed_update_restores_all_files(
    checkout: Path, monkeypatch, failing_tool: str
) -> None:
    paths = [checkout / name for name in ("Cargo.toml", "Cargo.lock", "uv.lock")]
    before = {path: path.read_bytes() for path in paths}

    def fail(command: list[str], **kwargs) -> None:
        (checkout / "Cargo.lock").write_text("partial cargo update")
        (checkout / "uv.lock").write_text("partial uv update")
        if command[0] == failing_tool:
            raise subprocess.CalledProcessError(1, command)

    monkeypatch.setattr(release.subprocess, "run", fail)
    with pytest.raises(subprocess.CalledProcessError):
        release.bump_version(checkout, "99.0.0")
    assert {path: path.read_bytes() for path in paths} == before


@pytest.mark.parametrize("newline", [b"\n", b"\r\n"])
def test_bump_preserves_manifest_and_uses_lock_tools(
    checkout: Path, monkeypatch, newline: bytes
) -> None:
    manifest = checkout / "Cargo.toml"
    before = manifest.read_bytes().replace(b"\n", newline)
    manifest.write_bytes(before)
    old = release.check_versions(checkout)
    commands = []

    def update(command: list[str], **kwargs) -> None:
        commands.append(command)
        assert kwargs == {"cwd": checkout, "check": True}
        if command[0] == "cargo":
            lock = checkout / "Cargo.lock"
            text = lock.read_text()
            for name in ("ezdwg-core", "ezdwg-python"):
                text = text.replace(
                    f'name = "{name}"\nversion = "{old}"',
                    f'name = "{name}"\nversion = "99.0.0-rc.1"',
                )
            lock.write_text(text)

    monkeypatch.setattr(release.subprocess, "run", update)
    release.bump_version(checkout, "99.0.0-rc.1")
    assert manifest.read_bytes() == before.replace(old.encode(), b"99.0.0-rc.1")
    assert release.check_versions(checkout) == "99.0.0-rc.1"
    assert commands == [["cargo", "update", "--workspace"], ["uv", "lock"]]
