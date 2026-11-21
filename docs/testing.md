# Testing and recordings

`cargo test --workspace` runs the tests of every crate. No radio is needed: a stack has a generator that makes its test signal, and the driver `synth` plays it.

## Test signals

```sh
sdrkit run fm --source synth --seconds 10      # one stack on its test signal
sdrkit verify fm                               # the checks of one stack
sdrkit verify                                  # the checks of every stack
```

`sdrkit verify` runs the test signal of each stack through the engine with a frequency offset and noise at an SNR of 20 dB (`--snr` sets another, `--clean` runs with no offset and no noise) for 5 seconds (`--seconds`). It prints a table of the checks and exits with code 1 if a check fails.

## Recordings

Recordings are not in the repository. They are in a folder beside the workspace, `../sdrkit-recordings` (or wherever `SDRKIT_RECORDINGS`
names; a relative path starts at the workspace root), laid out as `<family>/<file>`: `dvbt/dvbt_7mhz_2k_16qam_8msps.sigmf-data`,
`satimg/goes_file0.lrit`, and off-air recordings under `real/<family>/`. Each `.sigmf-data` has a copy of its `.sigmf-meta` beside
it, so the folder plays on its own (`sdrkit play ../sdrkit-recordings/fm/fm_stereo_rds_500k.sigmf-meta fm`). The repository keeps each one's `.sigmf-meta` and expected
results under `tests/vectors/<family>/`, and its entry in `tests/vectors/manifest.json`: path, size, SHA-256, origin (synthetic: the
generator, stack, settings and seed; real: where and when it was recorded) and whether it can be made again.

```sh
sdrkit vectors list dvbt         # each recording under dvbt/: present, missing or of the wrong size
sdrkit vectors verify dvbt       # sizes and checksums (fails when a recording is missing or wrong)
```

A test that reads a recording calls `sdrkit_dev_test::read_recording("<family>/<name>")` (or `sdrkit_dev_test::recording(...)` for another
file). When the recording is missing the test prints why and passes without checking: a missing recording skips a test, it never
fails one. A recording that is not regenerable (every one made by an independent test generator, and every off-air one) exists only
in the recordings folder, so keep a copy of it.

## Allocation tests and benchmarks

Some tests run a decode path under a counting allocator and fail if the path allocates in steady state: `tests/alloc.rs` of `sdrkit-algo-fec`, `tests/alloc_acquire.rs` of `sdrkit-algo-modem`, and `tests/sample_path_alloc.rs` of the driver crates.

`sdrkit bench [filter]` runs the benchmarks of the stacks and shows the processor time and the speed against real time. [benchmarks.md](benchmarks.md) describes them.
