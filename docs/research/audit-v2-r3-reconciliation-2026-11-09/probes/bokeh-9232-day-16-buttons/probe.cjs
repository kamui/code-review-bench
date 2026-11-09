// Runs Pikaday's own month renderer (bokeh/pikaday@6b7258e, the version bokehjs pins at head
// 36549bc) and counts the day buttons whose data-pika-day is "16".
const Pikaday = require('./pikaday-6b7258e.cjs');

function monthHtml(year, month, options) {
  const self = Object.create(Pikaday.prototype);
  self.config(options);
  return self.render(year, month, 'probe');
}
const count16 = (html) => (html.match(/data-pika-day="16"/g) || []).length;
const outsideDays = (html) =>
  [...html.matchAll(/class="is-outside-current-month[^"]*"[^>]*>\s*<button[^>]*data-pika-day="(\d+)"/g)].map((m) => Number(m[1]));

// 1. The tests' picker: the month render() opens on (September 2019) with render()'s options,
//    as shipped (adjacent days hidden) and with adjacent days shown.
for (const show of [false, true]) {
  const html = monthHtml(2019, 8, {
    defaultDate: new Date(2019, 8, 20), setDefaultDate: true,
    minDate: new Date(2019, 8, 1), maxDate: new Date(2019, 9, 3),
    showDaysInNextAndPreviousMonths: show,
  });
  console.log(`Sep 2019 showDaysInNextAndPreviousMonths=${show}: buttons with data-pika-day="16": ${count16(html)}; adjacent-month day numbers shown: ${JSON.stringify(outsideDays(html))}`);
}

// 2. Every month from 1900 to 2100, every firstDay, adjacent days shown.
let months = 0, worst = 0, maxOutside = 0, minLeading = 99, maxTrailing = 0;
for (let year = 1900; year <= 2100; year++) {
  for (let month = 0; month < 12; month++) {
    for (let firstDay = 0; firstDay < 7; firstDay++) {
      const html = monthHtml(year, month, { firstDay, showDaysInNextAndPreviousMonths: true });
      months++;
      worst = Math.max(worst, count16(html));
      const outside = outsideDays(html);
      for (const d of outside) {
        if (d > 15) minLeading = Math.min(minLeading, d); else maxTrailing = Math.max(maxTrailing, d);
      }
      maxOutside = Math.max(maxOutside, outside.length);
    }
  }
}
console.log(`months x firstDay rendered: ${months}; most buttons with data-pika-day="16" in one month: ${worst}; most adjacent-month cells in one month: ${maxOutside}; smallest leading day number: ${minLeading}; largest trailing day number: ${maxTrailing}`);
