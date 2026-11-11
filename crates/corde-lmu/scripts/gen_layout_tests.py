#!/usr/bin/env python3
"""Generate crates/corde-lmu/tests/layout_offsets.rs from reference/lmu-shared-memory-layout.txt.

Every direct member of every struct in the reference file becomes an
`offset_of!` assertion; nested members of struct-typed fields are covered by
the assertions of that struct (plus the assertion on the field itself).

Usage (from the repository root), then run `cargo fmt -p corde-lmu`:

    python3 -I crates/corde-lmu/scripts/gen_layout_tests.py \
        reference/lmu-shared-memory-layout.txt \
        crates/corde-lmu/tests/layout_offsets.rs
"""
import re
import sys
from pathlib import Path

REF = Path(sys.argv[1])
OUT = Path(sys.argv[2])

# Known struct sizes (used to skip the nested expansion that follows a
# struct-typed field in the reference file).
SIZES = {}
EXCEPTIONS = {
    "mSoC": "soc",
    "mID": "id",
    "FFBTorque": "ffb_torque",
}
# C++ struct → Rust type; None = no Rust equivalent (skipped)
RUST_TYPES = {
    "TelemVect3": "TelemVect3",
    "TelemWheelV01": "TelemWheelV01",
    "TelemInfoV01": "TelemInfoV01",
    "VehicleScoringInfoV01": "VehicleScoringInfoV01",
    "ScoringInfoV01": "ScoringInfoV01",
    "ApplicationStateV01": "ApplicationStateV01",
    "SharedMemoryGeneric": "SharedMemoryGeneric",
    "SharedMemoryPathData": "SharedMemoryPathData",
    "SharedMemoryScoringData": "SharedMemoryScoringData",
    "SharedMemoryTelemetryData": "SharedMemoryTelemetryData",
    "SharedMemoryObjectOut": "SharedMemoryObjectOut",
    "SharedMemoryLayout": None,
}


def snake(name: str) -> str:
    if name in EXCEPTIONS:
        return EXCEPTIONS[name]
    if name.startswith("m") and len(name) > 1 and name[1].isupper():
        name = name[1:]
    out = []
    for i, c in enumerate(name):
        if c.isupper():
            prev = name[i - 1] if i > 0 else ""
            nxt = name[i + 1] if i + 1 < len(name) else ""
            if prev and (prev.islower() or prev.isdigit() or (prev.isupper() and nxt.islower())):
                out.append("_")
            out.append(c.lower())
        else:
            out.append(c)
    return "".join(out)


blocks = []
cur = None
for line in REF.read_text().splitlines():
    m = re.match(r"^===== (\w+) sizeof=(\d+) align=(\d+)", line)
    if m:
        cur = {"name": m.group(1), "size": int(m.group(2)), "align": int(m.group(3)), "members": []}
        SIZES[cur["name"]] = cur["size"]
        blocks.append(cur)
        continue
    m = re.match(r"^\s*(\d+)\s+(.+?)\s+(\S+)$", line)
    if m and cur is not None:
        cur["members"].append((int(m.group(1)), m.group(2), m.group(3)))

lines = []
lines.append("//! Exhaustive size / offset tests of `corde_lmu::layout`, GENERATED from")
lines.append("//! `reference/lmu-shared-memory-layout.txt` by")
lines.append("//! `crates/corde-lmu/scripts/gen_layout_tests.py` — do not edit by hand, regenerate.")
lines.append("//!")
lines.append("//! Every direct member of every structure of the reference dump is checked;")
lines.append("//! nested members (e.g. `mPos.x`) are covered by the tests of their own type.")
lines.append("")
lines.append("use corde_lmu::layout::*;")
lines.append("use std::mem::{align_of, offset_of, size_of};")
lines.append("")

total = 0
for b in blocks:
    rust = RUST_TYPES.get(b["name"])
    if rust is None:
        continue
    fn = re.sub(r"(?<!^)(?=[A-Z])", "_", b["name"]).lower()
    lines.append("#[test]")
    lines.append(f"fn {fn}() {{")
    lines.append(f"    assert_eq!(size_of::<{rust}>(), {b['size']}, \"sizeof({b['name']})\");")
    # the pack(4) structs are declared packed(4): alignment 4 unless smaller
    if b["name"] in ("TelemVect3", "TelemWheelV01", "TelemInfoV01", "VehicleScoringInfoV01", "ScoringInfoV01", "ApplicationStateV01"):
        lines.append(f"    assert_eq!(align_of::<{rust}>(), {b['align']}, \"alignof({b['name']})\");")
    else:
        lines.append(f"    assert_eq!(align_of::<{rust}>(), {b['align']}, \"alignof({b['name']})\");")
    skip_until = -1
    for off, ctype, name in b["members"]:
        if off < skip_until:
            continue
        if "(anonymous" in ctype or "(anonymous" in name:
            continue
        if b["name"] == "TelemVect3" and name == "data":
            continue  # union alias of x/y/z
        field = snake(name)
        m = re.match(r"^struct (\w+)$", ctype)
        if m and m.group(1) in SIZES:
            skip_until = off + SIZES[m.group(1)]
        lines.append(f"    assert_eq!(offset_of!({rust}, {field}), {off}, \"{b['name']}::{name}\");")
        total += 1
    lines.append("}")
    lines.append("")

lines.append("#[test]")
lines.append("fn whole_block_constants() {")
lines.append("    assert_eq!(size_of::<SharedMemoryObjectOut>(), SHM_TOTAL_SIZE);")
lines.append("    assert_eq!(SHM_TOTAL_SIZE - 4, SHM_USEFUL_SIZE);")
lines.append("    assert_eq!(OFF_TELEM_INFO + MAX_VEHICLES * SIZE_TELEM_INFO, SHM_USEFUL_SIZE);")
lines.append("    assert_eq!(OFF_SCORING_STREAM + SCORING_STREAM_LEN, OFF_TELEMETRY);")
lines.append("    assert_eq!(OFF_CURRENT_ET, 1700);")
lines.append("    assert_eq!(telem_elapsed_time_offset(0), 128_480);")
lines.append("    assert_eq!(telem_elapsed_time_offset(103), 128_468 + 103 * 1888 + 12);")
lines.append("}")
lines.append("")

OUT.write_text("\n".join(lines))
print(f"{total} offset assertions written to {OUT}")
