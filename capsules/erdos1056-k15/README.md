# Erdős 1056 k=15 capsule

This non-authoritative verifier independently recomputes the exact inclusive
prime range selected by its compile-time bounds. The active build covers
`10429001..10429200`. It accepts one byte-exact artifact describing
either the first 16-cut factorial-residue witness in that range or the complete
negative scan. A negative result is only about this finite range.

Build the active static Linux ARM64 capsule with
`aarch64-linux-gnu-g++ (GCC) 15.2.0`:

```bash
mkdir -p capsules/erdos1056-k15/bin/linux-arm64/10429001-10429200
aarch64-linux-gnu-g++ -O3 -std=c++20 -static -s \
  -DCANOPUS_RANGE_START=10429001 -DCANOPUS_RANGE_END=10429200 \
  capsules/erdos1056-k15/verifier.cpp \
  -o capsules/erdos1056-k15/bin/linux-arm64/10429001-10429200/verifier
```

The active Linux ARM64 capsule SHA-256 root is
`0bf0a8ae62313904be8bbbd7c662301bb78516c5f2c97a0da79dc43298165c44`.

The Linux x86-64 capsule was built in `alpine:3.22.1` for `linux/amd64`,
pinned at
`sha256:4bcff63911fcb4448bd4fdacec207030997caf25e9bea4045fa6c8c44de311d1`,
using exact package `g++ 14.2.0-r6` with the same flags. Its SHA-256 root is
`4dbc19b0528675f54133299133543e1d1191665db0a781f519dfea036ecd2f15`.

Completed range binaries and registrations remain recoverable from their
recorded Git commits and release evidence; they are intentionally absent from
the active package.

The prepared mission copies the executable into its content-addressed bundle.
The separate verifier container has no network and no writable persistent
mounts.
