#!/bin/sh
# Usage: searches-head.sh <head checkout> <clone with base and head> <output directory>
# The searches of the project's documentation at the head and of the change itself, for M1 and C2n.
# Every command is echoed before its output.
set -u
head=$1; repo=$2; out=$3
base=bcccea3ef31c777b73cba41a6255cd866bf87237
headsha=fad334e1a9b54ea1acb8cce02a25934c5acfe99f
show() { echo "\$ $*"; ( cd "$head" && "$@" ); echo; }
showdiff() { echo "\$ git diff $base $headsha | $*"; ( cd "$repo" && git diff $base $headsha | "$@" ); echo; }
{
  echo "M1, project documentation at the head $headsha"
  show rg -n -i "psycopg2" docs/ref/databases.txt docs/releases/5.1.txt docs/releases/4.2.txt docs/topics/install.txt docs/ref/settings.txt docs/faq
  show rg -n -i "pool" docs/ref/databases.txt docs/releases/5.1.txt
  show rg -n -i "psycopg_pool|psycopg-pool|psycopg\[pool\]" docs
} > "$out/M1-project-docs.txt"
{
  echo "M1, the change: description (empty, see pr-17914.json and pr-17914-description-edits.graphql.json), commit message, and the diff"
  echo "\$ git log -1 --format=%B $headsha"; ( cd "$repo" && git log -1 --format=%B $headsha ); echo
  showdiff rg -n -i "psycopg2|psycopg_pool|psycopg\[pool\]|psycopg-pool|requires psycopg|is_psycopg3"
} > "$out/M1-change.txt"
{
  echo "C2n, project documentation at the head $headsha"
  show rg -n -i "assume_role|database-role" docs
  show rg -n -i "thread" docs/ref/databases.txt
  show rg -n -i "alter.*settings|settings at runtime|only place you should assign" docs/topics/settings.txt docs/ref/settings.txt docs/ref/databases.txt
  show rg -n -i "thread.sharing|created in a thread|inc_thread_sharing" docs
} > "$out/C2n-project-docs.txt"
{
  echo "C2n, the change: description (empty), commit message, and the diff"
  echo "\$ git log -1 --format=%B $headsha"; ( cd "$repo" && git log -1 --format=%B $headsha ); echo
  showdiff rg -n -i "assume_role|ensure_role|compose_sql|_configure_connection|access anything on self|aside from|no_pool_connection\(\)|thread"
} > "$out/C2n-change.txt"
