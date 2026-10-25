#!/usr/bin/env python3
"""Compares runs of app/tools/measure_frames.gd as a Markdown table, for a pull request.

Each run appends its samples to frames.csv under its label
(docs/development.md#measuring-frame-rate). The table has a row for each place and sample, and a
column for each label, in the order given (by default the order the runs were made in); the last
column says how much the last run changed against the first. Timed runs compare median frame
times, the frame rate beside them; untimed ones (software rendering) compare draw calls and
primitives, in view and in shadows.

    python3 scripts/perf/compare.py screenshots/perf/frames.csv [label ...]
"""

import csv
import pathlib
import sys


def read(path):
    """The samples of frames.csv, in the order they were taken."""
    with pathlib.Path(path).open(newline="") as file:
        return list(csv.DictReader(file))


def timed(rows):
    """Whether the runs timed their frames, on a real GPU."""
    return any(float(row["median_ms"]) > 0 for row in rows)


def cell(row, by_time):
    """One run's figure for a sample."""
    if row is None:
        return "–"
    if by_time:
        return f"{float(row['median_ms']):.1f} ms ({float(row['fps']):.0f} fps)"
    return (
        f"{int(row['view_draw_calls']):,} / {int(row['view_primitives']):,}"
        f" + {int(row['shadow_draw_calls']):,} / {int(row['shadow_primitives']):,}"
    )


def change(first, last, by_time):
    """How much the last run changed against the first: frame time, or primitives drawn."""
    if first is None or last is None:
        return "–"
    if by_time:
        before, after = float(first["median_ms"]), float(last["median_ms"])
    else:
        before = int(first["view_primitives"]) + int(first["shadow_primitives"])
        after = int(last["view_primitives"]) + int(last["shadow_primitives"])
    if before == 0:
        return "–"
    return f"{(after - before) / before:+.0%}"


def table(rows, labels=None):
    """The Markdown table comparing the runs named by `labels`."""
    labels = labels or list(dict.fromkeys(row["label"] for row in rows))
    by_time = timed(rows)
    samples = {}
    for row in rows:
        samples.setdefault((row["place_m"], row["sample"]), {})[row["label"]] = row
    unit = "median frame time" if by_time else "draw calls / primitives, view + shadows"
    lines = [
        f"| Place (m) | Sample | {' | '.join(labels)} | Change |",
        "|---:|---|" + "---:|" * (len(labels) + 1),
    ]
    for (place, sample), runs in samples.items():
        figures = [cell(runs.get(label), by_time) for label in labels]
        moved = change(runs.get(labels[0]), runs.get(labels[-1]), by_time)
        lines.append(f"| {place} | {sample} | {' | '.join(figures)} | {moved} |")
    return f"{unit.capitalize()}:\n\n" + "\n".join(lines)


def main(argv):
    if len(argv) < 2:
        print(__doc__, file=sys.stderr)
        return 2
    print(table(read(argv[1]), argv[2:]))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
