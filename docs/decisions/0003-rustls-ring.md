# 0003: rustls with the ring provider for all TLS

- Status: accepted (2026-10-21)

## Context
reqwest 0.13 defaults to rustls with aws-lc-rs, which can need cmake/NASM
to build on Windows. The IMAP connection also needs TLS.

## Decision
Use `rustls` with the `ring` provider, installed once at startup, for both
reqwest (`rustls-no-provider` feature, which verifies against the Windows
certificate store through rustls-platform-verifier) and `tokio-rustls` for IMAP
(webpki roots).

## Consequences
One TLS stack, no extra native build tools, and a smaller binary than
shipping two TLS libraries.
