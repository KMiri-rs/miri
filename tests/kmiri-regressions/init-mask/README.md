# KMiri init-mask regression reproduction

This directory contains a standalone non-test MIR entry, a parameterized runner,
and recorded evidence. Both SB/TB are used. No checks are disabled.

## Run with a matching binary and MIR sysroot

```bash
python3 tests/kmiri-regressions/init-mask/replay.py \
  --miri /absolute/path/to/miri \
  --sysroot /absolute/path/to/matching-mir-sysroot \
  --target x86_64-unknown-linux-gnu \
  --output /absolute/path/to/new-results
```

The binary and sysroot must match the compiler recorded by the workspace.
The default target is x86_64; the earlier aggregate G0 suite also tested RV64.
`--case NAME` narrows the test while retaining both borrow models.
All output directories must be fresh. Each raw interpreter exit is preserved;
the launcher's exit is 0 only if every case has its expected classification.

## Exact commands used in the verified Docker environment

```bash
docker exec -it blueos-miri-20261001 bash
export SIDE=/blueos/experiments/side-g0-publication-20261005
export TC=/blueos/experiments/roadmap-20261003/legacy-pair/toolchain
export LD_LIBRARY_PATH="$TC/lib:$TC/lib/rustlib/x86_64-unknown-linux-gnu/lib"
export SYSROOT=/blueos/experiments/roadmap-20261003/legacy-pair/cache/miri
export RESULTS="$(mktemp -d)"
python3 "$SIDE/drafts/init-mask/replay.py" \
  --miri "$SIDE/validation-v1/build-baseline/miri" \
  --sysroot "$SYSROOT" --output "$RESULTS/baseline"
python3 "$SIDE/drafts/init-mask/replay.py" \
  --miri "$SIDE/validation-v1/build-init-mask/miri" \
  --sysroot "$SYSROOT" --output "$RESULTS/modified"
python3 "$SIDE/drafts/init-mask/replay.py" \
  --miri "$SIDE/validation-v1/build-rollback-init-mask/miri" \
  --sysroot "$SYSROOT" --output "$RESULTS/rollback"
```

## Recorded evidence and attribution

The original and fixed candidates were independently compiled at base `7a7637adb5ff7fa4a187b6166974d1297e0ad1f7`
with compiler `390279b302ca98ae270f434100ae3730531d1246`.
The actual rollback was rebuilt and matches original outcomes and source hashes.
`recorded-g0-results.json` is the additional replay of the original larger G0 probe
on both recorded toolchains; its paths/hashes intentionally identify that probe,
not this reduced source.
`logs/` contains raw reduced-probe results and diagnostics.

This is a checker regression, not a newly discovered BlueOS/Tock kernel UB.
No real kernel path is claimed to have triggered it.
