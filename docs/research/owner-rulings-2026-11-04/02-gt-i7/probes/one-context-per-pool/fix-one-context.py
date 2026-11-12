"""Writes a patched copy of the head's adapters.py in which every verify=True pool gets its own new
context, loaded with the default bundle, instead of the module-level _preloaded_ssl_context.
Nothing else changes. Usage: python -I fix-one-context.py <head src/requests/adapters.py> <out path>"""
import sys
s = open(sys.argv[1]).read()
old = '''    elif verify is True:
        pool_kwargs["ssl_context"] = _preloaded_ssl_context'''
new = '''    elif verify is True:
        own = create_urllib3_context()
        own.load_verify_locations(extract_zipped_paths(DEFAULT_CA_BUNDLE_PATH))
        pool_kwargs["ssl_context"] = own'''
assert s.count(old) == 1
open(sys.argv[2], "w").write(s.replace(old, new, 1))
