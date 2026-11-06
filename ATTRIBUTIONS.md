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

### Client link: WebSocket server and client (added 2026-11-06)

| Crate | Version | Licence | Pulled in by |
|---|---|---|---|
| axum | 0.8.9 | MIT | host |
| axum-core | 0.5.6 | MIT | axum |
| tokio-tungstenite | 0.29.0 | MIT | axum, client |
| tungstenite | 0.29.0 | MIT OR Apache-2.0 | tokio-tungstenite |
| futures-util, futures-core, futures-task, futures-sink, futures-macro | 0.3.34 | MIT OR Apache-2.0 | host, client |
| slab | 0.4.12 | MIT | futures-util |
| hyper | 1.12.0 | MIT | axum |
| hyper-util | 0.1.21 | MIT | axum |
| http | 1.5.0 | MIT OR Apache-2.0 | axum, hyper, tungstenite |
| http-body | 1.1.0 | MIT | axum, hyper |
| http-body-util | 0.1.5 | MIT | axum |
| httparse | 1.10.1 | MIT OR Apache-2.0 | hyper, tungstenite |
| httpdate | 1.0.3 | MIT OR Apache-2.0 | hyper |
| atomic-waker | 1.1.2 | Apache-2.0 OR MIT | hyper |
| tower, tower-layer, tower-service | 0.5.3, 0.3.3, 0.3.3 | MIT | axum |
| sync_wrapper | 1.0.2 | Apache-2.0 | axum, tower |
| matchit | 0.8.4 | MIT AND BSD-3-Clause | axum |
| mime | 0.3.18 | MIT OR Apache-2.0 | axum |
| serde_path_to_error | 0.1.20 | MIT OR Apache-2.0 | axum |
| serde_urlencoded | 0.7.1 | MIT/Apache-2.0 | axum |
| form_urlencoded | 1.2.2 | MIT OR Apache-2.0 | axum |
| percent-encoding | 2.3.2 | MIT OR Apache-2.0 | axum |
| ryu | 1.0.23 | Apache-2.0 OR BSL-1.0; **used under Apache-2.0** | serde_urlencoded |
| base64 | 0.22.1 | MIT OR Apache-2.0 | axum |
| sha1 | 0.10.7 | MIT OR Apache-2.0 | axum, tungstenite (WebSocket handshake) |
| digest, block-buffer, crypto-common | 0.10.7, 0.10.4, 0.1.7 | MIT OR Apache-2.0 | sha1 |
| generic-array | 0.14.7 | MIT | digest |
| typenum | 1.20.1 | MIT OR Apache-2.0 | generic-array |
| cpufeatures | 0.2.17 | MIT OR Apache-2.0 | sha1 |
| version_check | 0.9.5 | MIT/Apache-2.0 | generic-array (build time) |
| data-encoding | 2.11.1 | MIT | tungstenite |
| rand, rand_chacha, rand_core | 0.9.5, 0.9.0, 0.9.5 | MIT OR Apache-2.0 | tungstenite |
| ppv-lite86 | 0.2.21 | MIT OR Apache-2.0 | rand_chacha |
| zerocopy, zerocopy-derive | 0.8.63 | BSD-2-Clause OR Apache-2.0 OR MIT | ppv-lite86 |
| tracing, tracing-core | 0.1.44, 0.1.36 | MIT | axum |
| once_cell | 1.21.4 | MIT OR Apache-2.0 | tracing-core |
| log | 0.4.35 | MIT OR Apache-2.0 | tungstenite |
| socket2 | 0.6.5 | MIT OR Apache-2.0 | tokio |

Listed by Cargo but only built for WebAssembly targets, never for Windows, Linux or macOS: sqlite-wasm-rs 0.6.2 (MIT), rsqlite-vfs 0.2.0 (MIT), thiserror / thiserror-impl 2.0.21 (MIT OR Apache-2.0), wasm-bindgen 0.2.130 and its macro crates (MIT OR Apache-2.0), js-sys 0.3.107 (MIT OR Apache-2.0), bumpalo 3.20.3 (MIT OR Apache-2.0), rustversion 1.0.23 (MIT OR Apache-2.0), wasip2 1.0.4 and wit-bindgen 0.57.1 (Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT).
