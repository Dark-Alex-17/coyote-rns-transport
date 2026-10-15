# coyote-rns-transport

## What this is

Coyote's published fork of the upstream `reticulum-rs-transport` crate from
[LXMF-rs](https://github.com/FreeTAKTeam/LXMF-rs): the transport, link,
interface, receipt, and resource layers of the Reticulum network stack.

Apart from the patch list below, only the package name changes: the library
target stays `rns_transport`, so every `use rns_transport::...` path written
against upstream compiles unchanged against this crate.

The version follows cargo semver relative to the upstream baseline. `0.14.0`
is upstream `0.14.0` renamed, with the link-close patch applied. Fork patches
that do not break the public API (additive API, internal fixes) bump the patch
digit; a fork change that breaks the public API bumps the minor digit. When
upstream ships its next release the fork is rebased onto it and takes that
version; if upstream itself ships a `0.14.x` patch release, the fork rebases
onto it and takes the next unused patch number.

## Consumer

```toml
[dependencies]
coyote-rns-transport = "0.14.1"
```

```rust
use rns_transport::destination::link::Link;
```

Replace upstream `reticulum-rs-transport` with this crate rather than adding
it alongside: both crates have the library name `rns_transport` and cannot
coexist in one dependency graph.

## Versioning and releases

The root `VERSION` file is the workspace number: the upstream baseline that
`lxmf --version` prints and that HIL evidence records. The fork publishes only
`coyote-rns-transport`, so this crate's version moves independently of it and
root `VERSION` stays at the upstream baseline until the next upstream rebase.

Release notes live in this crate's `CHANGELOG.md`. The per-release ledger of
every file changed since the previous release, with the commit that changed
it, is `docs/fork/PATCHSET.md`. A release is published by pushing the tag
`coyote-rns-transport-v<version>`: `tools/scripts/check-publish-tag-version.sh`
checks that the tag matches the crate version and
`.github/workflows/publish-coyote-rns-transport.yml` runs the publish.

## Patch list

`docs/fork/PATCHSET.md` is the single per-release ledger of every file changed
since the previous release, with the commit that changed it and the upstream
site; earlier sections carry the earlier patches. What `0.14.1` carries on top
of upstream `0.14.0`:

- Link close no longer prints to stdout: the `println!` in
  `Link::finalize_local_close` is removed and the `log::warn!` on the next line
  carries the information. `tools/scripts/python-lxmd-rust-lxmd-smoke.sh`
  waits for the warn's own message, `close /<32 hex link id>/`. When
  re-applying keep that `warn!` and keep its message starting with `close `
  followed by the link id.
- Later Resource segments and Resource retries are dispatched on the link's
  bound interface instead of through the path table, so a `Transport` built
  with broadcast off no longer drops segments 2 and later as `DroppedNoRoute`.
- A Resource advertisement reject is sent after the link lock is released;
  sending it under the lock deadlocked the transport on the first oversize
  request or response advertisement.
- An outbound Resource sender is registered before its advertisement is
  written, so a fast peer's first part request is served; the
  `pending_outgoing` staging map is gone.
- The hex parsers (`AddressHash`, `Identity`, `PrivateIdentity`) return
  `IncorrectHash` on multibyte input instead of panicking.
- `Transport::send_announce` logs and returns instead of panicking when the
  announce cannot be built; `try_send_announce` returns the error.
- An inbound Resource advertisement above the reference cap of
  `3 * MAX_EFFICIENT_SIZE` is dropped before any receiver state is created
  (`RNS/Resource.py:1374`).
- Additive API: `Transport::stop`, `Link::rtt`,
  `destination_hash::destination_address`, `AddressHash::short`, and the
  `rmpv` and `rand_core` re-exports from the crate root.

## Rebase recipe

1. Add the upstream remote once:
   `git remote add upstream https://github.com/FreeTAKTeam/LXMF-rs.git`
2. `git fetch upstream --tags`
3. Merge or rebase `main` onto the upstream release tag.
4. Re-apply every entry in the patch list and re-run
   `cargo test -p coyote-rns-transport --lib no_stdout_prints_tests`.
5. Re-apply the rename wherever upstream added new sites:
   `grep -rn reticulum-rs-transport . --exclude-dir=target --exclude-dir=.git | grep -v Cargo.lock`
   shows the candidates, and the authoritative check is
   `cargo test -p reticulumd --test fork_publish_contract`, whose
   `old_package_name_survives_only_as_prose` walk fails on every runnable form
   of the old name (`-p`, `--package`, `package =`, `name =`, dependency and
   crates.io URL spellings). Prose that explains the relationship to upstream
   and the needle literals inside the test itself are allowed; the only docs
   that may keep the old commands are the historical subtrees the test skips
   (`docs/goals/`, `docs/migrations/`, `docs/adr/`, `docs/release-notes-*`).
6. Bump the version at every site: `version` in this crate's `Cargo.toml`,
   the workspace `[workspace.dependencies] rns-transport` entry,
   `crates/apps/rns-tools/Cargo.toml` and `crates/apps/reticulumd/Cargo.toml`
   (`package = "coyote-rns-transport", version = ...`), the consumer line in
   this README and in `docs/getting-started.md`, and the version pins in
   `crates/apps/reticulumd/tests/fork_publish_contract.rs`. Regenerate
   `Cargo.lock` (`cargo update -p coyote-rns-transport --offline`) and add the
   release headings to `CHANGELOG.md` and `docs/fork/PATCHSET.md`.
7. `cargo check --workspace && cargo test -p coyote-rns-transport && cargo test -p reticulumd --test fork_publish_contract && tools/scripts/check-boundaries.sh && cargo publish -p coyote-rns-transport --locked --dry-run`
8. Commit, then either tag `coyote-rns-transport-vX.Y.Z` and push the tag, or
   run the `publish-coyote-rns-transport` workflow by hand from the Actions
   tab, setting the `ref` input to the tag; the dropdown choice alone does not
   select what is published. The repository secret `CARGO_REGISTRY_TOKEN`
   must exist for the publish step. A manual run from a ref that is not a
   release tag skips the tag-matches-version check and publishes whatever
   version that ref carries; publish from the tag when you can. The workflow
   runs without a GitHub deployment environment; if you want required
   reviewers or tag-only deployment branches, create an environment named
   `crates-io` in the repository settings and add `environment: crates-io`
   to the `publish` job.

## Licence

Same as upstream: `EPL-2.0 OR GPL-2.0-or-later`.
