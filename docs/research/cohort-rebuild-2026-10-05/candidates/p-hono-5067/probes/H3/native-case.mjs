const B='----probeBoundary'
const mp=`--${B}\r\nContent-Disposition: form-data; name="foo"\r\n\r\nbar\r\n--${B}--\r\n`
for (const [ct,body] of [['Application/X-WWW-Form-Urlencoded','foo=bar'],[`Multipart/Form-Data; boundary=${B}`,mp]]) {
  try { const f=await new Request('http://x/',{method:'POST',headers:{'Content-Type':ct},body}).formData(); console.log(ct,'-> native formData ok',JSON.stringify([...f.entries()])) }
  catch(e){ console.log(ct,'-> native formData THREW',e.message) }
}
