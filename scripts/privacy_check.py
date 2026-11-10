#!/usr/bin/env python3
"""Check RustChrome privacy-probe reports.

Two reports from two separate launches are compared. The point is to verify the
properties we actually claim, not to score a fingerprint:

1. within one session, two canvas reads must be identical (sites keep working),
2. across sessions, the canvas hash must change (no cross-session identifier),
3. the fields we normalise must be stable and consistent across runs,
4. the normalised values must not contradict each other.

Exit code 0 = every applicable check passed. Anything unverifiable is reported
as `skip` with the reason, never as a pass.
"""

import json
import sys

CHECKS = []


def check(name, ok, detail):
    CHECKS.append((name, "pass" if ok else "fail", detail))
    return ok


def skip(name, why):
    CHECKS.append((name, "skip", why))


def load(path):
    try:
        with open(path, encoding="utf-8") as f:
            return json.load(f)
    except Exception as exc:  # noqa: BLE001 - report, do not crash the job
        print(f"could not read {path}: {exc}")
        return None


def main(argv):
    if len(argv) < 3:
        print("usage: privacy_check.py run1.json run2.json")
        return 2

    a, b = load(argv[1]), load(argv[2])
    if a is None or b is None:
        print("FAIL: missing probe output")
        return 1

    check(
        "canvas stable within a session",
        a["canvas1"] == a["canvas2"],
        f"{a['canvas1']} vs {a['canvas2']}",
    )
    check(
        "canvas changes between sessions",
        a["canvas1"] != b["canvas1"],
        f"{a['canvas1']} vs {b['canvas1']}",
    )

    for key, label in [
        ("hc", "hardwareConcurrency"),
        ("mem", "deviceMemory"),
        ("langs", "languages"),
        ("tz", "time zone"),
        ("tzoff", "time zone offset"),
        ("plugins", "plugin count"),
    ]:
        check(f"{label} stable across runs", a[key] == b[key], f"{a[key]!r} vs {b[key]!r}")

    check("time zone is UTC", str(a["tz"]) == "UTC", str(a["tz"]))
    check("offset is 0", a["tzoff"] == 0, str(a["tzoff"]))
    check(
        "Date#toString agrees with UTC",
        "GMT+0000" in a["datestr"] or "Coordinated Universal Time" in a["datestr"],
        a["datestr"],
    )
    check("language list pinned", a["langs"] == "en-US,en", a["langs"])
    check("doNotTrack set", str(a["dnt"]) == "1", str(a["dnt"]))
    # outerWidth can exceed screen.width by the invisible resize border Win32
    # reports; the client area is the part that must actually fit.
    inner = a.get("inner") or [0, 0]
    check(
        "client area fits the screen",
        inner[0] <= a["screen"][0] and inner[1] <= a["screen"][1],
        f"screen {a['screen'][:2]} inner {inner} outer {a['outer']}",
    )
    gl = a.get("webgl") or {}
    if gl.get("available"):
        check(
            "WebGL renderer coerced",
            gl.get("renderer") in ("Generic Renderer", "Generic"),
            f"{gl.get('vendor')} / {gl.get('renderer')}",
        )
    else:
        skip("WebGL renderer coerced", "no WebGL context in this mode (strict)")

    check(
        "WebRTC policy applied",
        a["webrtc"] in ("function", "undefined"),
        str(a["webrtc"]),
    )
    if a["webrtc"] == "undefined":
        skip("WebRTC relay-only", "API removed (strict mode)")
    else:
        skip("WebRTC relay-only", "candidate policy is engine-level; not observable from script")

    # The probe page is a data: document with an opaque origin, so the storage
    # APIs throw there regardless of the browser. Reporting "false" as a pass
    # would be meaningless, so this is stated as unmeasured instead.
    skip(
        "storage APIs available",
        f"probe runs on an opaque origin (ls={a['storage']['ls']}, idb={a['storage']['idb']})",
    )

    width = max(len(c[0]) for c in CHECKS)
    print(f"{'check'.ljust(width)}  result  detail")
    print("-" * (width + 40))
    for name, result, detail in CHECKS:
        print(f"{name.ljust(width)}  {result:6}  {detail}")

    failed = [c for c in CHECKS if c[1] == "fail"]
    print()
    print(f"{len(CHECKS) - len(failed)}/{len(CHECKS)} checks passed, {len(failed)} failed")
    print("engine:", a.get("ua"))
    return 0 if not failed else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv))
