const fs = require('fs');
const path = require('path');
const vm = require('vm');

const [sourceRoot, depsRoot, mode = 'original', vendorRoot] = process.argv.slice(2);
const ts = require(path.join(depsRoot, 'node_modules/typescript'));
const sourcePath = path.join(sourceRoot, 'bokehjs/src/lib/models/widgets/date_picker.ts');
const source = fs.readFileSync(sourcePath, 'utf8');
const transpiled = ts.transpileModule(source, {
  compilerOptions: {target: ts.ScriptTarget.ES2017, module: ts.ModuleKind.CommonJS},
}).outputText;
const exportsObject = {};
class Host {}
vm.runInNewContext(transpiled, {
  exports: exportsObject,
  require(name) {
    if (name === './input_widget') return {InputWidget: Host, InputWidgetView: Host};
    if (name === 'pikaday') return function Pikaday() {};
    return {};
  },
  Date,
});
const proto = exportsObject.DatePickerView.prototype;
function convert(value) {
  const d = new Date(value);
  return proto._unlocal_date.call(proto, d, typeof value === 'number');
}
function day(d) {
  return [d.getFullYear(), String(d.getMonth() + 1).padStart(2, '0'), String(d.getDate()).padStart(2, '0')].join('-');
}
if (mode === 'node') {
  console.log(JSON.stringify({timezone: process.env.TZ, node: process.version, tz: process.versions.tz}));
  for (const [name, value, expected] of [
    ['historical-selection', new Date(1890, 0, 1).toDateString(), '1890-01-01'],
    ['modern-selection', new Date(2019, 8, 20).toDateString(), '2019-09-20'],
    ['python-midnight', Date.UTC(2019, 8, 20), '2019-09-20'],
    ['python-early', Date.UTC(2019, 8, 20, 3), '2019-09-20'],
    ['python-late', Date.UTC(2019, 8, 20, 23, 30), '2019-09-20'],
  ]) {
    const d = new Date(value);
    console.log(JSON.stringify({name, value, instant: d.toISOString(), offset_minutes: d.getTimezoneOffset(), expected, actual: day(convert(value))}));
  }
  let wrong = 0;
  for (let halfhour = 0; halfhour < 48; halfhour++) {
    if (day(convert(Date.UTC(2019, 8, 20, 0, halfhour * 30))) !== '2019-09-20') wrong++;
  }
  console.log(JSON.stringify({python_halfhours:48, wrong}));
} else if (mode === 'browser') {
  process.env.TMPDIR = path.join(depsRoot, '../t');
  process.env.XDG_CACHE_HOME = path.join(depsRoot, '../cache');
  process.env.XDG_CONFIG_HOME = path.join(depsRoot, '../browser-config');
  fs.mkdirSync(process.env.TMPDIR, {recursive:true});
  const {chromium} = require(path.join(depsRoot, 'node_modules/playwright-core'));
  (async () => {
    const browser = await chromium.launch({executablePath:'/usr/bin/google-chrome', headless:true, args:['--no-sandbox', '--disable-dev-shm-usage']});
    console.log('browser=' + browser.version());
    try { for (const zone of ['Europe/Paris', 'America/New_York', 'UTC']) {
      const context = await browser.newContext({timezoneId:zone});
      const page = await context.newPage();
      await page.setContent('<html><body></body></html>');
      await page.addScriptTag({content:fs.readFileSync(path.join(vendorRoot, 'pikaday.js'),'utf8')});
      const result = await page.evaluate(({code}) => {
        class InputWidgetView {
          constructor() {
            this._root_element = document.createElement('div');
            this.group_el = document.createElement('div');
            this._root_element.appendChild(this.group_el);
            document.body.appendChild(this._root_element);
          }
          render() { this.group_el.replaceChildren(); }
          change_input() {}
        }
        const out = {};
        new Function('exports','require',code)(out, name => {
          if (name === './input_widget') return {InputWidgetView, InputWidget:class {}};
          if (name === 'pikaday') return window.Pikaday;
          if (name === 'core/dom') return {input:attrs => Object.assign(document.createElement('input'), attrs)};
          if (name === 'styles/widgets/inputs') return {bk_input:'bk-input'};
          return {};
        });
        const results = [];
        for (const scenario of [
          {name:'historical-selection', value:Date.UTC(1890,0,15), select:[1890,0,1]},
          {name:'modern-selection', value:Date.UTC(2019,8,15), select:[2019,8,20]},
          {name:'python-midnight', value:Date.UTC(2019,8,20)},
          {name:'python-early', value:Date.UTC(2019,8,20,3)},
          {name:'python-late', value:Date.UTC(2019,8,20,23,30)},
        ]) {
          const view = new out.DatePickerView();
          let value = scenario.value;
          const time = new Date(scenario.value);
          view.model = {min_date:scenario.select ? null : Date.UTC(2019,8,5,time.getUTCHours(),time.getUTCMinutes()),
            max_date:scenario.select ? null : scenario.value, disabled:false};
          Object.defineProperty(view.model, 'value', {get:() => value, set:next => {value = next; view.render();}});
          view.render();
          const before = view.input_el.value;
          if (scenario.select) {
            view._picker.show();
            view._picker.gotoDate(new Date(...scenario.select));
            const button = view._picker.el.querySelector(`button[data-pika-year="${scenario.select[0]}"][data-pika-month="${scenario.select[1]}"][data-pika-day="${scenario.select[2]}"]`);
            if (!button) throw new Error('Calendar day is not reachable');
            button.dispatchEvent(new MouseEvent('mousedown', {bubbles:true}));
          }
          let allowed_days = null;
          if (!scenario.select) {
            view._picker.show();
            view._picker.gotoDate(new Date(2019,8,15));
            allowed_days = [...view._picker.el.querySelectorAll('td:not(.is-disabled) button[data-pika-year="2019"][data-pika-month="8"]')]
              .map(button => Number(button.getAttribute('data-pika-day')));
          }
          results.push({name:scenario.name, initial_display:before, model_value:value, display:view.input_el.value, allowed_days});
          view._picker.destroy();
          view._root_element.remove();
        }
        const historical = new Date(1890,0,1);
        const native = document.createElement('input'); native.type='date'; native.value='1890-01-01';
        const field = document.createElement('input'); document.body.appendChild(field);
        const picker = new window.Pikaday({field, defaultDate:historical, setDefaultDate:true});
        const controls = {native_date_value:native.value, native_date_utc:native.valueAsDate.toISOString(),
          javascript_local_day:historical.toDateString(), instant:historical.toISOString(), offset_minutes:historical.getTimezoneOffset(),
          standalone_pikaday:field.value};
        picker.destroy();
        return {zone:Intl.DateTimeFormat().resolvedOptions().timeZone, results, controls};
      }, {code:transpiled});
      console.log(JSON.stringify(result));
      await context.close();
    } } finally { await browser.close(); }
  })().catch(error => {console.error(error);process.exitCode=1;});
} else {
  throw new Error('Expected node or browser mode');
}
