"""Step 4: what the Python side holds after the viewer clicks a day.

usage: receive.py "<string stored by the widget>"   (PYTHONPATH at the commit's source tree)
"""
import datetime
import sys

from bokeh.models import DatePicker

limit = datetime.datetime(2019, 9, 20, 23, 30)
picker = DatePicker(value=datetime.date(2019, 9, 10), max_date=limit)
picker.set_from_json("value", sys.argv[1])
print("  max_date set by the application: %r" % (limit,))
print("  browser sends value %r; Python now holds %r" % (sys.argv[1], picker.value))
print("  held value is after the day of max_date: %s" % (picker.value > limit.date()))
