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
cargo publish -p ezdwg-core --dry-run --locked
uv run scripts/bump_version.py --check --tag v0.14.0
```

Use the matching Cargo version for the tag, including any prerelease suffix.
Commit the version and release notes, then push the commit and its `v<version>`
tag to publish through the release workflow. CI checks version consistency;
the release workflow also rejects tags that disagree with `Cargo.toml` before
building distributions.

## Rust crate publication

`ezdwg-core` is the public Rust crate. `ezdwg-python` is a private Cargo
package (`publish = false`) used to build the Python distribution; it keeps
its local path dependency so wheel and sdist builds do not require an
already-published core version.

Before the first automated crate release, create a crates.io API token with
permission to publish `ezdwg-core` and store it as the repository Actions
secret `CARGO_REGISTRY_TOKEN`. The token must permit creating the crate on
its first release. See the [Cargo publishing guide](https://doc.rust-lang.org/cargo/reference/publishing.html)
for account and token setup.

The release workflow tests the Rust crate, runs `cargo publish --dry-run`,
and tests the README examples from the extracted package. After verification
and Python distribution builds succeed, a `v<version>` tag publishes
`ezdwg-core` to crates.io. The crate job also runs when manually dispatching
the workflow on a matching tag; a branch dispatch does not publish the crate.
PyPI publication remains a separate job using `PYPI_API_TOKEN`.

To inspect or publish the crate manually from a clean release checkout:

```bash
cargo package -p ezdwg-core --list --locked
cargo publish -p ezdwg-core --dry-run --locked
cargo login
cargo publish -p ezdwg-core --locked
```

The archive contains the Rust sources, crate README, MIT license, and Cargo
metadata. Python bindings and repository DWG fixtures are not bundled.
Keep `crates/ezdwg-core/LICENSE` identical to the root `LICENSE`; CI checks it.
The README is also the crate-level API documentation, so its examples are
compiled by Rust's documentation tests.

The crate uses the same version as Python; continue using the version helper
above. crates.io versions cannot be overwritten. If one publication job
succeeds and another fails, rerun only the failed jobs instead of republishing
an existing crate version.
