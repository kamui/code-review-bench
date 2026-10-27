import os
import subprocess
import psycopg

params = dict(host='127.0.0.1', port=55432, user='postgres', password='bench', autocommit=True)
name = 'r4v17914_platform'
with psycopg.connect(dbname='template1', **params) as a:
    a.execute('DROP DATABASE IF EXISTS "' + name + '" WITH (FORCE)')
    a.execute('CREATE DATABASE "' + name + '"')
try:
    with psycopg.connect(dbname=name, **params) as c:
        try:
            c.execute('DROP DATABASE "' + name + '"')
        except Exception as e:
            print('Native DROP on current database:', type(e).__name__, str(e))
    with psycopg.connect(dbname='template1', **params) as a:
        a.execute('DROP DATABASE "' + name + '"')
        print('Native DROP from template1: success')
        a.execute('CREATE DATABASE "' + name + '"')
    r = subprocess.run(['dropdb', '-h', '127.0.0.1', '-p', '55432', '-U', 'postgres',
                        '--maintenance-db=template1', name],
                       env=dict(os.environ, PGPASSWORD='bench'), capture_output=True, text=True)
    print('dropdb --maintenance-db=template1:', r.returncode, r.stdout, r.stderr)
finally:
    with psycopg.connect(dbname='template1', **params) as a:
        a.execute('DROP DATABASE IF EXISTS "' + name + '" WITH (FORCE)')
