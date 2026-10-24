import sys
root, variant = sys.argv[1], sys.argv[2]
def edit(path, old, new):
    p = f"{root}/{path}"
    s = open(p).read()
    assert s.count(old) == 1, (path, old)
    open(p, "w").write(s.replace(old, new))
CRE = "django/db/backends/postgresql/creation.py"
BASE = "django/db/backends/postgresql/base.py"
M1 = '''
    def destroy_test_db(
        self, old_database_name=None, verbosity=1, keepdb=False, suffix=None
    ):
        self.connection.close()
        self.connection.close_pool()
        super().destroy_test_db(old_database_name, verbosity, keepdb, suffix)
'''
GTV2 = '''
    def create_test_db(self, *args, **kwargs):
        self.connection.close()
        self.connection.close_pool()
        return super().create_test_db(*args, **kwargs)

    def setup_worker_connection(self, _worker_id):
        self.connection.close()
        self.connection.close_pool()
        super().setup_worker_connection(_worker_id)
'''
anchor = "    def _destroy_test_db(self, test_database_name, verbosity):\n"
if variant in ("fix-m1", "fix-both"):
    edit(CRE, anchor, M1.lstrip("\n") + "\n" + anchor)
if variant in ("fix-gtv2", "fix-both"):
    edit(CRE, anchor, GTV2.lstrip("\n") + "\n" + anchor)
    edit(BASE, '''                            "NAME": connection.settings_dict["NAME"],
                        },
                        alias=self.alias,''', '''                            "NAME": connection.settings_dict["NAME"],
                        },
                        alias=NO_DB_ALIAS,''')
if variant == "fix-identity":
    edit(BASE, '''        if self.alias == NO_DB_ALIAS or not pool_options:
            return None
''', '''        if self.alias == NO_DB_ALIAS or not pool_options:
            return None

        pool = self._connection_pools.get(self.alias)
        if pool is not None and pool.kwargs.get("dbname") != (
            self.settings_dict["NAME"] or "postgres"
        ):
            pool.close()
            del self._connection_pools[self.alias]
''')
