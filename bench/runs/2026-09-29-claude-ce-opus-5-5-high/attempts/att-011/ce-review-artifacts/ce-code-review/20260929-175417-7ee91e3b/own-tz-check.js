function oldU(date){const s=date.toISOString().substr(0,10).split('-');return new Date(+s[0],+s[1]-1,+s[2])}
function newU(date){const o=date.getTimezoneOffset()*60000;date.setTime(date.getTime()-o);const s=date.toISOString().substr(0,10).split('-');return new Date(+s[0],+s[1]-1,+s[2])}
const inputs={num:Date.UTC(2019,8,20),iso:'2019-09-20',str:'Fri Sep 20 2019'}
for(const [k,v] of Object.entries(inputs)) console.log(process.env.TZ.padEnd(20),k.padEnd(4),'old:',oldU(new Date(v)).toDateString(),' new:',newU(new Date(v)).toDateString())
