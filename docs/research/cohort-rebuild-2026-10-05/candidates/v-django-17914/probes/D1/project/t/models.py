from django.db import models


class Item(models.Model):
    owner = models.CharField(max_length=50)
