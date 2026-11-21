# Benchmarks

The hand-written vector kernels for desktop processors (SSE2, SSSE3 and AVX2 in `sdrkit-algo-fec`: Viterbi and Reed-Solomon) have only
been built, never run, on the Mac they were written on. On an x86-64 PC (Linux or Windows, with AVX2 if possible), from the
workspace root, run:

```sh
cargo test --release -p sdrkit-algo-fec -p sdrkit-algo-dsp -p sdrkit-algo-modem -p sdrkit-util-bits -p sdrkit-components && cargo run --profile dist -p sdrkit-host-cli -- bench --all --json > bench-pc.json
```

(`--profile dist` is the whole-program-optimised build that ships, so the numbers are the published ones; it takes a while to
build.) The tests compare every vector kernel with its scalar twin, bit for bit, on the kernels the machine has (each kernel checks the CPU
before it runs), and the error-correction tests decode known frames with them. The benchmark run prints the machine (`machine:`
line without `--json`: OS, architecture, cores and the vector kernels found) and one row per benchmark. Send back `bench-pc.json`
and the test output.

What each number means:

- **CPU (one core, per s of signal)**: the share of one core the benchmark needs to keep up with one second of its signal at the
  rate in its name (`_2400k` is 2.4 Msps, `_20m` is 20 Msps, `_1500` is a 1,500-byte frame). Below 100 % it runs in real time;
  the lower, the more room is left for everything else.
- **Real time**: the same as a speed, `100 / CPU`: `40x` means forty such streams fit on one core. Below `1x` the row says too
  slow.
- **Items/s**: samples, bits, symbols or bytes handled per second of CPU time.
- **Memory**: the most memory the benchmark held at once above what was in use before it (a hot path allocates nothing once it
  runs; this is its buffers).

The targets each must beat on a PC (a phone is the harder case; these are the PC numbers that leave it room):

| Benchmarks | Target |
|---|---|
| `fec.viterbi_k7`, `fec.viterbi_k7_stream` | about 2 ns per decoded bit: DVB-T at 31.7 Mbit/s under 10 % of a core |
| `fec.rs_204_188`, `fec.rs_255_223` | DVB-T's 31.7 Mbit/s of packets under 5 %; CCSDS frames far below |
| `fec.ldpc_dvbs2_short_1_2`, `fec.turbo_lte_6144` | under 100 % at the code's own rate in its name; LDPC and turbo are the heaviest decoders and may use the helper threads |
| `bits.*` (`crc16_1500`, `crc32_1500`, `descramble_*`, `hdlc_per_bit`, `sync_per_bit`) | under 1 % at 10 Mbit/s |
| `dsp.*` | under 5 % each at the rate in the name (the front end runs several in a row at 2.4 Msps) |
| `modem.*` | under 25 % at the rate in the name (`_20m` rows are 802.11 and DVB at their full rate) |
| `sim.*` | under 50 %: only test signals use them |
| `graph.*`, `engine.*` | the AM stack and the front end at 2.4 Msps under 5 %; the mixer and spectrum under 1 % |
| `stack.<id>` | each stack on its own test signal, at the stack's rate: under 25 % (FM broadcast under 5 %, ARCHITECTURE.md) |
