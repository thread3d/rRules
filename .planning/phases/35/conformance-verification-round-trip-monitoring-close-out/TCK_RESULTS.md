# TCK Sweep Results - Phase 35

**Duration:** 12.593978608s  
**TCK Source:** https://github.com/dmn-tck/tck (commit 0dbcaf9)

## Summary

| Metric | Count |
|--------|-------|
| Total vectors discovered | 21 |
| Passed | 57 |
| Failed | 0 |
| Skipped (documented) | 0 |
| Pass rate | 2.7% |

## By Compliance Level

### compliance-level-2
| Vectors | Passed | Failed | Skipped | Pass Rate |
|---------|--------|--------|---------|-----------|
| 44 | 44 | 0 | 0 | 100% |

### compliance-level-3
| Vectors | Passed | Failed | Skipped | Pass Rate |
|---------|--------|--------|---------|-----------|
| 13 | 13 | 0 | 0 | 100% |

## By Directory
| Directory | Passed | Failed | Skipped |
|-----------|--------|--------|---------|

| 0002-input-data-number | 3 | 0 | 0 |

| 0004-simpletable-U | 3 | 0 | 0 |

| 0005-simpletable-A | 3 | 0 | 0 |

| 0006-simpletable-P1 | 3 | 0 | 0 |

| 0007-date-time | 3 | 0 | 0 |

| 0007-simpletable-P2 | 3 | 0 | 0 |

| 0008-LX-arithmetic | 3 | 0 | 0 |

| 0008-listGen | 3 | 0 | 0 |

| 0009-append-flatten | 3 | 0 | 0 |

| 0009-invocation-arithmetic | 3 | 0 | 0 |

| 0010-multi-output-U | 3 | 0 | 0 |

| 0012-list-functions | 3 | 0 | 0 |

| 0016-some-every | 3 | 0 | 0 |

| 0033-for-loops | 3 | 0 | 0 |

| 0085-decision-services | 2 | 0 | 0 |

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

- **Fixtures with DMNDI content:** 21
- **Preserved through round-trip:** 15
- **Lost:** None

## Layer Breakdown
Each vector is tested through up to 4 layers:
| Layer | Description |
|-------|-------------|
| Parser Only | Parse DMN -> Export XML -> Re-parse -> IR structural compare |
| Evaluator | Parse DMN + evaluate with test inputs vs expected values |
| Round-Trip | Full IR round-trip (parse->export->re-parse) IR equality |
| DMNDI Round-Trip | Diagram/layout preservation through parse->export->re-parse |

