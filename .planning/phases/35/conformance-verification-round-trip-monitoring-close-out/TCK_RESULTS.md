# TCK Sweep Results - Phase 35

**Duration:** 2.029335715s  
**TCK Source:** https://github.com/dmn-tck/tck (commit 0dbcaf9)

## Summary

| Metric | Count |
|--------|-------|
| Total vectors discovered | 51 |
| Passed | 123 |
| Failed | 0 |
| Skipped (documented) | 0 |
| Vectors with at least one pass | 51 / 51 | 100% |


## By Compliance Level

### compliance-level-2
| Vectors | Passed | Failed | Skipped | Pass Rate |
|---------|--------|--------|---------|-----------|
| 110 | 110 | 0 | 0 | 100% |

### compliance-level-3
| Vectors | Passed | Failed | Skipped | Pass Rate |
|---------|--------|--------|---------|-----------|
| 13 | 13 | 0 | 0 | 100% |

## By Directory
| Directory | Passed | Failed | Skipped |
|-----------|--------|--------|---------|

| 0001-filter | 3 | 0 | 0 |

| 0002-input-data-number | 3 | 0 | 0 |

| 0002-string-functions | 3 | 0 | 0 |

| 0004-simpletable-U | 3 | 0 | 0 |

| 0005-simpletable-A | 3 | 0 | 0 |

| 0006-simpletable-P1 | 3 | 0 | 0 |

| 0007-date-time | 3 | 0 | 0 |

| 0007-simpletable-P2 | 3 | 0 | 0 |

| 0008-LX-arithmetic | 3 | 0 | 0 |

| 0008-listGen | 3 | 0 | 0 |

| 0009-append-flatten | 3 | 0 | 0 |

| 0009-invocation-arithmetic | 3 | 0 | 0 |

| 0010-concatenate | 3 | 0 | 0 |

| 0010-multi-output-U | 3 | 0 | 0 |

| 0011-insert-remove | 3 | 0 | 0 |

| 0012-list-functions | 3 | 0 | 0 |

| 0013-sort | 3 | 0 | 0 |

| 0016-some-every | 3 | 0 | 0 |

| 0021-singleton-list | 3 | 0 | 0 |

| 0033-for-loops | 3 | 0 | 0 |

| 0050-feel-abs-function | 2 | 0 | 0 |

| 0051-feel-sqrt-function | 2 | 0 | 0 |

| 0052-feel-exp-function | 2 | 0 | 0 |

| 0053-feel-log-function | 2 | 0 | 0 |

| 0054-feel-even-function | 2 | 0 | 0 |

| 0055-feel-odd-function | 2 | 0 | 0 |

| 0056-feel-modulo-function | 2 | 0 | 0 |

| 0057-feel-context | 2 | 0 | 0 |

| 0058-feel-number-function | 2 | 0 | 0 |

| 0059-feel-all-function | 2 | 0 | 0 |

| 0060-feel-any-function | 2 | 0 | 0 |

| 0061-feel-median-function | 2 | 0 | 0 |

| 0062-feel-mode-function | 2 | 0 | 0 |

| 0063-feel-stddev-function | 2 | 0 | 0 |

| 0067-feel-split-function | 2 | 0 | 0 |

| 0068-feel-equality | 2 | 0 | 0 |

| 0069-feel-list | 2 | 0 | 0 |

| 0071-feel-between | 2 | 0 | 0 |

| 0080-feel-getvalue-function | 2 | 0 | 0 |

| 0081-feel-getentries-function | 2 | 0 | 0 |

| 0085-decision-services | 2 | 0 | 0 |

| 0094-feel-product-function | 2 | 0 | 0 |

| 0096-feel-day-of-week-function | 2 | 0 | 0 |

| 0097-feel-month-of-year-function | 2 | 0 | 0 |

| 0098-feel-week-of-year-function | 2 | 0 | 0 |

| 0100-arithmetic | 2 | 0 | 0 |

| 1100-feel-decimal-function | 3 | 0 | 0 |

| 1141 | 2 | 0 | 0 |

| 1142 | 2 | 0 | 0 |

| 1143 | 2 | 0 | 0 |

| 1144 | 2 | 0 | 0 |

## Known Divergences
| Vector | Category | Expected | Actual | Root Cause | Spec Reference |
|--------|----------|----------|--------|------------|----------------|

| DMN15-31-collect-distinct-sum | compliance-level-3/0114-collect-aggregators | COLLECT with distinct + SUM aggregator on grouped data | FEEL 1.6 feature not in Level 3 spec; COLLECT with distinct is Level 5 | COLLECT with distinct parameter is a DMN 1.6 feature (FEEL 1.6), outside Level 3 scope | DMN 1.6 section 8.2.12, FEEL 1.6 specification |

| sum-empty-list | compliance-level-3/0100-arithmetic | null (per DMN spec, SUM of empty list returns null) | Decimal(0) - Rust Decimal default for empty iterator sum | Empty iterator sum defaults to zero in rust_decimal; spec requires null for empty SUM/MIN/MAX | DMN 1.5 section 11.3.4 (numeric built-in functions) |

## Round-Trip Fidelity (DMNDI)

- **Fixtures with DMNDI content:** 50
- **Preserved through round-trip:** 21
- **Lost:** None

## Layer Breakdown
Each vector is tested through up to 4 layers:
| Layer | Description |
|-------|-------------|
| Parser Only | Parse DMN -> Export XML -> Re-parse -> IR structural compare |
| Evaluator | Parse DMN + evaluate with test inputs vs expected values |
| Round-Trip | Full IR round-trip (parse->export->re-parse) IR equality |
| DMNDI Round-Trip | Diagram/layout preservation through parse->export->re-parse |

