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

### Desktop app (`desktop/`, its own Cargo workspace; added 2026-11-07)

| What | Version | Licence | Notes |
|---|---|---|---|
| **Qt 6** (Core, Gui, Qml, Quick, Quick Controls) | 6.12.0 | LGPL-3.0 | **Dynamically linked** (Qt's own DLLs, unmodified) |
| cxx-qt, cxx-qt-lib, cxx-qt-build, cxx-qt-gen, cxx-qt-macro | 0.10.0 | MIT OR Apache-2.0 | the Rust ↔ Qt bridge |
| qt-build-utils | 0.10.0 | MIT OR Apache-2.0 | build time |
| cxx, cxx-build, cxx-gen, cxxbridge-flags, cxxbridge-macro | 1.0.205 / 0.7.205 | MIT OR Apache-2.0 | the Rust ↔ C++ bridge |
| link-cplusplus, scratch, static_assertions | 1.0.12, 1.0.9, 1.1.0 | MIT OR Apache-2.0 | cxx, cxx-qt |
| clang-format, convert_case, indoc | 0.3.0, 0.6.0, 2.0.8 | MIT OR Apache-2.0 / MIT | cxx-qt-gen (build time) |
| codespan-reporting | 0.11.1, 0.13.1 | Apache-2.0 | build time |
| termcolor, winapi-util | 1.4.1, 0.1.11 | Unlicense OR MIT | build time |
| unicode-segmentation, unicode-width, semver, jobserver | 1.13.3, 0.1.14/0.2.3, 1.0.28, 0.1.35 | MIT OR Apache-2.0 | build time |
| **Inter** (variable) | 4.1 | SIL OFL 1.1 | `desktop/fonts/InterVariable.ttf`, licence text beside it |
| **JetBrains Mono** | 2.x | SIL OFL 1.1 | `desktop/fonts/JetBrainsMono.ttf`, licence text beside it |

### Control room: MCP server (added 2026-11-06)

| Crate | Version | Licence | Pulled in by |
|---|---|---|---|
| rmcp, rmcp-macros | 3.9.0 | Apache-2.0 | host (the official Rust MCP SDK) |
| schemars, schemars_derive | 1.2.3 | MIT | rmcp (tool input schemas) |
| serde_derive_internals | 0.30.0 | MIT OR Apache-2.0 | schemars_derive (build time) |
| dyn-clone | 1.0.20 | MIT OR Apache-2.0 | schemars |
| ref-cast, ref-cast-impl | 1.0.27 | MIT OR Apache-2.0 | schemars |
| indexmap | 2.14.2 | Apache-2.0 OR MIT | rmcp |
| equivalent | 1.0.3 | Apache-2.0 OR MIT | indexmap |
| chrono | 0.4.45 | MIT OR Apache-2.0 | rmcp, schemars |
| num-traits | 0.2.19 | MIT OR Apache-2.0 | chrono |
| autocfg | 1.5.1 | Apache-2.0 OR MIT | num-traits (build time) |
| iana-time-zone | 0.1.65 | MIT OR Apache-2.0 | chrono |
| windows-core, windows-implement, windows-interface, windows-result, windows-strings | 0.62.2, 0.60.2, 0.59.3, 0.4.1, 0.5.1 | MIT OR Apache-2.0 | iana-time-zone (Windows) |
| async-trait | 0.1.92 | MIT OR Apache-2.0 | rmcp |
| pastey | 0.2.3 | MIT OR Apache-2.0 | rmcp |
| sha2 | 0.11.1 | MIT OR Apache-2.0 | rmcp |
| hybrid-array | 0.4.15 | MIT OR Apache-2.0 | sha2's digest crates |
| const-oid | 0.10.2 | Apache-2.0 OR MIT | digest |
| chacha20 | 0.10.2 | MIT OR Apache-2.0 | rand |
| sse-stream | 0.2.6 | MIT OR Apache-2.0 | rmcp |
| tokio-stream | 0.1.19 | MIT | rmcp |
| tokio-util | 0.7.20 | MIT | rmcp |
| futures-channel, futures-executor, futures-io | 0.3.34 | MIT OR Apache-2.0 | futures-util |
| tracing-attributes | 0.1.31 | MIT | tracing |
| darling, darling_core, darling_macro | 0.24.1 | MIT | rmcp-macros (build time) |
| ident_case | 1.0.1 | MIT/Apache-2.0 | darling_core (build time) |
| strsim | 0.11.1 | MIT | darling_core (build time) |

Only built on Android or Haiku, never here: android_system_properties 0.1.6, iana-time-zone-haiku 0.1.2 (MIT OR Apache-2.0). Only on macOS/iOS: core-foundation-sys 0.8.7 (MIT OR Apache-2.0).

Listed by Cargo but only built for WebAssembly targets, never for Windows, Linux or macOS: sqlite-wasm-rs 0.6.2 (MIT), rsqlite-vfs 0.2.0 (MIT), thiserror / thiserror-impl 2.0.21 (MIT OR Apache-2.0), wasm-bindgen 0.2.130 and its macro crates (MIT OR Apache-2.0), js-sys 0.3.107 (MIT OR Apache-2.0), bumpalo 3.20.3 (MIT OR Apache-2.0), rustversion 1.0.23 (MIT OR Apache-2.0), wasip2 1.0.4 and wit-bindgen 0.57.1 (Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT).

### Paired devices: signature checks (added 2026-11-07)

ECDSA P-256 from the RustCrypto project, to check the signatures devices make with their own keys.

| Crate | Version | Licence | Pulled in by |
|---|---|---|---|
| p256 | 0.14.0 | Apache-2.0 OR MIT | host |
| base64 | 0.23.1 | MIT OR Apache-2.0 | host (already used by the WebSocket crates) |
| ecdsa, elliptic-curve, primeorder, primefield | 0.17.0, 0.14.3, 0.14.0, 0.14.0 | Apache-2.0 OR MIT | p256 |
| ff, group, wnaf | 0.14.0, 0.14.0, 0.14.1 | MIT OR Apache-2.0 | elliptic-curve |
| crypto-bigint, ctutils, cmov, cpubits | 0.7.10, 0.4.8, 0.5.4, 0.1.1 | Apache-2.0 OR MIT | elliptic-curve |
| sec1, pkcs8, spki, der, pem-rfc7468, base16ct, base64ct, serdect | 0.8.1, 0.11.0, 0.8.1, 0.8.3, 1.0.0, 1.0.0, 1.8.3, 0.4.3 | Apache-2.0 OR MIT | key formats |
| signature, rfc6979, hmac | 3.0.0, 0.6.0, 0.13.0 | Apache-2.0 OR MIT | ecdsa |
| subtle | 2.6.1 | BSD-3-Clause | constant-time comparisons |
| zeroize | 1.9.1 | Apache-2.0 OR MIT | clears secrets from memory |

### HTTPS and push notifications (added 2026-11-07)

TLS for the phone link (rustls with the `ring` backend), and Web Push: message encryption (RFC 8291) and VAPID signatures (RFC 8292), sent over rustls with the system's trusted certificates.

| Crate | Version | Licence | Pulled in by |
|---|---|---|---|
| rustls | 0.23.46 | Apache-2.0 OR ISC OR MIT | host |
| tokio-rustls | 0.26.6 | MIT OR Apache-2.0 | host |
| rustls-pki-types | 1.15.1 | MIT OR Apache-2.0 | host |
| rustls-webpki | 0.103.16 | ISC | rustls |
| ring | 0.17.14 | Apache-2.0 AND ISC | rustls (crypto backend) |
| untrusted | 0.9.0 | ISC | ring, rustls-webpki |
| rustls-native-certs | 0.8.5 | Apache-2.0 OR ISC OR MIT | host (trusted certificates for push services) |
| schannel | 0.1.30 | MIT | rustls-native-certs (Windows) |
| security-framework, security-framework-sys, core-foundation | 3.7.0, 2.17.0, 0.10.1 | MIT OR Apache-2.0 | rustls-native-certs (macOS) |
| openssl-probe | 0.2.2 | MIT OR Apache-2.0 | rustls-native-certs (Linux; finds certificate files, links no OpenSSL) |
| aes-gcm, aead, aes, cipher, ctr, ghash, polyval, universal-hash, inout | 0.11.1, 0.6.1, 0.9.4, 0.5.2, 0.10.1, 0.6.0, 0.7.3, 0.6.1, 0.2.5 | MIT OR Apache-2.0 | host (push encryption) |
| hkdf | 0.13.0 | MIT OR Apache-2.0 | host (push encryption) |
| getrandom | 0.2.17, 0.3.4, 0.4.3 | MIT OR Apache-2.0 | host, ring (randomness) |
| windows-sys, windows-targets, windows_* | 0.52.x | MIT OR Apache-2.0 | ring, schannel (Windows) |

## Web app (`web/`, added 2026-11-07)

Built with Node; only Svelte's runtime ships, inside the built files the host serves.

| Package | Version | Licence | Ships? |
|---|---|---|---|
| svelte | 5.57.3 | MIT | yes (its runtime) |
| vite | 8.3.6 | MIT | no, build tool |
| @sveltejs/vite-plugin-svelte | 7.3.1 | MIT | no, build tool |
| their 36 build-time dependencies | | 32 MIT, 3 Apache-2.0, 1 ISC, 1 BSD-3-Clause; lightningcss (Vite's CSS minifier) MPL-2.0 | no |
