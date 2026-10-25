#!/usr/bin/env bash
# Regenerate the failure ledger inside TCK-COVERAGE-GAPS.md.
#
# The block between the `BEGIN/END GENERATED LEDGER` markers in that file is
# owned by the harness: this script runs the TCK sweep with `TCK_LEDGER_OUT`
# pointing at a temporary file and splices the result into the document. Run it
# whenever the conformance state changes — closing a gap, or accepting one.
#
# Usage: make tck-ledger   (or ./scripts/update-tck-ledger.sh)
set -euo pipefail

cd "$(dirname "$0")/.."

ledger=$(mktemp)
trap 'rm -f "$ledger"' EXIT

echo "Running the TCK sweep..."
TCK_LEDGER_OUT="$ledger" cargo test -q -p engine --test tck_sweep -- tck_sweep --nocapture \
    | grep -E 'Baseline met|TCK regression|Ledger written' || true

if [ ! -s "$ledger" ]; then
    echo "ERROR: the sweep produced no ledger — it probably failed before reporting." >&2
    exit 1
fi

python3 - "$ledger" <<'PY'
import pathlib, sys

ledger = pathlib.Path(sys.argv[1]).read_text().strip("\n")
document_path = pathlib.Path("TCK-COVERAGE-GAPS.md")
document = document_path.read_text()

begin = "<!-- BEGIN GENERATED LEDGER -->"
end = "<!-- END GENERATED LEDGER -->"
try:
    start = document.index(begin) + len(begin)
    stop = document.index(end)
except ValueError:
    sys.exit(f"ERROR: {document_path} is missing the {begin!r} / {end!r} markers")

document_path.write_text(f"{document[:start]}\n{ledger}\n{document[stop:]}")
print(f"Ledger updated in {document_path} ({len(ledger.splitlines())} lines).")
PY
