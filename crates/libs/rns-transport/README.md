# coyote-rns-transport

## What this is

Coyote's published fork of the upstream `reticulum-rs-transport` crate from
[LXMF-rs](https://github.com/FreeTAKTeam/LXMF-rs): the transport, link,
interface, receipt, and resource layers of the Reticulum network stack.

Apart from the patch list below, only the package name changes: the library
target stays `rns_transport`, so every `use rns_transport::...` path written
against upstream compiles unchanged against this crate.

The version is upstream's version plus Coyote's patches. `0.14.0` is upstream
`0.14.0` with the patch list below applied. Further patches on top of the same
upstream release bump the patch digit (`0.14.x`). When upstream ships its next
release the fork is rebased onto it and takes that version. If upstream itself
ships a `0.14.x` patch release, the fork rebases onto it and takes the next
unused patch number.

## Consumer

```toml
[dependencies]
coyote-rns-transport = "0.14.0"
```

```rust
use rns_transport::destination::link::Link;
```

Replace upstream `reticulum-rs-transport` with this crate rather than adding
it alongside: both crates have the library name `rns_transport` and cannot
coexist in one dependency graph.

## Patch list

- `destination/link_parts/link_sections/next_watchdog_deadline.rs`: the
  `println!("{}", link_close_line(&self.id))` emitted on every link close,
  which printed `link: close <id>`, is removed. Upstream still prints to
  stdout from inside the library; the `log::warn!` on the next line carries
  the same information through the logging facade. A unit test
  (`no_non_test_stdout_or_stderr_prints_in_library_sources`) pins that no
  non-test print remains anywhere under `src/`.
  `tools/scripts/python-lxmd-rust-lxmd-smoke.sh` waits for `link: close` in
  the lxmd log; after this patch that text comes from the tracing subscriber
  rendering the target of the `log::warn!`
  (`rns_transport::destination::link: close <id>`), and only when ANSI colour
  is off, which is why the smoke script sets `NO_COLOR=1` on the lxmd launch.
  When re-applying keep that call a `warn!` inside `destination::link` and
  keep the message starting with `close `. The `link_close_line` helper and
  its `link_close_line_preserves_compatibility_marker` test are now a
  test-only pair, kept to minimise the diff against upstream.

## Rebase recipe

1. Add the upstream remote once:
   `git remote add upstream https://github.com/FreeTAKTeam/LXMF-rs.git`
2. `git fetch upstream --tags`
3. Merge or rebase `main` onto the upstream release tag.
4. Re-apply every entry in the patch list and re-run
   `cargo test -p coyote-rns-transport --lib no_stdout_prints_tests`.
5. Re-apply the rename wherever upstream added new sites:
   `grep -rn reticulum-rs-transport . --exclude-dir=target --exclude-dir=.git --exclude-dir=docs | grep -v Cargo.lock`
   must return only prose that explains the relationship to upstream plus the
   needle literals inside
   `crates/apps/reticulumd/tests/fork_publish_contract.rs`.
6. Bump the version at every site: `version` in this crate's `Cargo.toml`,
   the workspace `[workspace.dependencies] rns-transport` entry,
   `crates/apps/rns-tools/Cargo.toml` and `crates/apps/reticulumd/Cargo.toml`
   (`package = "coyote-rns-transport", version = ...`), the consumer line in
   this README, and the version sentence under What this is.
7. `cargo check --workspace && cargo test -p coyote-rns-transport && cargo publish -p coyote-rns-transport --locked --dry-run`
8. Commit, then either tag `coyote-rns-transport-vX.Y.Z` and push the tag, or
   run the `publish-coyote-rns-transport` workflow by hand from the Actions
   tab, setting the `ref` input to the tag; the dropdown choice alone does not
   select what is published. The repository secret `CARGO_REGISTRY_TOKEN`
   must exist for the publish step. A manual run from a ref that is not a
   release tag skips the tag-matches-version check and publishes whatever
   version that ref carries; publish from the tag when you can.

## Licence

Same as upstream: `EPL-2.0 OR GPL-2.0-or-later`.
