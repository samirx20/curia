# Attributions

Third-party code Curia uses, with its licence. Only permissive licences are allowed (MIT, Apache-2.0, BSD, ISC, zlib, MPL-2.0, Unlicense, Unicode); LGPL only when dynamically linked; never GPL or AGPL. Every new dependency, direct or indirect, is checked and listed here.

Checked 2026-11-06 with `cargo metadata` (every crate the build resolves).

## Rust (host, core)

| Crate | Version | Licence | Pulled in by |
|---|---|---|---|
| anyhow | 1.0.104 | MIT OR Apache-2.0 | host |
| serde, serde_core, serde_derive | 1.0.229 | MIT OR Apache-2.0 | host, core |
| serde_json | 1.0.154 | MIT OR Apache-2.0 | host, core |
| tokio | 1.54.1 | MIT | host |
| tokio-macros | 2.7.2 | MIT | tokio |
| windows-sys | 0.61.2 | MIT OR Apache-2.0 | host (Windows) |
| windows-link | 0.2.1 | MIT OR Apache-2.0 | windows-sys |
| bytes | 1.12.1 | MIT | tokio |
| mio | 1.2.4 | MIT | tokio |
| pin-project-lite | 0.2.17 | Apache-2.0 OR MIT | tokio |
| signal-hook-registry | 1.4.8 | MIT OR Apache-2.0 | tokio |
| libc | 0.2.191 | MIT OR Apache-2.0 | tokio, mio |
| errno | 0.3.14 | MIT OR Apache-2.0 | tokio |
| wasi | 0.11.1 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT | mio |
| memchr | 2.8.3 | Unlicense OR MIT | serde_json |
| itoa | 1.0.18 | MIT OR Apache-2.0 | serde_json |
| zmij | 1.0.23 | MIT | serde_json |
| proc-macro2 | 1.0.107 | MIT OR Apache-2.0 | serde_derive, tokio-macros (build time) |
| quote | 1.0.47 | MIT OR Apache-2.0 | serde_derive, tokio-macros (build time) |
| syn | 3.0.9 | MIT OR Apache-2.0 | serde_derive, tokio-macros (build time) |
| unicode-ident | 1.0.27 | (MIT OR Apache-2.0) AND Unicode-3.0 | proc-macro2 (build time) |

### Storage and ids (added 2026-11-06)

| Crate | Version | Licence | Pulled in by |
|---|---|---|---|
| rusqlite | 0.41.1 | MIT | host |
| libsqlite3-sys | 0.39.1 | MIT; bundles **SQLite**, which is public domain | rusqlite |
| bitflags | 2.13.2 | MIT OR Apache-2.0 | rusqlite |
| fallible-iterator | 0.3.0 | MIT/Apache-2.0 | rusqlite |
| fallible-streaming-iterator | 0.1.9 | MIT/Apache-2.0 | rusqlite |
| hashlink | 0.12.2 | MIT OR Apache-2.0 | rusqlite |
| hashbrown | 0.17.1 | MIT OR Apache-2.0 | hashlink |
| foldhash | 0.2.0 | Zlib | hashbrown |
| smallvec | 1.16.4 | MIT OR Apache-2.0 | rusqlite |
| cc, find-msvc-tools, shlex | 1.11.1, 0.1.15, 2.0.1 | MIT OR Apache-2.0 | libsqlite3-sys (build time) |
| pkg-config | 0.3.34 | MIT OR Apache-2.0 | libsqlite3-sys (build time) |
| vcpkg | 0.2.15 | MIT/Apache-2.0 | libsqlite3-sys (build time) |
| uuid | 1.29.0 | Apache-2.0 OR MIT | host |
| getrandom | 0.4.3 | MIT OR Apache-2.0 | uuid |
| cfg-if | 1.0.5 | MIT OR Apache-2.0 | getrandom |
| r-efi | 6.0.0 | MIT OR Apache-2.0 OR LGPL-2.1-or-later; **used under MIT** (UEFI targets only, never built here) | getrandom |

Listed by Cargo but only built for WebAssembly targets, never for Windows, Linux or macOS: sqlite-wasm-rs 0.6.2 (MIT), rsqlite-vfs 0.2.0 (MIT), thiserror / thiserror-impl 2.0.21 (MIT OR Apache-2.0), wasm-bindgen 0.2.130 and its macro crates (MIT OR Apache-2.0), js-sys 0.3.107 (MIT OR Apache-2.0), futures-core / futures-task / futures-util 0.3.34 (MIT OR Apache-2.0), slab 0.4.12 (MIT), bumpalo 3.20.3 (MIT OR Apache-2.0), rustversion 1.0.23 (MIT OR Apache-2.0).
