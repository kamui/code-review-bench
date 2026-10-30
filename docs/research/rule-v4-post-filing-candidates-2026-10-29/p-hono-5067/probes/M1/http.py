"""Over-HTTP run for group M1, at base and head, on Node, Bun and Deno. python3 -I http.py

Each runtime serves http-probe.mjs on 127.0.0.1; curl posts a valid multipart body with each header.
Writes http-base.txt and http-head.txt beside this file.
"""
import importlib.util
import pathlib
import subprocess
import time

OUT = pathlib.Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location('driver', OUT / 'run.py')
driver = importlib.util.module_from_spec(spec)
spec.loader.exec_module(driver)
NODE_SERVER = driver.CLONE / 'node_modules/@hono/node-server/dist/index.mjs'
HEADERS = [
    'multipart/form-data-extra; boundary=X',
    'multipart/form-data,foo; boundary=X',
    'multipart/form-data boundary=X',
    'multipart/form-data; boundary=X',
]
BODY = '--X\r\nContent-Disposition: form-data; name="foo"\r\n\r\nbar\r\n--X--\r\n'
RUNTIMES = [('node', ['node']), ('bun', ['bun']), ('deno', ['deno', 'run', '--allow-read', '--allow-env', '--allow-net'])]


def run(label):
    lines = []
    for number, (name, runtime) in enumerate(RUNTIMES):
        port = str(18700 + number)
        server = subprocess.Popen([*runtime, str(OUT / 'http-probe.mjs'), str(driver.SCRATCH / 'api.mjs'), port, str(NODE_SERVER)],
                                  stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
        try:
            assert server.stdout.readline().strip() == 'ready'
            time.sleep(0.3)
            for header in HEADERS:
                done = subprocess.run(['curl', '-s', '-o', '-', '-w', ' HTTP %{http_code}', '-X', 'POST', '-H', 'Content-Type: ' + header,
                                       '--data-binary', BODY, 'http://127.0.0.1:' + port + '/'], text=True, capture_output=True)
                lines.append(name + ' | Content-Type: ' + header + ' | ' + done.stdout.strip())
        finally:
            server.terminate()
            server.wait()
    (OUT / ('http-' + label + '.txt')).write_text('curl -s -X POST -H "Content-Type: <header>" --data-binary <valid multipart body, boundary X> http://127.0.0.1:<port>/\n'
                                                   'route: app.post("/", async (c) => c.json(await c.req.parseBody()))\n' + '\n'.join(lines) + '\n')


if __name__ == '__main__':
    for label, commit in [('base', driver.BASE), ('head', driver.HEAD)]:
        driver.checkout(commit)
        driver.bundle()
        run(label)
    driver.checkout(driver.HEAD)
