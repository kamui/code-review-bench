import os

from django.conf import settings


def config(engine, options=None):
    return {
        'ENGINE': engine,
        'NAME': 'postgres',
        'USER': 'dossier',
        'HOST': '127.0.0.1',
        'PORT': '55439',
        'OPTIONS': options or {},
    }


settings.configure(USE_TZ=True, DATABASES={
    'default': config('django.db.backends.postgresql'),
    'feature': config('feature_backend'),
    'role_override': config('role_backend'),
    'documented_role': config('django.db.backends.postgresql', {'assume_role': 'dossier_application'}),
})
import django
django.setup()
from django.db import connections

print('revision:', os.environ['DOSSIER_REVISION'])
with connections['default'].cursor() as cursor:
    cursor.execute('CREATE ROLE dossier_application')
for alias in ['feature', 'role_override', 'documented_role']:
    wrapper = connections[alias]
    with wrapper.cursor() as cursor:
        cursor.execute('SELECT current_user, 1')
        print(alias, 'query:', cursor.fetchone())
    if alias == 'feature':
        print('documented feature override:', wrapper.features.allows_group_by_selected_pks_on_model(None))
    if alias == 'role_override':
        print('role override calls:', wrapper.role_calls)
    wrapper.close()
with connections['default'].cursor() as cursor:
    cursor.execute('DROP ROLE dossier_application')
connections.close_all()
