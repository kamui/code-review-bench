"""Step 1: what the Python side of bokeh, at the checked-out commit, sends to the browser.

Run with PYTHONPATH pointing at the commit's source tree. Prints one JSON list on stdout.
"""
import datetime
import json

from bokeh.core.json_encoder import serialize_json
from bokeh.models import DatePicker

INPUTS = [
    ("date(2019, 9, 20)", datetime.date(2019, 9, 20)),
    ("datetime(2019, 9, 20)", datetime.datetime(2019, 9, 20)),
    ("datetime(2019, 9, 20, 3, 0)", datetime.datetime(2019, 9, 20, 3, 0)),
    ("datetime(2019, 9, 20, 12, 0)", datetime.datetime(2019, 9, 20, 12, 0)),
    ("datetime(2019, 9, 20, 23, 30)", datetime.datetime(2019, 9, 20, 23, 30)),
    ('"2019-09-20T23:30"', "2019-09-20T23:30"),
]

rows = []
for label, value in INPUTS:
    picker = DatePicker(value=value, min_date=value, max_date=value)
    wire = json.loads(serialize_json(picker.to_json(include_defaults=False)))
    assert wire["value"] == wire["min_date"] == wire["max_date"]
    rows.append({
        "python": label,
        "held_as": "%s %s" % (type(picker.value).__name__, picker.value),
        "wire_ms": wire["value"],
    })

print(json.dumps(rows))
