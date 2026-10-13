"""A stand-in so that `from psycopg_pool import ConnectionPool` succeeds without psycopg 3.

No real install gives this: the real psycopg_pool imports psycopg (run, see result-head.txt,
environment "psycopg2 + psycopg-pool package"). It exists only to show what Django's `pool`
property does after the import when the driver is psycopg2.
"""


class ConnectionPool:
    def __init__(self, *args, **kwargs):
        raise RuntimeError("stand-in ConnectionPool was constructed")
