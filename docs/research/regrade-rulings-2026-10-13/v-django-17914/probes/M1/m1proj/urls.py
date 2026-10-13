from django.db import connection
from django.http import HttpResponse
from django.urls import path


def q(request):
    with connection.cursor() as cursor:
        cursor.execute("SELECT 1")
        return HttpResponse(f"SELECT 1 gave {cursor.fetchone()}")


urlpatterns = [path("q", q)]
