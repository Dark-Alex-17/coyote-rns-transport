# Changelog

The section layout follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/)
and versions follow the cargo semver rules described in the crate README.

## Unreleased

### Added

- In-crate `two_transport_loopback` test fixture: two `Transport`s joined
  over a real `TcpServer` / `TcpClient` pair on 127.0.0.1 with broadcast
  disabled on both sides, plus a smoke test that opens a link and exchanges
  one `Data` packet in each direction.

### Changed

- Workspace clippy configuration names the tokio `MutexGuard`,
  `RwLockReadGuard` and `RwLockWriteGuard` types under
  `await-holding-invalid-types`, with the lint set to `allow` in the
  workspace and `reticulumd` lint tables until the existing hits are
  cleared.

## 0.14.0 - 2026-10-14

Initial fork release, based on upstream `reticulum-rs-transport` 0.14.0.

### Changed

- Package renamed to `coyote-rns-transport`; the library target stays
  `rns_transport`.

### Removed

- The `println!` on link close in `Link::finalize_local_close`; the
  `log::warn!` on the next line carries the information. See
  [docs/fork/PATCHSET.md](https://github.com/Dark-Alex-17/coyote-rns-transport/blob/main/docs/fork/PATCHSET.md).
