# Erdős 1056 k=15 capsule

This non-authoritative verifier independently recomputes the exact inclusive
prime range selected by its compile-time bounds. The active build covers
`10428801..10429000`. It accepts one byte-exact artifact describing
either the first 16-cut factorial-residue witness in that range or the complete
negative scan. A negative result is only about this finite range.

Build the active static Linux ARM64 capsule with
`aarch64-linux-gnu-g++ (GCC) 15.2.0`:

```bash
mkdir -p capsules/erdos1056-k15/bin/linux-arm64/10428801-10429000
aarch64-linux-gnu-g++ -O3 -std=c++20 -static -s \
  -DCANOPUS_RANGE_START=10428801 -DCANOPUS_RANGE_END=10429000 \
  capsules/erdos1056-k15/verifier.cpp \
  -o capsules/erdos1056-k15/bin/linux-arm64/10428801-10429000/verifier
```

The active Linux ARM64 capsule SHA-256 root is
`c9533a8650ca2a76f37c4d482e5467849eb6df4e11d18759207abd94739293f3`.

The Linux x86-64 capsule was built in `alpine:3.22.1` for `linux/amd64`,
pinned at
`sha256:4bcff63911fcb4448bd4fdacec207030997caf25e9bea4045fa6c8c44de311d1`,
using exact package `g++ 14.2.0-r6` with the same flags. Its SHA-256 root is
`f9c33462ce457cf465aa95ae7e557a33c3c3988f5a9b7891b2accedb0eabee9e`.

Completed range binaries and registrations remain recoverable from their
recorded Git commits and release evidence; they are intentionally absent from
the active package.

The prepared mission copies the executable into its content-addressed bundle.
The separate verifier container has no network and no writable persistent
mounts.
