## Built package, overlap rule alone, 100 to 1000 fields, three alternating rounds
files: timing-dist-base-round1.json, timing-dist-base-round2.json, timing-dist-base-round3.json, timing-dist-head-round1.json, timing-dist-head-round2.json, timing-dist-head-round3.json
| case | fields | base median ms | base min ms | head median ms | head min ms | ratio of medians | ratio of minima | samples each |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| no-arguments | 100 | 0.7 | 0.6 | 22.9 | 17.4 | 34.97 | 29.95 | 15, 15 |
| no-arguments | 200 | 2.6 | 2.3 | 81.2 | 68.4 | 31.59 | 29.68 | 15, 15 |
| no-arguments | 300 | 5.8 | 5.1 | 191.6 | 164.4 | 32.99 | 32.32 | 15, 15 |
| no-arguments | 1000 | 60.7 | 56.1 | 2373.2 | 1983.8 | 39.10 | 35.38 | 9, 9 |
| one-scalar | 100 | 17.9 | 16.1 | 50.6 | 37.7 | 2.82 | 2.34 | 15, 15 |
| one-scalar | 200 | 68.3 | 64.6 | 191.4 | 156.2 | 2.80 | 2.42 | 15, 15 |
| one-scalar | 300 | 160.5 | 146.9 | 402.7 | 383.3 | 2.51 | 2.61 | 15, 15 |
| one-scalar | 1000 | 1850.3 | 1696.2 | 4490.8 | 4204.4 | 2.43 | 2.48 | 9, 9 |
| two-scalars | 100 | 32.8 | 30.8 | 57.7 | 51.7 | 1.76 | 1.68 | 15, 15 |
| two-scalars | 200 | 139.9 | 125.4 | 227.0 | 208.7 | 1.62 | 1.67 | 15, 15 |
| two-scalars | 300 | 308.4 | 282.2 | 492.4 | 471.8 | 1.60 | 1.67 | 15, 15 |
| two-scalars | 1000 | 3441.0 | 3291.6 | 5885.8 | 5485.0 | 1.71 | 1.67 | 9, 9 |
| nested-input-object | 100 | 138.4 | 127.4 | 167.2 | 149.0 | 1.21 | 1.17 | 15, 15 |
| nested-input-object | 200 | 586.1 | 530.9 | 646.6 | 612.2 | 1.10 | 1.15 | 15, 15 |
| nested-input-object | 300 | 1324.4 | 1199.8 | 1443.3 | 1383.2 | 1.09 | 1.15 | 15, 15 |
| nested-input-object | 1000 | 14470.7 | 13102.9 | 16977.8 | 15691.7 | 1.17 | 1.20 | 9, 9 |

Growth: exponent k in time ~ fields^k, from the lowest samples at the smallest and largest sizes run.
| case | base | head |
| --- | --- | --- |
| no-arguments | 1.98 (100 to 1000) | 2.06 (100 to 1000) |
| one-scalar | 2.02 (100 to 1000) | 2.05 (100 to 1000) |
| two-scalars | 2.03 (100 to 1000) | 2.03 (100 to 1000) |
| nested-input-object | 2.01 (100 to 1000) | 2.02 (100 to 1000) |

## Built package, overlap rule alone, 2000 fields
files: timing-dist-base-2000.json, timing-dist-head-2000.json
| case | fields | base median ms | base min ms | head median ms | head min ms | ratio of medians | ratio of minima | samples each |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| no-arguments | 2000 | 238.7 | 233.6 | 8372.9 | 7774.3 | 35.08 | 33.29 | 3, 3 |
| one-scalar | 2000 | 7030.6 | 6945.4 | 18888.7 | 17763.3 | 2.69 | 2.56 | 3, 3 |
| two-scalars | 2000 | 14071.8 | 13539.7 | 23668.1 | 22556.5 | 1.68 | 1.67 | 3, 3 |
| nested-input-object | 2000 | 60463.7 | 59897.9 | 66654.4 | 64027.2 | 1.10 | 1.07 | 3, 3 |

Growth: exponent k in time ~ fields^k, from the lowest samples at the smallest and largest sizes run.
| case | base | head |
| --- | --- | --- |

## Built package, all default rules
files: timing-dist-base-allrules.json, timing-dist-head-allrules.json
| case | fields | base median ms | base min ms | head median ms | head min ms | ratio of medians | ratio of minima | samples each |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| no-arguments | 300 | 8.1 | 7.2 | 168.9 | 167.4 | 20.79 | 23.38 | 5, 5 |
| no-arguments | 1000 | 79.3 | 72.1 | 1947.2 | 1879.6 | 24.54 | 26.06 | 3, 3 |
| one-scalar | 300 | 209.3 | 187.4 | 364.4 | 352.6 | 1.74 | 1.88 | 5, 5 |
| one-scalar | 1000 | 1744.2 | 1726.1 | 4113.6 | 4078.5 | 2.36 | 2.36 | 3, 3 |
| two-scalars | 300 | 299.0 | 296.1 | 477.6 | 467.0 | 1.60 | 1.58 | 5, 5 |
| two-scalars | 1000 | 3334.2 | 3309.8 | 5423.0 | 5351.4 | 1.63 | 1.62 | 3, 3 |
| nested-input-object | 300 | 1184.4 | 1169.8 | 1415.7 | 1393.1 | 1.20 | 1.19 | 5, 5 |
| nested-input-object | 1000 | 13405.3 | 13247.0 | 16396.5 | 16143.2 | 1.22 | 1.22 | 3, 3 |

Growth: exponent k in time ~ fields^k, from the lowest samples at the smallest and largest sizes run.
| case | base | head |
| --- | --- | --- |
| no-arguments | 1.92 (300 to 1000) | 2.01 (300 to 1000) |
| one-scalar | 1.84 (300 to 1000) | 2.03 (300 to 1000) |
| two-scalars | 2.00 (300 to 1000) | 2.03 (300 to 1000) |
| nested-input-object | 2.02 (300 to 1000) | 2.03 (300 to 1000) |

## Built package, overlap rule alone, NODE_ENV=production
files: timing-dist-base-production.json, timing-dist-head-production.json
| case | fields | base median ms | base min ms | head median ms | head min ms | ratio of medians | ratio of minima | samples each |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| no-arguments | 300 | 1.5 | 1.2 | 177.0 | 171.2 | 118.56 | 146.42 | 5, 5 |
| no-arguments | 1000 | 15.0 | 12.6 | 1840.9 | 1812.8 | 122.95 | 143.53 | 3, 3 |
| one-scalar | 300 | 151.3 | 147.3 | 392.0 | 372.6 | 2.59 | 2.53 | 5, 5 |
| one-scalar | 1000 | 1802.5 | 1791.6 | 4217.2 | 4215.1 | 2.34 | 2.35 | 3, 3 |
| two-scalars | 300 | 316.4 | 307.0 | 501.7 | 498.2 | 1.59 | 1.62 | 5, 5 |
| two-scalars | 1000 | 3627.6 | 3500.9 | 5449.7 | 5426.9 | 1.50 | 1.55 | 3, 3 |
| nested-input-object | 300 | 1266.6 | 1241.7 | 1509.6 | 1457.8 | 1.19 | 1.17 | 5, 5 |
| nested-input-object | 1000 | 13968.5 | 13503.7 | 16874.1 | 16382.2 | 1.21 | 1.21 | 3, 3 |

Growth: exponent k in time ~ fields^k, from the lowest samples at the smallest and largest sizes run.
| case | base | head |
| --- | --- | --- |
| no-arguments | 1.98 (300 to 1000) | 1.96 (300 to 1000) |
| one-scalar | 2.08 (300 to 1000) | 2.01 (300 to 1000) |
| two-scalars | 2.02 (300 to 1000) | 1.98 (300 to 1000) |
| nested-input-object | 1.98 (300 to 1000) | 2.01 (300 to 1000) |

## Source through the project's test loader, overlap rule alone
files: timing-src-base.json, timing-src-head.json
| case | fields | base median ms | base min ms | head median ms | head min ms | ratio of medians | ratio of minima | samples each |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| no-arguments | 100 | 1.3 | 1.1 | 30.0 | 28.1 | 22.57 | 24.54 | 5, 5 |
| no-arguments | 200 | 6.5 | 3.5 | 114.0 | 112.9 | 17.56 | 31.91 | 5, 5 |
| no-arguments | 300 | 7.7 | 6.9 | 248.4 | 244.1 | 32.08 | 35.62 | 5, 5 |
| no-arguments | 1000 | 73.9 | 72.8 | 2872.4 | 2805.2 | 38.87 | 38.52 | 3, 3 |
| one-scalar | 100 | 36.0 | 34.4 | 110.2 | 109.5 | 3.06 | 3.19 | 5, 5 |
| one-scalar | 200 | 139.5 | 136.2 | 449.7 | 440.3 | 3.22 | 3.23 | 5, 5 |
| one-scalar | 300 | 325.7 | 313.5 | 1135.4 | 1023.5 | 3.49 | 3.26 | 5, 5 |
| one-scalar | 1000 | 3559.3 | 3509.9 | 11916.5 | 11414.9 | 3.35 | 3.25 | 3, 3 |
| two-scalars | 100 | 71.6 | 69.6 | 185.1 | 184.8 | 2.59 | 2.66 | 5, 5 |
| two-scalars | 200 | 277.3 | 261.7 | 739.2 | 734.1 | 2.67 | 2.81 | 5, 5 |
| two-scalars | 300 | 637.9 | 612.0 | 1712.1 | 1675.0 | 2.68 | 2.74 | 5, 5 |
| two-scalars | 1000 | 7249.3 | 6925.8 | 19192.8 | 18938.5 | 2.65 | 2.73 | 3, 3 |
| nested-input-object | 100 | 557.4 | 551.2 | 726.0 | 697.8 | 1.30 | 1.27 | 5, 5 |
| nested-input-object | 200 | 2322.4 | 2277.4 | 2824.4 | 2768.4 | 1.22 | 1.22 | 5, 5 |
| nested-input-object | 300 | 5225.1 | 5138.5 | 6041.2 | 6006.2 | 1.16 | 1.17 | 5, 5 |
| nested-input-object | 1000 | 58067.2 | 55632.4 | 67204.9 | 65679.4 | 1.16 | 1.18 | 3, 3 |

Growth: exponent k in time ~ fields^k, from the lowest samples at the smallest and largest sizes run.
| case | base | head |
| --- | --- | --- |
| no-arguments | 1.80 (100 to 1000) | 2.00 (100 to 1000) |
| one-scalar | 2.01 (100 to 1000) | 2.02 (100 to 1000) |
| two-scalars | 2.00 (100 to 1000) | 2.01 (100 to 1000) |
| nested-input-object | 2.00 (100 to 1000) | 1.97 (100 to 1000) |

## Source through the project's test loader, no arguments, 2000 fields
files: timing-src-base-noargs-2000.json, timing-src-head-noargs-2000.json
| case | fields | base median ms | base min ms | head median ms | head min ms | ratio of medians | ratio of minima | samples each |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| no-arguments | 2000 | 250.5 | 237.0 | 11314.5 | 10936.1 | 45.17 | 46.14 | 3, 3 |

Growth: exponent k in time ~ fields^k, from the lowest samples at the smallest and largest sizes run.
| case | base | head |
| --- | --- | --- |

## Fix variants, built package, overlap rule alone
Median ms (lowest sample ms).
| case | fields | base | head | fix-fastpath | fix-with-arguments-only | fix-both | fix-maintainers-pr-3967 | fix-memo |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| no-arguments | 300 | 6.0 (5.1) | 159.6 (152.8) | 5.7 (5.6) | 163.7 (155.6) | 7.2 (6.3) | 4.9 (4.7) | 5.9 (5.4) |
| no-arguments | 1000 | 56.4 (54.7) | 1816.1 (1753.5) | 62.8 (61.1) | 1871.7 (1789.2) | 80.4 (70.9) | 54.1 (52.5) | 65.3 (61.8) |
| one-scalar | 300 | 156.4 (149.2) | 353.8 (341.9) | 409.1 (346.5) | 161.4 (153.8) | 162.5 (152.9) | 159.8 (152.0) | 6.7 (5.8) |
| one-scalar | 1000 | 1763.5 (1699.1) | 4017.0 (3959.0) | 4234.2 (4053.1) | 1896.8 (1766.3) | 1822.1 (1743.7) | 1804.6 (1777.1) | 67.6 (64.6) |
| two-scalars | 300 | 304.6 (284.8) | 468.8 (458.6) | 490.5 (469.3) | 321.3 (289.5) | 316.6 (304.0) | 312.2 (298.9) | 6.4 (6.1) |
| two-scalars | 1000 | 3447.9 (3227.5) | 5265.2 (5183.8) | 5469.5 (5422.5) | 3373.0 (3313.0) | 3532.1 (3400.3) | 3503.9 (3423.0) | 67.4 (61.4) |
| nested-input-object | 300 | 1226.2 (1190.2) | 1387.8 (1365.4) | 1445.6 (1397.4) | 1228.7 (1188.9) | 1233.5 (1190.0) | 1232.5 (1192.0) | 8.5 (7.1) |
| nested-input-object | 1000 | 13582.7 (13269.4) | 15672.9 (15372.3) | 15939.0 (15681.6) | 13761.4 (13596.7) | 14257.3 (13918.6) | 13865.3 (13212.7) | 73.1 (67.8) |

## Time for one pair, built package, overlap rule alone, three rounds
Microseconds for one pair of selections: the median time divided by fields * (fields - 1) / 2.
| case | fields | base | head | added at head |
| --- | --- | --- | --- | --- |
| no-arguments | 100 | 0.13 | 4.62 | 4.49 |
| no-arguments | 200 | 0.13 | 4.08 | 3.95 |
| no-arguments | 300 | 0.13 | 4.27 | 4.14 |
| no-arguments | 1000 | 0.12 | 4.75 | 4.63 |
| one-scalar | 100 | 3.62 | 10.21 | 6.59 |
| one-scalar | 200 | 3.43 | 9.62 | 6.19 |
| one-scalar | 300 | 3.58 | 8.98 | 5.40 |
| one-scalar | 1000 | 3.70 | 8.99 | 5.29 |
| two-scalars | 100 | 6.62 | 11.65 | 5.03 |
| two-scalars | 200 | 7.03 | 11.41 | 4.37 |
| two-scalars | 300 | 6.88 | 10.98 | 4.10 |
| two-scalars | 1000 | 6.89 | 11.78 | 4.89 |
| nested-input-object | 100 | 27.96 | 33.77 | 5.81 |
| nested-input-object | 200 | 29.45 | 32.49 | 3.04 |
| nested-input-object | 300 | 29.53 | 32.18 | 2.65 |
| nested-input-object | 1000 | 28.97 | 33.99 | 5.02 |

## Work asked of the printer
Calls to print() for one pair / syntax nodes handed to print() for one pair, 100 fields.
| case | base | head | fix-fastpath | fix-with-arguments-only | fix-both | fix-maintainers-pr-3967 | fix-memo |
| --- | --- | --- | --- | --- | --- | --- | --- |
| no-arguments | 0.00 / 0.00 | 2.00 / 2.00 | 0.00 / 0.00 | 2.00 / 2.00 | 0.00 / 0.00 | 0.00 / 0.00 | 0.02 / 0.02 |
| one-scalar | 2.00 / 2.00 | 2.00 / 8.00 | 2.00 / 8.00 | 2.00 / 2.00 | 2.00 / 2.00 | 2.00 / 2.00 | 0.02 / 0.08 |
| two-scalars | 4.00 / 4.00 | 2.00 / 14.00 | 2.00 / 14.00 | 4.00 / 4.00 | 4.00 / 4.00 | 4.00 / 4.00 | 0.02 / 0.14 |
| nested-input-object | 2.00 / 36.00 | 2.00 / 42.00 | 2.00 / 42.00 | 2.00 / 36.00 | 2.00 / 36.00 | 2.00 / 36.00 | 0.02 / 0.42 |

## Small queries, built package
Microseconds for one validation: overlap rule alone / all default rules.
| workload | query bytes | base | head |
| --- | --- | --- | --- |
| no-arguments x2 | 16 | 16 / 54 | 17 / 53 |
| one-scalar x2 | 24 | 21 / 57 | 22 / 57 |
| two-scalars x2 | 36 | 18 / 64 | 26 / 70 |
| nested-input-object x2 | 122 | 52 / 118 | 55 / 111 |
| no-arguments x5 | 34 | 12 / 49 | 52 / 89 |
| one-scalar x5 | 54 | 47 / 94 | 91 / 144 |
| two-scalars x5 | 84 | 84 / 133 | 118 / 166 |
| nested-input-object x5 | 299 | 305 / 427 | 353 / 420 |
| no-arguments x10 | 64 | 16 / 58 | 176 / 220 |
| one-scalar x10 | 104 | 178 / 235 | 366 / 423 |
| two-scalars x10 | 164 | 336 / 398 | 484 / 564 |
| nested-input-object x10 | 594 | 1299 / 1463 | 1452 / 1550 |
| no-arguments x20 | 124 | 31 / 81 | 706 / 762 |
| one-scalar x20 | 204 | 703 / 769 | 1521 / 1648 |
| two-scalars x20 | 324 | 1353 / 1446 | 2030 / 2173 |
| nested-input-object x20 | 1184 | 5345 / 5434 | 5886 / 6078 |
| no-arguments x50 | 304 | 133 / 202 | 4619 / 4505 |
| one-scalar x50 | 504 | 4275 / 4423 | 9629 / 9734 |
| two-scalars x50 | 804 | 8616 / 8564 | 12791 / 12898 |
| nested-input-object x50 | 2954 | 32572 / 33182 | 37130 / 39771 |
| 5 fragments on one object, each selecting two fields with an argument | 527 | 172 / 284 | 454 / 577 |
| 20 fragments on one object, each selecting two fields with an argument | 2092 | 2566 / 2959 | 8213 / 8759 |
| 50 fragments on one object, each selecting two fields with an argument | 5242 | 15838 / 16970 | 50776 / 51321 |

## Fragment workloads, built package
Microseconds for one validation: overlap rule alone / all default rules.
| workload | query bytes | base | head | fix-fastpath | fix-with-arguments-only |
| --- | --- | --- | --- | --- | --- |
| 5 fragments on one object, each selecting two fields with an argument | 527 | 198 / 334 | 589 / 726 | 380 / 495 | 396 / 496 |
| 5 fragments on one object, fields with arguments only | 447 | 153 / 268 | 336 / 485 | 324 / 489 | 159 / 280 |
| 20 fragments on one object, each selecting two fields with an argument | 2092 | 2606 / 2904 | 8108 / 8353 | 5416 / 5866 | 5987 / 6281 |
| 20 fragments on one object, fields with arguments only | 1772 | 2322 / 2696 | 4868 / 5194 | 5360 / 5380 | 2450 / 3033 |
| 50 fragments on one object, each selecting two fields with an argument | 5242 | 16032 / 17149 | 51912 / 53244 | 34992 / 37621 | 36382 / 37298 |
| 50 fragments on one object, fields with arguments only | 4442 | 14667 / 15631 | 32057 / 34757 | 33474 / 35000 | 16435 / 16295 |

## The project's own benchmarks at the head, run against each built package
| benchmark | base | head |
| --- | --- | --- |
| Validate Introspection Query (project-benchmark-validateGQL), ms for one measure() | 0.228 | 0.234 |
| Validate Invalid Query (project-benchmark-validateInvalidGQL), ms for one measure() | 0.660 | 0.616 |

## The benchmark the maintainers added later (pull request 3967), run against each built package
| benchmark | base | head | fix-fastpath | fix-with-arguments-only | fix-both | fix-maintainers-pr-3967 | fix-memo |
| --- | --- | --- | --- | --- | --- | --- | --- |
| Many repeated fields (later-benchmark-repeated-fields), ms for one measure() | 4.943 | 117.389 | 4.972 | 119.636 | 4.676 | 5.172 | 6.231 |

