| Server interval | Base | Head | Range fix | Nonpositive rejection | Both |
| --- | --- | --- | --- | --- | --- |
| 50 ms | report | report | report | report | report |
| 0 | panic | panic | panic | reject-and-retry | reject-and-retry |
| -1 s | panic | panic | panic | reject-and-retry | reject-and-retry |
| -1 ns | panic | panic | panic | reject-and-retry | reject-and-retry |
| -10,000,000,000 s | reject-and-retry | panic | reject-and-retry | reject-and-retry | reject-and-retry |
| 9,223,372,037 s | reject-and-retry | open-without-report | reject-and-retry | open-without-report | reject-and-retry |
| 10,000,000,000 s | reject-and-retry | open-without-report | reject-and-retry | open-without-report | reject-and-retry |
| 315,576,000,000 s | reject-and-retry | open-without-report | reject-and-retry | open-without-report | reject-and-retry |
| 315,576,000,001 s | reject-and-retry | reject-and-retry | reject-and-retry | reject-and-retry | reject-and-retry |
| 9,223,372,036 s | open-without-report | open-without-report | open-without-report | open-without-report | open-without-report |

All rows are run. reject-and-retry means warning, stream closure and reopening. open-without-report means one open stream and no report during 1.2 seconds; conversion values are independently recorded in values-base.txt and values-head.txt.
