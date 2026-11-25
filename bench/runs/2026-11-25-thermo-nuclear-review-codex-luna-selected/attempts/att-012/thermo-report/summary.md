# Review findings

## Empty pool options silently disable pooling

**Priority: P2.** The documented `OPTIONS["pool"]` accepts a dict to pass to `ConnectionPool`, with `True` as the separate shorthand for defaults. An empty dict is a valid dict of default options, but `pool` returns `None` for every falsey value at [django/db/backends/postgresql/base.py:205](../../clone/django/db/backends/postgresql/base.py#L205), and `get_connection_params()` similarly treats it as disabled. As a result, configuring `"pool": {}` silently opens standalone connections instead of pooling; users who use an empty dict expecting the documented defaults get no pooling and none of the pool's connection limits or lifecycle behavior.

## A pooled connection can carry a changed session role to another borrower

**Priority: P2.** The pool's `configure` callback applies `assume_role` when a connection is created, while `_close()` returns that connection to the pool with `putconn()` at lines 391-397 of [django/db/backends/postgresql/base.py:376](../../clone/django/db/backends/postgresql/base.py#L376). No reset callback is supplied to `ConnectionPool`; its default return handling rolls back an open transaction but leaves committed session state intact. Since `SET ROLE` is session state (and Django connections run in autocommit by default), application code that changes the role and then closes its Django wrapper can return a connection with that role still active. A later request or thread can borrow it and run queries with the previous borrower's privileges, including bypassing the configured `assume_role` expectation.
