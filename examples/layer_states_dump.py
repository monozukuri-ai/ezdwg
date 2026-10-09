#!/usr/bin/env python3
"""Dump layer states: names, structured summary, optional --diff."""

from __future__ import annotations

import sys


def main(argv: list[str]) -> int:
    if len(argv) < 2:
        print(
            f"usage: {argv[0]} <file.dwg> [--diff [STATE]]",
            file=sys.stderr,
        )
        return 2

    path = argv[1]
    diff_mode = False
    diff_state: str | None = None
    args = argv[2:]
    i = 0
    while i < len(args):
        if args[i] == "--diff":
            diff_mode = True
            if i + 1 < len(args) and not args[i + 1].startswith("-"):
                diff_state = args[i + 1]
                i += 2
            else:
                i += 1
        else:
            print(f"unknown argument: {args[i]}", file=sys.stderr)
            return 2

    from ezdwg import read, raw
    from ezdwg.layer_states import build_layer_state_table, LayerStateMasks

    print("=== Layer-state discovery ===")
    print(f"file: {path}")
    try:
        print(f"version: {raw.detect_version(path)}")
    except Exception as e:
        print(f"version: ? ({e})")

    names = raw.decode_layer_state_names(path)
    print(f"\n--- names ({len(names)}) ---")
    for n in names:
        print(f"  state: {n!r}")

    table = build_layer_state_table(path)
    stats = table.resolution_stats()
    print("\n--- structured ---")
    print(f"states: {stats['states']}")
    print(
        f"entries: {stats['entries']} "
        f"(named={stats['named']}, unnamed={stats['unnamed']})"
    )
    for st in table:
        bits = [b.name for b in LayerStateMasks if b and (st.mask & b)]
        print(f"  {st.name!r}")
        print(
            f"    handle={st.handle:#x} mask={int(st.mask)} ({', '.join(bits) or '—'})"
        )
        print(f"    description={st.description!r}")
        rs = st.resolution_stats()
        print(
            f"    entries={rs['entries']} "
            f"(named={rs['named']}, unnamed={rs['unnamed']})"
        )
        for e in st.entries[:5]:
            print(
                f"      name={e.name!r} handle={e.layer_handle!r} "
                f"flags={e.flags} color={e.color} lw={e.lineweight_index} "
                f"on={e.on} plot={e.plot} plot_style={e.plot_style!r}"
            )
        if len(st.entries) > 5:
            print(f"      ... +{len(st.entries) - 5} more")

    if diff_mode:
        doc = read(path)
        targets = [table.get(diff_state)] if diff_state else list(table)
        targets = [t for t in targets if t is not None]
        if diff_state and not targets:
            print(f"\n--- diff: state {diff_state!r} not found ---", file=sys.stderr)
            return 1
        print("\n--- diff vs live layers ---")
        for st in targets:
            d = st.diff(doc)
            print(f"  {st.name!r} summary={d.summary}")
            changed = [e for e in d.entries if e.status == "changed"]
            for e in changed[:10]:
                print(f"    {e.layer_name}: {e.changes}")
            if len(changed) > 10:
                print(f"    ... +{len(changed) - 10} more changed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
