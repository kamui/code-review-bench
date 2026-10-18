#!/usr/bin/env python3
"""Prints the header and the build facts of a cases.mjs output, then only the cases that carry one of the tags.
usage: python3 -I select.py <all-state.txt> <tag> [<tag> ...]"""
import re
import sys

text = open(sys.argv[1], encoding="utf-8").read().splitlines()
tags = set(sys.argv[2:])
keep = True
for line in text:
    m = re.match(r"^(\S+) \[([^\]]*)\]\. ", line)
    if m:
        keep = bool(tags & set(m.group(2).split()))
    if keep or line.startswith("#") or not line.strip():
        print(line)
