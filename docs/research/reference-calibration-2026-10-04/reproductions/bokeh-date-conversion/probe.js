// The two versions of the date picker's conversion, copied from the diff of bokeh PR #9232.
function before(date) {
  const datestr = date.toISOString().substr(0, 10)
  const tup = datestr.split('-')
  return new Date(Number(tup[0]), Number(tup[1]) - 1, Number(tup[2]))
}
function after(date) {
  const timeOffsetInMS = date.getTimezoneOffset() * 60000
  date.setTime(date.getTime() - timeOffsetInMS)
  const datestr = date.toISOString().substr(0, 10)
  const tup = datestr.split('-')
  return new Date(Number(tup[0]), Number(tup[1]) - 1, Number(tup[2]))
}
const show = d => `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`
// Python sends a date as midnight UTC. A date the user clicks arrives as local midnight.
const fromPython = () => new Date(Date.UTC(2019, 9, 3))
const clicked = () => new Date(2019, 9, 3)
console.log(process.env.TZ.padEnd(20), '| set from Python 2019-10-03: before', show(before(fromPython())), 'after', show(after(fromPython())),
  '| clicked 2019-10-03: before', show(before(clicked())), 'after', show(after(clicked())))
