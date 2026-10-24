const engine = typeof Bun !== "undefined" ? "bun " + Bun.version : "node " + process.version;
const body = '--sampleBoundary\r\nContent-Disposition: form-data; name="foo"\r\n\r\nbar\r\n--sampleBoundary--\r\n';
for (const type of ["multipart/form-data; boundary=sampleBoundary", "Multipart/Form-Data; boundary=sampleBoundary", "application/x-www-form-urlencoded", "Application/X-WWW-Form-Urlencoded"]) {
  const b = type.startsWith("multi") || type.startsWith("Multi") ? body : "foo=bar";
  try { const fd = await new Response(b, { headers: { "content-type": type } }).formData(); console.log(engine, JSON.stringify(type), "ok", JSON.stringify(Object.fromEntries(fd))); }
  catch (e) { console.log(engine, JSON.stringify(type), "error", String(e).slice(0, 100)); }
}
