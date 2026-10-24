// date_picker.ts head logic (_unlocal_date) vs the getters, for a local instant just after midnight in 1890 Amsterdam (LMT +00:19:32).
const d = new Date(1890, 0, 1, 0, 0, 10);
const offset = d.getTimezoneOffset();
const shifted = new Date(d.getTime() - offset * 60000);
const iso = shifted.toISOString().split("T")[0];
console.log(JSON.stringify({ engine: typeof Bun !== "undefined" ? "bun " + Bun.version : "node " + process.version, offsetMinutes: offset, wholeMinutes: Number.isInteger(offset), shiftedIsoDate: iso, getters: [d.getFullYear(), d.getMonth() + 1, d.getDate()].join("-") }));
