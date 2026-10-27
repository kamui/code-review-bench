This is a sample app based on the Django tutorial for version 1.10.
The point of this is to check to see if I can recreate the problem I
am seeing in my app where the tests throw errors when trying to drop
the test database after running my tests.


## TESTING WITH DIFFERENT DATABASE ENGINES ##

### PostgreSQL ###

I have PosgreSQL 9.5.5 installed on an Ubuntu 16.04 server on a Vagrant
VM. For this test, I set up the database and database user as follows:

    postgres=# create user d110example password 'd110example' CREATEDB;

    postgres=# create database d110example owner d110example;


    postgres=# \l
    List of databases
    Name             |    Owner    | Encoding |   Collate   |    Ctype    |   Access privileges
    ------------------+-------------+----------+-------------+-------------+-----------------------
    d110example      | d110example | UTF8     | en_US.UTF-8 | en_US.UTF-8 |
    postgres         | postgres    | UTF8     | en_US.UTF-8 | en_US.UTF-8 |
    template0        | postgres    | UTF8     | en_US.UTF-8 | en_US.UTF-8 | =c/postgres          +
                     |             |          |             |             | postgres=CTc/postgres
    template1        | postgres    | UTF8     | en_US.UTF-8 | en_US.UTF-8 | =c/postgres          +
                     |             |          |             |             | postgres=CTc/postgres
    (4 rows)

    postgres=# \du
    List of roles
    Role name   |                         Attributes                         | Member of
    -------------+------------------------------------------------------------+-----------
    d110example | Create DB                                                  | {}
    postgres    | Superuser, Create role, Create DB, Replication, Bypass RLS | {}


    # And these lines in the pg_hba.conf (my VM answers on 10.1.99.100)

    # local DATABASE            USER            METHOD  [OPTIONS]
    local   d110example         d110example     md5
    local   test_d110example    d110example     md5
    # host  DATABASE            USER            ADDRESS         METHOD  [OPTIONS]
    host    d110example         d110example     10.1.99.0/24    md5
    host    test_d110example    d110example     10.1.99.0/24    md5

This gives the following error:

    $ python manage.py test -v 2 polls
    Creating test database for alias 'default' ('test_d110example')...
    /Users/cnk/.pyenv/versions/d110example-3.5.2/lib/python3.5/site-packages/django/db/backends/postgresql/base.py:248: RuntimeWarning: Normally Django will use a connection to the 'postgres' database to avoid running initialization queries against the production database when it's not needed (for example, when running tests). Django was unable to create a connection to the 'postgres' database and will use the default database instead.
      RuntimeWarning
    Operations to perform:
      Synchronize unmigrated apps: messages, staticfiles
      Apply all migrations: admin, auth, contenttypes, polls, sessions
    Synchronizing apps without migrations:
      Creating tables...
        Running deferred SQL...
    Running migrations:
      Applying contenttypes.0001_initial... OK
      Applying auth.0001_initial... OK
      Applying admin.0001_initial... OK
      Applying admin.0002_logentry_remove_auto_add... OK
      Applying contenttypes.0002_remove_content_type_name... OK
      Applying auth.0002_alter_permission_name_max_length... OK
      Applying auth.0003_alter_user_email_max_length... OK
      Applying auth.0004_alter_user_username_opts... OK
      Applying auth.0005_alter_user_last_login_null... OK
      Applying auth.0006_require_contenttypes_0002... OK
      Applying auth.0007_alter_validators_add_error_messages... OK
      Applying auth.0008_alter_user_username_max_length... OK
      Applying polls.0001_initial... OK
      Applying sessions.0001_initial... OK
    test_detail_view_with_a_future_question (polls.tests.QuestionIndexDetailTests) ... ok
    test_detail_view_with_a_past_question (polls.tests.QuestionIndexDetailTests) ... ok
    test_was_published_recently_with_future_question (polls.tests.QuestionMethodTests) ... ok
    test_was_published_recently_with_old_question (polls.tests.QuestionMethodTests) ... ok
    test_was_published_recently_with_recent_question (polls.tests.QuestionMethodTests) ... ok
    test_index_view_with_a_future_question (polls.tests.QuestionViewTests) ... ok
    test_index_view_with_a_past_question (polls.tests.QuestionViewTests) ... ok
    test_index_view_with_future_question_and_past_question (polls.tests.QuestionViewTests) ... ok
    test_index_view_with_no_questions (polls.tests.QuestionViewTests) ... ok
    test_index_view_with_two_past_questions (polls.tests.QuestionViewTests) ... ok

    ----------------------------------------------------------------------
    Ran 10 tests in 0.099s

    OK
    Destroying test database for alias 'default' ('test_d110example')...
    /Users/cnk/.pyenv/versions/d110example-3.5.2/lib/python3.5/site-packages/django/db/backends/postgresql/base.py:248: RuntimeWarning: Normally Django will use a connection to the 'postgres' database to avoid running initialization queries against the production database when it's not needed (for example, when running tests). Django was unable to create a connection to the 'postgres' database and will use the default database instead.
      RuntimeWarning
    Traceback (most recent call last):
      File "/Users/cnk/.pyenv/versions/d110example-3.5.2/lib/python3.5/site-packages/django/db/backends/utils.py", line 62, in execute
        return self.cursor.execute(sql)
    psycopg2.OperationalError: cannot drop the currently open database


    The above exception was the direct cause of the following exception:

    Traceback (most recent call last):
      File "manage.py", line 22, in <module>
        execute_from_command_line(sys.argv)
      File "/Users/cnk/.pyenv/versions/d110example-3.5.2/lib/python3.5/site-packages/django/core/management/__init__.py", line 367, in execute_from_command_line
        utility.execute()
      File "/Users/cnk/.pyenv/versions/d110example-3.5.2/lib/python3.5/site-packages/django/core/management/__init__.py", line 359, in execute
        self.fetch_command(subcommand).run_from_argv(self.argv)
      File "/Users/cnk/.pyenv/versions/d110example-3.5.2/lib/python3.5/site-packages/django/core/management/commands/test.py", line 29, in run_from_argv
        super(Command, self).run_from_argv(argv)
      File "/Users/cnk/.pyenv/versions/d110example-3.5.2/lib/python3.5/site-packages/django/core/management/base.py", line 294, in run_from_argv
        self.execute(*args, **cmd_options)
      File "/Users/cnk/.pyenv/versions/d110example-3.5.2/lib/python3.5/site-packages/django/core/management/base.py", line 345, in execute
        output = self.handle(*args, **options)
      File "/Users/cnk/.pyenv/versions/d110example-3.5.2/lib/python3.5/site-packages/django/core/management/commands/test.py", line 72, in handle
        failures = test_runner.run_tests(test_labels)
      File "/Users/cnk/.pyenv/versions/d110example-3.5.2/lib/python3.5/site-packages/django/test/runner.py", line 551, in run_tests
        self.teardown_databases(old_config)
      File "/Users/cnk/.pyenv/versions/d110example-3.5.2/lib/python3.5/site-packages/django/test/runner.py", line 526, in teardown_databases
        connection.creation.destroy_test_db(old_name, self.verbosity, self.keepdb)
      File "/Users/cnk/.pyenv/versions/d110example-3.5.2/lib/python3.5/site-packages/django/db/backends/base/creation.py", line 264, in destroy_test_db
        self._destroy_test_db(test_database_name, verbosity)
      File "/Users/cnk/.pyenv/versions/d110example-3.5.2/lib/python3.5/site-packages/django/db/backends/base/creation.py", line 283, in _destroy_test_db
        % self.connection.ops.quote_name(test_database_name))
      File "/Users/cnk/.pyenv/versions/d110example-3.5.2/lib/python3.5/site-packages/django/db/backends/utils.py", line 64, in execute
        return self.cursor.execute(sql, params)
      File "/Users/cnk/.pyenv/versions/d110example-3.5.2/lib/python3.5/site-packages/django/db/utils.py", line 94, in __exit__
        six.reraise(dj_exc_type, dj_exc_value, traceback)
      File "/Users/cnk/.pyenv/versions/d110example-3.5.2/lib/python3.5/site-packages/django/utils/six.py", line 685, in reraise
        raise value.with_traceback(tb)
      File "/Users/cnk/.pyenv/versions/d110example-3.5.2/lib/python3.5/site-packages/django/db/backends/utils.py", line 62, in execute
        return self.cursor.execute(sql)
    django.db.utils.OperationalError: cannot drop the currently open database


### SQLite3 ###

The generated project used SQLite3 with an in memory database for
testing. This worked fine.

    $ python manage.py test -v 2 polls
    Creating test database for alias 'default' ('file:memorydb_default?mode=memory&cache=shared')...
    Operations to perform:
      Synchronize unmigrated apps: messages, staticfiles
      Apply all migrations: admin, auth, contenttypes, polls, sessions
    Synchronizing apps without migrations:
      Creating tables...
        Running deferred SQL...
    Running migrations:
      Applying contenttypes.0001_initial... OK
      Applying auth.0001_initial... OK
      Applying admin.0001_initial... OK
      Applying admin.0002_logentry_remove_auto_add... OK
      Applying contenttypes.0002_remove_content_type_name... OK
      Applying auth.0002_alter_permission_name_max_length... OK
      Applying auth.0003_alter_user_email_max_length... OK
      Applying auth.0004_alter_user_username_opts... OK
      Applying auth.0005_alter_user_last_login_null... OK
      Applying auth.0006_require_contenttypes_0002... OK
      Applying auth.0007_alter_validators_add_error_messages... OK
      Applying auth.0008_alter_user_username_max_length... OK
      Applying polls.0001_initial... OK
      Applying sessions.0001_initial... OK
    test_detail_view_with_a_future_question (polls.tests.QuestionIndexDetailTests) ... ok
    test_detail_view_with_a_past_question (polls.tests.QuestionIndexDetailTests) ... ok
    test_was_published_recently_with_future_question (polls.tests.QuestionMethodTests) ... ok
    test_was_published_recently_with_old_question (polls.tests.QuestionMethodTests) ... ok
    test_was_published_recently_with_recent_question (polls.tests.QuestionMethodTests) ... ok
    test_index_view_with_a_future_question (polls.tests.QuestionViewTests) ... ok
    test_index_view_with_a_past_question (polls.tests.QuestionViewTests) ... ok
    test_index_view_with_future_question_and_past_question (polls.tests.QuestionViewTests) ... ok
    test_index_view_with_no_questions (polls.tests.QuestionViewTests) ... ok
    test_index_view_with_two_past_questions (polls.tests.QuestionViewTests) ... ok

    ----------------------------------------------------------------------
    Ran 10 tests in 0.043s

    OK
    Destroying test database for alias 'default' ('file:memorydb_default?mode=memory&cache=shared')...


### MySQL ###

I have MySQL version: 5.7.17-0ubuntu0.16.04.1 (Ubuntu) installed on a
Vagrant VM. For this test, I set up the database and database user as follows:

    mysql> create database d110example;
    Query OK, 1 row affected (0.00 sec)

    mysql> grant all on d110example.* to 'd110example'@'%' identified by 'd110example';
    Query OK, 0 rows affected, 1 warning (0.00 sec)

    mysql> grant all on test_d110example.* to 'd110example'@'%' identified by 'd110example';
    Query OK, 0 rows affected, 1 warning (0.00 sec)

    mysql> show grants for d110example;
    +-------------------------------------------------------------------+
    | Grants for d110example@%                                          |
    +-------------------------------------------------------------------+
    | GRANT USAGE ON *.* TO 'd110example'@'%'                           |
    | GRANT ALL PRIVILEGES ON `d110example`.* TO 'd110example'@'%'      |
    | GRANT ALL PRIVILEGES ON `test_d110example`.* TO 'd110example'@'%' |
    +-------------------------------------------------------------------+
    3 rows in set (0.00 sec)

    mysql> select db, user, host from db;
    +------------------+-------------+-----------+
    | db               | user        | host      |
    +------------------+-------------+-----------+
    | d110example      | d110example | %         |
    | test_d110example | d110example | %         |
    | sys              | mysql.sys   | localhost |
    +------------------+-------------+-----------+
    3 rows in set (0.00 sec)

    mysql> show databases;
    +--------------------+
    | Database           |
    +--------------------+
    | information_schema |
    | d110example        |
    | mysql              |
    | performance_schema |
    | sys                |
    +--------------------+
    5 rows in set (0.01 sec)

Test works fine - creates and drops test_d110example just fine:

    $ python manage.py test -v 2 polls
    python manage.py test -v 2 polls
    Creating test database for alias 'default' ('test_d110example')...
    Operations to perform:
      Synchronize unmigrated apps: messages, staticfiles
      Apply all migrations: admin, auth, contenttypes, polls, sessions
    Synchronizing apps without migrations:
      Creating tables...
        Running deferred SQL...
    Running migrations:
      Applying contenttypes.0001_initial... OK
      Applying auth.0001_initial... OK
      Applying admin.0001_initial... OK
      Applying admin.0002_logentry_remove_auto_add... OK
      Applying contenttypes.0002_remove_content_type_name... OK
      Applying auth.0002_alter_permission_name_max_length... OK
      Applying auth.0003_alter_user_email_max_length... OK
      Applying auth.0004_alter_user_username_opts... OK
      Applying auth.0005_alter_user_last_login_null... OK
      Applying auth.0006_require_contenttypes_0002... OK
      Applying auth.0007_alter_validators_add_error_messages... OK
      Applying auth.0008_alter_user_username_max_length... OK
      Applying polls.0001_initial... OK
      Applying sessions.0001_initial... OK
    test_detail_view_with_a_future_question (polls.tests.QuestionIndexDetailTests) ... ok
    test_detail_view_with_a_past_question (polls.tests.QuestionIndexDetailTests) ... ok
    test_was_published_recently_with_future_question (polls.tests.QuestionMethodTests) ... ok
    test_was_published_recently_with_old_question (polls.tests.QuestionMethodTests) ... ok
    test_was_published_recently_with_recent_question (polls.tests.QuestionMethodTests) ... ok
    test_index_view_with_a_future_question (polls.tests.QuestionViewTests) ... ok
    test_index_view_with_a_past_question (polls.tests.QuestionViewTests) ... ok
    test_index_view_with_future_question_and_past_question (polls.tests.QuestionViewTests) ... ok
    test_index_view_with_no_questions (polls.tests.QuestionViewTests) ... ok
    test_index_view_with_two_past_questions (polls.tests.QuestionViewTests) ... ok

    ----------------------------------------------------------------------
    Ran 10 tests in 0.102s

    OK
    Destroying test database for alias 'default' ('test_d110example')...
