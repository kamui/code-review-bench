"""Write one fix variant of the head's src/requests/adapters.py in place.

    python make-variants.py VARIANT PATH

lazy-verify-true  the load moves into a cached helper that _urllib3_request_context calls for verify=True
lazy              the same helper, called only for https:// URLs (the smallest deferral)
guard             the import-time load is kept; FileNotFoundError leaves no shared context, and
                  cert_verify raises the base's OSError for a default-verified https:// request
guard-oserror     as guard, catching every OSError
both              lazy, with the base's OSError raised by the helper when the default file is missing
both-verify-true  lazy-verify-true, with that OSError: the shape the three comments propose
"""
import sys, pathlib
variant, path = sys.argv[1], pathlib.Path(sys.argv[2])
src = path.read_text()
BLOCK = '''_preloaded_ssl_context = create_urllib3_context()
_preloaded_ssl_context.load_verify_locations(
    extract_zipped_paths(DEFAULT_CA_BUNDLE_PATH)
)
'''
USE = '''    elif verify is True:
        pool_kwargs["ssl_context"] = _preloaded_ssl_context
'''
VERIFY = '''            if verify is not True:
                # `verify` must be a str with a path then
'''
assert BLOCK in src and USE in src and VERIFY in src
HELPER = '''_preloaded_ssl_context = None


def _get_preloaded_ssl_context():
    global _preloaded_ssl_context
    if _preloaded_ssl_context is None:
        cert_loc = extract_zipped_paths(DEFAULT_CA_BUNDLE_PATH){check}
        context = create_urllib3_context()
        context.load_verify_locations(cert_loc)
        _preloaded_ssl_context = context
    return _preloaded_ssl_context
'''
CHECK = '''
        if not os.path.exists(cert_loc):
            raise OSError(
                f"Could not find a suitable TLS CA certificate bundle, "
                f"invalid path: {cert_loc}"
            )'''
if variant in ("lazy-verify-true", "both-verify-true"):
    src = src.replace(BLOCK, HELPER.format(check=CHECK if variant == "both-verify-true" else ""))
    src = src.replace(USE, '''    elif verify is True:
        pool_kwargs["ssl_context"] = _get_preloaded_ssl_context()
''')
elif variant in ("lazy", "both"):
    src = src.replace(BLOCK, HELPER.format(check=CHECK if variant == "both" else ""))
    src = src.replace(USE, '''    elif verify is True:
        if scheme == "https":
            pool_kwargs["ssl_context"] = _get_preloaded_ssl_context()
''')
elif variant in ("guard", "guard-oserror"):
    exc = "FileNotFoundError" if variant == "guard" else "OSError"
    src = src.replace(BLOCK, f'''_preloaded_ssl_context = create_urllib3_context()
try:
    _preloaded_ssl_context.load_verify_locations(
        extract_zipped_paths(DEFAULT_CA_BUNDLE_PATH)
    )
except {exc}:
    _preloaded_ssl_context = None
''')
    src = src.replace(VERIFY, '''            if verify is True and _preloaded_ssl_context is None:
                raise OSError(
                    f"Could not find a suitable TLS CA certificate bundle, "
                    f"invalid path: {extract_zipped_paths(DEFAULT_CA_BUNDLE_PATH)}"
                )
''' + VERIFY)
else:
    raise SystemExit("unknown variant")
path.write_text(src)
