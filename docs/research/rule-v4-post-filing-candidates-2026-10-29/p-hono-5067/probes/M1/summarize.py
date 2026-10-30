"""Print a compact table of every result-<label>.txt beside this file. python3 -I summarize.py > summary-table.txt"""
import json
import pathlib

OUT = pathlib.Path(__file__).resolve().parent
LABELS = ['base', 'head', 'fix-GT-p6', 'fix-M1', 'fix-GT-p6+M1', 'fix-one-expression',
          'fix-GT-p1', 'fix-GT-p2', 'fix-GT-p3', 'fix-GT-p4', 'fix-GT-p5', 'fix-GT-p7', 'fix-PE-p1']


def short(outcome):
    if outcome['ok']:
        return json.dumps(outcome['value'], separators=(',', ':'))
    error = outcome['error']
    for key, text in [('not one of', 'ERR(Content-Type not multipart or urlencoded)'),
                      ('incorrect MIME type/boundary', "ERR(can't decode: MIME type/boundary)"),
                      ('can not be decoded', 'ERR(body can not be decoded as form data)'),
                      ('missing final boundary', 'ERR(missing final boundary)')]:
        if key in error:
            return text
    return 'ERR(' + error[:60] + ')'


for label in LABELS:
    path = OUT / ('result-' + label + '.txt')
    print('## ' + label)
    print('runtime | case | Content-Type | platform formData() | parseBody(Request) | c.req.parseBody() in app | validator(form)')
    for line in path.read_text().splitlines():
        if not line.startswith('{'):
            if line.strip():
                print('  ' + line)
            continue
        row = json.loads(line)
        r = row['result']
        app = str(r['app']['status']) + ' ' + r['app']['body'][:40]
        print(' | '.join([row['runtime'], row['label'], row['contentType'], short(r['platformFormData']), short(r['parseBody']),
                          app, str(r['validator']['status']) + ' ' + r['validator']['body'][:30]]))
    print()
