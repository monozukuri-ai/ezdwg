# /// script
# requires-python = ">=3.11"
# dependencies = []
# ///
"""Update the shared release version and regenerate its lock files using uv."""

from __future__ import annotations

import argparse
import re
import subprocess
import sys
from pathlib import Path

import tomllib

ROOT = Path(__file__).resolve().parents[1]
VERSION = re.compile(
    r"(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)"
    r"(?:-(alpha|beta|rc)\.(0|[1-9][0-9]*))?"
)


def python_version(version: str) -> str:
    match = VERSION.fullmatch(version)
    if match is None:
        raise ValueError("use X.Y.Z or X.Y.Z-{alpha,beta,rc}.N (without a v prefix)")
    major, minor, patch, stage, number = match.groups()
    suffix = {"alpha": "a", "beta": "b", "rc": "rc"}
    return f"{major}.{minor}.{patch}" + (f"{suffix[stage]}{number}" if stage else "")


def read_toml(path: Path) -> dict:
    with path.open("rb") as stream:
        return tomllib.load(stream)


def check_versions(root: Path, tag: str | None = None) -> str:
    workspace = read_toml(root / "Cargo.toml")["workspace"]
    version = workspace["package"]["version"]
    expected_python = python_version(version)
    project = read_toml(root / "pyproject.toml")["project"]
    if "version" in project or "version" not in project.get("dynamic", []):
        raise ValueError("pyproject.toml must use dynamic version metadata")
    locked = read_toml(root / "Cargo.lock")["package"]
    for member in workspace["members"]:
        package = read_toml(root / member / "Cargo.toml")["package"]
        if package["version"] != {"workspace": True}:
            raise ValueError(f"{member} must inherit version.workspace = true")
        versions = [p["version"] for p in locked if p["name"] == package["name"]]
        if versions != [version]:
            raise ValueError(f"Cargo.lock is out of date for {package['name']}")
    packages = read_toml(root / "uv.lock")["package"]
    local = [p for p in packages if p["name"] == project["name"]]
    if len(local) != 1 or local[0].get("source") != {"editable": "."}:
        raise ValueError("uv.lock must contain the local ezdwg package")
    # uv omits the version for local projects with dynamic metadata.
    if local[0].get("version", expected_python) != expected_python:
        raise ValueError("uv.lock contains an outdated ezdwg version")
    if tag is not None and tag != f"v{version}":
        raise ValueError(f"tag {tag!r} does not match v{version}")
    return version


def bump_version(root: Path, version: str) -> None:
    python_version(version)  # Reject invalid input before writing anything.
    paths = [root / name for name in ("Cargo.toml", "Cargo.lock", "uv.lock")]
    original = {path: path.read_bytes() for path in paths}
    manifest = original[paths[0]].decode("utf-8")
    updated, count = re.subn(
        r'(?ms)(^\[workspace\.package\]\r?\n(?:(?!^\[).)*?^version\s*=\s*")[^"]+("[^\n]*)',
        lambda match: f"{match[1]}{version}{match[2]}",
        manifest,
    )
    if count != 1:
        raise ValueError("expected one [workspace.package] version in Cargo.toml")
    try:
        paths[0].write_bytes(updated.encode("utf-8"))
        subprocess.run(["cargo", "update", "--workspace"], cwd=root, check=True)
        subprocess.run(["uv", "lock"], cwd=root, check=True)
        check_versions(root)
    except BaseException:
        for path, contents in original.items():
            path.write_bytes(contents)
        raise


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    action = parser.add_mutually_exclusive_group(required=True)
    action.add_argument("version", nargs="?", help="new Cargo version, e.g. 0.14.0")
    action.add_argument(
        "--check", action="store_true", help="check versions and lock freshness"
    )
    parser.add_argument("--tag", help="also check a release tag (with --check)")
    args = parser.parse_args()
    if args.tag is not None and not args.check:
        parser.error("--tag requires --check")
    try:
        if args.check:
            version = check_versions(ROOT, args.tag)
            subprocess.run(["uv", "lock", "--check"], cwd=ROOT, check=True)
            print(f"Versions and locks are consistent: {version}")
        else:
            bump_version(ROOT, args.version)
            print(
                f"Updated to {args.version}. Review Cargo.toml, Cargo.lock, and uv.lock."
            )
    except (OSError, ValueError, KeyError, subprocess.CalledProcessError) as exc:
        print(f"error: {exc}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
