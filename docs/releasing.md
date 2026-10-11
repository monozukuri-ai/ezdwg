# Releasing

The only release version definition is `[workspace.package].version` in the
root `Cargo.toml`. Both Rust crates inherit it, and maturin derives the Python
package version from Cargo. The CLI reads the installed package metadata.

## Update the version

From the repository root, with Rust and uv installed:

```bash
uv run scripts/bump_version.py 0.14.0
uv run scripts/bump_version.py --check
```

Replace the example version with the intended release. The script updates
`Cargo.toml`, refreshes `Cargo.lock` and `uv.lock` without requesting dependency
upgrades, and checks that the versions agree. It restores those three files if
an update fails. The release helper uses Python 3.11 or newer, selected by uv;
the package continues to support Python 3.10 or newer.

Prereleases use Cargo notation, such as `0.14.0-rc.1`; maturin converts that to
the Python version `0.14.0rc1`. The helper accepts stable releases and
`alpha.N`, `beta.N`, and `rc.N` prereleases.

Do not edit the crate versions, `pyproject.toml`, or lock versions manually.
`uv.lock` may omit the local package version because its metadata is dynamic.
The uv cache configuration watches the Cargo manifests so a version change
refreshes Python metadata on the next build or sync.

## Validate and release

Review the diff and update the release notes in `CHANGELOG.md` and
`docs/changelog.md`. Then build and test:

```bash
uv run --with maturin maturin develop --release
uv run --with pytest --extra dxf --extra plot pytest -q tests
cargo test --all --quiet
uv run scripts/bump_version.py --check --tag v0.14.0
```

Use the matching Cargo version for the tag, including any prerelease suffix.
Commit the version and release notes, then push the commit and its `v<version>`
tag to publish through the release workflow. CI checks version consistency;
the release workflow also rejects tags that disagree with `Cargo.toml` before
building distributions.
