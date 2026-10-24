Reference
=========


It checks if the authentication backend stored in the session is present in
:setting:`AUTHENTICATION_BACKENDS`. If so, it uses the backend's
``get_user()`` method to retrieve the user model instance and then verifies
the session by calling the user model's
:meth:`~django.contrib.auth.models.AbstractBaseUser.get_session_auth_hash`
method. If the verification fails and :setting:`SECRET_KEY_FALLBACKS` are
provided, it verifies the session against each fallback key using
:meth:`~django.contrib.auth.models.AbstractBaseUser.\
get_session_auth_fallback_hash`.
