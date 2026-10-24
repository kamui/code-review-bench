Target
======

.. currentmodule:: django.contrib.auth

.. class:: models.AbstractBaseUser

    .. method:: models.AbstractBaseUser.get_session_auth_fallback_hash()

        .. versionadded:: 4.1.8

        Yields the HMAC of the password field using
        :setting:`SECRET_KEY_FALLBACKS`. Used by ``get_user()``.
