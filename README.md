# ColdTrail

ColdTrail is an offline Rust decoder for a fictional but realistic cold-chain
transport telemetry protocol. It models the kind of structured data used by
pharmaceutical, biologics, and food-distribution fleets: route manifests,
environmental samples, compressed replay logs, fragmented reports, and
restricted field-service command scripts.

The repository is arranged for ClusterFuzzLite:

- first-party Rust library code under `src/`
- cargo-fuzz compatible harnesses under `fuzz/fuzz_targets/`
- per-target seed corpora under `fuzz/corpus/`
- root `.clusterfuzzlite/build.sh` that builds every harness into `$OUT`

The crate has no registry dependencies. The fuzz crate uses a vendored
`libfuzzer-sys` shim and the build script links the fuzzing engine provided by
the ClusterFuzzLite image, so clean-checkout builds do not require network
access.
