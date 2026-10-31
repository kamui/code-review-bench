// Head bokehjs _unlocal_date (date_picker.ts at 36549bc, lines 78-87) against the one-line form the comments name.
function head(date) {
  const timeOffsetInMS = date.getTimezoneOffset() * 60000
  date.setTime(date.getTime() - timeOffsetInMS)
  const datestr = date.toISOString().substr(0, 10)
  const tup = datestr.split('-')
  return new Date(Number(tup[0]), Number(tup[1]) - 1, Number(tup[2]))
}
const oneLiner = (d) => new Date(d.getFullYear(), d.getMonth(), d.getDate())
const engine = typeof Bun !== 'undefined' ? 'bun ' + Bun.version : 'node ' + process.version
const out = { engine, tz: process.env.TZ }
// 1. Local midnight, 1 January 1890 (Paris used local mean time, +00:09:21).
const lmt = new Date(1890, 0, 1, 0, 0, 0)
out.lmt = { offsetMinutes: lmt.getTimezoneOffset(), head: head(new Date(lmt)).toDateString(), oneLiner: oneLiner(lmt).toDateString() }
// 2. Every half hour of 2019 (the second grader's comparison).
let diffs = 0, n = 0
for (let t = Date.UTC(2019, 0, 1); t < Date.UTC(2020, 0, 1); t += 30 * 60000) {
  n++
  if (head(new Date(t)).getTime() !== oneLiner(new Date(t)).getTime()) diffs++
}
out.y2019 = { instants: n, differences: diffs }
// 3. Every local midnight from 1880 to 1920.
let d2 = 0, first = null
for (let y = 1880; y < 1920; y++) for (let m = 0; m < 12; m++) for (let day = 1; day <= 28; day++) {
  const d = new Date(y, m, day)
  if (head(new Date(d)).getTime() !== oneLiner(d).getTime()) { d2++; if (!first) first = d.toDateString() }
}
out.midnights1880to1919 = { differences: d2, first }
console.log(JSON.stringify(out))
