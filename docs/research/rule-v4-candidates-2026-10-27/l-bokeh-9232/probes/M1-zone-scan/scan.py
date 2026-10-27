"""List every timezone and the last date on which the head DatePicker shows a selected day as the day before.

    python3 -I scan.py > result.txt

The head subtracts getTimezoneOffset() minutes, which V8 truncates to whole minutes, then takes the UTC day. The
day comes out wrong only when the zone is ahead of UTC on that date by an offset with leftover seconds. This walks
every day from 1850 to 2030 in each zone of the system's timezone data and reports the first and last such day."""
import sys
from datetime import date, datetime, timedelta
from zoneinfo import ZoneInfo, available_timezones

rows = []
for name in sorted(available_timezones()):
    zone, first, last, day = ZoneInfo(name), None, None, date(1850, 1, 1)
    while day <= date(2030, 12, 31):
        seconds = int(datetime(day.year, day.month, day.day, tzinfo=zone).utcoffset().total_seconds())
        if seconds > 0 and seconds % 60:
            first, last = first or day, day
        day += timedelta(days=1)
    if last:
        rows.append((last.isoformat(), name, first.isoformat()))
print(f"python {sys.version.split()[0]}; zones scanned {len(available_timezones())}; zones affected {len(rows)}")
print(f"zones affected on or after 1900-01-01: {sum(1 for row in rows if row[0] >= '1900-01-01')}")
for last, name, first in sorted(rows, reverse=True):
    print(f"{name}\tfirst {first}\tlast {last}")
