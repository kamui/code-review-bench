import copy
from types import SimpleNamespace
from unittest.mock import patch

from django.conf import settings
settings.configure(DATABASES={'default': {'ENGINE': 'django.db.backends.postgresql', 'NAME': 'app', 'OPTIONS': {'pool': True}}}, USE_TZ=True, INSTALLED_APPS=[])
import django
django.setup()
from django.db import connections
from django.db.backends.postgresql.base import DatabaseWrapper
import psycopg
import psycopg_pool
from psycopg.pq import TransactionStatus

print('versions', django.get_version(), psycopg.__version__, psycopg_pool.__version__)

def wrapper(alias, options=None):
    config = copy.deepcopy(connections['default'].settings_dict)
    if options is not None:
        config['OPTIONS'] = options
    return DatabaseWrapper(config, alias)

class FakeCursor:
    def __enter__(self):
        return self
    def __exit__(self, *args):
        pass
    def execute(self, *args):
        pass

class FakeConnection:
    """Only replace the wire connection; Django and pool logic remain real."""
    def __init__(self, **kwargs):
        self.info = SimpleNamespace(parameter_status=lambda name: 'UTC', server_version=150000)
        self.pgconn = SimpleNamespace(transaction_status=TransactionStatus.IDLE)
        self.autocommit = True
        self.closed = False
        self.dbname = kwargs.get('dbname')
    @classmethod
    def connect(cls, *args, **kwargs):
        return cls(**kwargs)
    def cursor(self):
        return FakeCursor()
    def close(self):
        self.closed = True
        self.pgconn.transaction_status = TransactionStatus.UNKNOWN

# The configure callback uses the real operations and mogrify path.
role = wrapper('role', {'assume_role': 'app_role', 'pool': {'connection_class': FakeConnection, 'min_size': 1, 'max_size': 1, 'timeout': 0.15, 'num_workers': 1}})
try:
    role.connect()
except Exception as exc:
    print('role startup:', type(exc).__name__, str(exc))
    print('role pool stats:', role.pool.get_stats())
finally:
    role.close_pool()

# Test setup must replace an existing pool when the database name changes.
test = connections['default']
test.settings_dict['OPTIONS']['pool'] = {'connection_class': FakeConnection, 'min_size': 1, 'max_size': 1, 'timeout': 0.2}
test.pg_version = 150000
test.connect()
print('pre-test physical DB:', test.connection.dbname)
initial_pool = test.pool  # Real unopened pool, using production DB parameters.
observed = []
def record_command(command, *args, **kwargs):
    test.connect()
    observed.append((command, test.settings_dict['NAME'], test.connection.dbname))
    test.close()
with patch.object(test.creation, '_create_test_db'), patch('django.core.management.call_command', side_effect=record_command):
    test.creation.create_test_db(verbosity=0, autoclobber=True, serialize=False)
print('test setup (command, settings DB, pool DB):', observed)
assert test.pool is initial_pool
# Worker setup also changes NAME without removing an existing pool.
test.creation.setup_worker_connection(1)
test.connect()
print('worker setup (settings DB, physical DB):', test.settings_dict['NAME'], test.connection.dbname)
test.close()
test.close_pool()

empty = wrapper('empty', {'pool': {}})
print('empty dictionary pool:', empty.pool)
