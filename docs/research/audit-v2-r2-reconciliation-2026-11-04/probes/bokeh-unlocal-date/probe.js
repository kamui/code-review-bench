// _unlocal_date at base ccb4bcb and head 36549bc (bokehjs/src/lib/models/widgets/date_picker.ts),
// bodies copied verbatim with the TypeScript types removed, run in one time zone (TZ env).
// Pikaday's rules used below are read from the pinned fork bokeh/pikaday@6b7258e (pikaday.js):
//   setDate: setToStartOfDay (setHours(0,0,0,0)), then field.value = toString() = _d.toDateString()  (l.780-784, 728-737)
//   a day is disabled when day < minDate or day > maxDate, after setToStartOfDay on min/max          (l.906, 926, 1107-1108)
// Inputs as the tests send them: Bokeh's convert_datetime_type turns a naive datetime into ms since the epoch,
// read as UTC (bokeh/util/serialization.py l.178-180); the clicked value is date.toDateString() (date_picker.ts l.95).
function unlocal_base(date) {
  const datestr = date.toISOString().substr(0, 10)
  const tup = datestr.split('-')
  return new Date(Number(tup[0]), Number(tup[1])-1, Number(tup[2]))
}
function unlocal_head(date) {
  const timeOffsetInMS = date.getTimezoneOffset() * 60000
  date.setTime(date.getTime() - timeOffsetInMS)
  const datestr = date.toISOString().substr(0, 10)
  const tup = datestr.split('-')
  return new Date(Number(tup[0]), Number(tup[1])-1, Number(tup[2]))
}
const startOfDay = d => { const x = new Date(d.getTime()); x.setHours(0,0,0,0); return x }
const shown = d => startOfDay(d).toDateString()
const rows = []
for (const [name, f] of [['base', unlocal_base], ['head', unlocal_head]]) {
  const value = Date.UTC(2019, 8, 20)            // datetime(2019, 9, 20)
  const min = Date.UTC(2019, 8, 1)               // datetime(2019, 9, 1)
  const clicked = 'Mon Sep 16 2019'              // after clicking day 16
  const r = {
    commit: name,
    initial_field: shown(f(new Date(value))),
    after_click_field: shown(f(new Date(clicked))),
    min_shown: shown(f(new Date(min))),
  }
  // max_date = datetime.utcnow(): earliest possible run is the cut-off; also the start and end of that UTC day
  for (const [label, iso] of [['cutoff', '2019-10-03T04:32:25Z'], ['utc_midnight', '2019-10-03T00:00:00Z'], ['utc_2359', '2019-10-03T23:59:59Z']]) {
    const mx = startOfDay(f(new Date(Date.parse(iso))))
    const mn = startOfDay(f(new Date(min)))
    const day16 = new Date(2019, 8, 16)
    r['max_' + label] = mx.toDateString()
    r['day16_disabled_' + label] = (day16 < mn) || (day16 > mx)
  }
  rows.push(r)
}
console.log(JSON.stringify({tz: process.env.TZ, offset_min_sep16: new Date(2019, 8, 16).getTimezoneOffset(), rows}))
