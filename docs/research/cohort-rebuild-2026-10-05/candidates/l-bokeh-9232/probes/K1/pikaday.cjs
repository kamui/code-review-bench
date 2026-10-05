// Step 3: the calendar library bokeh pins (bokeh/pikaday#6b7258e), driven the way
// DatePickerView.render() drives it, with the commit's own _unlocal_date. jsdom stands in for
// the browser page; dates and time zones come from node, so TZ decides the viewer's zone.
// usage: TZ=<zone> NODE_PATH=<scratch>/js/node_modules node pikaday.cjs <path to date_picker.ts> [<file for the clicked value>]
const fs = require('node:fs')
const {JSDOM} = require('jsdom')
const {unlocal_date_from} = require('./extract.cjs')

const dom = new JSDOM('<!doctype html><html><body></body></html>')
globalThis.window = dom.window
globalThis.document = dom.window.document
const Pikaday = require('pikaday')

const {fn: unlocal_date} = unlocal_date_from(process.argv[2])
const utc = (...parts) => Date.UTC(...parts)

function render(model) {
  let stored = null
  const input_el = document.createElement('input')
  input_el.type = 'text'
  document.body.appendChild(input_el)
  // Same options, in the same order, as DatePickerView.render() at both commits.
  const picker = new Pikaday({
    field: input_el,
    defaultDate: unlocal_date(new Date(model.value)),
    setDefaultDate: true,
    minDate: model.min_date != null ? unlocal_date(new Date(model.min_date)) : undefined,
    maxDate: model.max_date != null ? unlocal_date(new Date(model.max_date)) : undefined,
    // DatePickerView._on_select() stores exactly this string as the new value.
    onSelect: (date) => { stored = date.toDateString() },
  })
  document.body.appendChild(picker.el)
  picker.show()
  const selectable = []
  for (const button of picker.el.querySelectorAll('button.pika-button')) {
    const cell = button.closest('td')
    if (!cell.classList.contains('is-disabled') && !cell.classList.contains('is-outside-current-month'))
      selectable.push(button)
  }
  const day = button => Number(button.getAttribute('data-pika-day'))
  const month = picker.el.querySelector('.pika-label').firstChild.textContent.trim()
  const result = {text: input_el.value, month, first: day(selectable[0]), last: day(selectable[selectable.length - 1])}
  selectable[selectable.length - 1].dispatchEvent(new dom.window.MouseEvent('mousedown', {bubbles: true}))
  result.stored = stored
  picker.destroy()
  return result
}

const scenarios = [
  ['reference bug input: value=date(2019,9,20), min_date=date(2019,9,5), max_date=date(2019,9,20)',
    {value: utc(2019, 8, 20), min_date: utc(2019, 8, 5), max_date: utc(2019, 8, 20)}],
  ['K1 input, late:      value=datetime(2019,9,20,23,30), min_date=datetime(2019,9,5,23,30), max_date=datetime(2019,9,20,23,30)',
    {value: utc(2019, 8, 20, 23, 30), min_date: utc(2019, 8, 5, 23, 30), max_date: utc(2019, 8, 20, 23, 30)}],
  ['K1 input, early:     value=datetime(2019,9,20,3,0), min_date=datetime(2019,9,5,3,0), max_date=datetime(2019,9,20,3,0)',
    {value: utc(2019, 8, 20, 3), min_date: utc(2019, 8, 5, 3), max_date: utc(2019, 8, 20, 3)}],
]

console.log(`TZ=${process.env.TZ}`)
const results = scenarios.map(([label, model]) => {
  const r = render(model)
  console.log(`  ${label}`)
  console.log(`      text in the box: "${r.text}"   selectable days in ${r.month}: ${r.first} to ${r.last}   clicking day ${r.last} stores "${r.stored}"`)
  return r
})
if (process.argv[3])
  fs.writeFileSync(process.argv[3], results[1].stored)
