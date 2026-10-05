// Step 2: the commit's own _unlocal_date, run in the time zone given by the TZ variable.
// usage: TZ=<zone> node convert.cjs <path to date_picker.ts> <wire.json>
const fs = require('node:fs')
const {unlocal_date_from} = require('./extract.cjs')

const [source_path, wire_path] = process.argv.slice(2)
const {fn: unlocal_date} = unlocal_date_from(source_path)
const wire = JSON.parse(fs.readFileSync(wire_path, 'utf8'))

const pad = n => String(n).padStart(2, '0')
const local_day = d => `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`
const WRITTEN = '2019-09-20'
const verdict = shown => {
  const days = Math.round((Date.parse(shown) - Date.parse(WRITTEN)) / 86400000)
  return days === 0 ? 'same day' : days > 0 ? `${days} DAY LATE` : `${-days} DAY EARLY`
}

// DatePickerView.render() calls this._unlocal_date(new Date(this.model.<property>)).
const shown_for = model_value => local_day(unlocal_date(new Date(model_value)))

const offset_minutes = -new Date(Date.UTC(2019, 8, 20, 12)).getTimezoneOffset()
const sign = offset_minutes < 0 ? '-' : '+'
const offset = `UTC${sign}${pad(Math.floor(Math.abs(offset_minutes) / 60))}:${pad(Math.abs(offset_minutes) % 60)}`
console.log(`TZ=${process.env.TZ} (${offset} on 2019-09-20)`)

for (const row of wire) {
  const shown = shown_for(row.wire_ms)
  console.log(`  set from Python ${row.python.padEnd(30)} sent as ${new Date(row.wire_ms).toISOString()}  picker gets ${shown}  ${verdict(shown)}`)
}

// DatePickerView._on_select() stores date.toDateString(); the next render() parses that string.
const clicked = new Date(2019, 8, 20).toDateString()
const shown_clicked = shown_for(clicked)
console.log(`  clicked in the calendar ${`"${clicked}"`.padEnd(22)} parsed as ${new Date(clicked).toISOString()}  picker gets ${shown_clicked}  ${verdict(shown_clicked)}`)

const wrong = {}
for (let minutes = 0; minutes < 24 * 60; minutes += 30) {
  const shown = shown_for(Date.UTC(2019, 8, 20, 0, minutes))
  const v = verdict(shown)
  if (v !== 'same day')
    (wrong[v] ||= []).push(`${pad(Math.floor(minutes / 60))}:${pad(minutes % 60)}`)
}
const windows = Object.entries(wrong).map(([v, times]) => `${v} for times ${times[0]} to ${times[times.length - 1]} (${times.length} of 48 half-hours)`)
console.log(`  every half-hour of datetime(2019, 9, 20, H, M): ${windows.length ? windows.join('; ') : 'same day for all 48'}`)
