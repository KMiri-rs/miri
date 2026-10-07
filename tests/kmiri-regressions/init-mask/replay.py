#!/usr/bin/env python3
"""Run each probe under SB/TB; preserve raw interpreter failures and outputs."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess

HERE = Path(__file__).resolve().parent
CASES = {'global_uninit': 'ub', 'direct_uninit': 'ub', 'copy_initialized': 'pass', 'copy_uninitialized': 'ub', 'copy_uninitialized_no_read': 'pass', 'partial_valid': 'pass', 'partial_invalid': 'ub'}

def main():
    p = argparse.ArgumentParser()
    p.add_argument("--miri", required=True)
    p.add_argument("--sysroot", required=True)
    p.add_argument("--target", default="x86_64-unknown-linux-gnu")
    p.add_argument("--output", required=True)
    p.add_argument("--case", choices=list(CASES))
    a = p.parse_args()
    out = Path(a.output).resolve()
    out.mkdir(parents=True, exist_ok=False)
    binary = Path(a.miri).resolve()
    source = HERE / "probe.rs"
    rows = []
    cases = {a.case: CASES[a.case]} if a.case else CASES
    for model in ["stacked", "tree"]:
        for case, expected in cases.items():
            command = [str(binary), "--edition=2021", "--cfg", "miri",
                       "--cfg", f'probe="{case}"', "--target", a.target,
                       "--sysroot", str(Path(a.sysroot).resolve()),
                       "-C", "panic=abort", "-Zmiri-backtrace=full", str(source)]
            if model == "tree":
                command.append("-Zmiri-tree-borrows")
            if (HERE / "pages.toml").exists():
                command.append("-Zkmiri-toml=" + str(HERE / "pages.toml"))
            try:
                r = subprocess.run(command, capture_output=True, timeout=600)
                stdout, stderr, rc = r.stdout, r.stderr, r.returncode
            except subprocess.TimeoutExpired as e:
                stdout, stderr, rc = e.stdout or b"", e.stderr or b"", 124
            text = (stdout + stderr).decode(errors="replace")
            if rc == 0 and "G0_DONE" in text:
                actual = "pass"
            elif rc == 1 and "Undefined Behavior" in text and "uninitialized" in text:
                actual = "ub"
            elif rc == 1 and "unsupported operation" in text and "untyped physical copy" in text:
                actual = "unsupported"
            else:
                actual = "unexpected"
            stem = out / (model + "-" + case)
            stem.with_suffix(".stdout").write_bytes(stdout)
            stem.with_suffix(".stderr").write_bytes(stderr)
            stem.with_suffix(".rc").write_text(str(rc) + "\n")
            row = dict(model=model, case=case, expected=expected, actual=actual,
                       accepted=actual == expected, rc=rc, command=command,
                       binary_sha256=hashlib.sha256(binary.read_bytes()).hexdigest(),
                       source_sha256=hashlib.sha256(source.read_bytes()).hexdigest())
            rows.append(row)
            print(json.dumps(row), flush=True)
    accepted = all(r["accepted"] for r in rows)
    (out / "results.json").write_text(json.dumps(dict(accepted=accepted, results=rows), indent=2))
    return 0 if accepted else 1

if __name__ == "__main__":
    raise SystemExit(main())
