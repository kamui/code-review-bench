#!/usr/bin/env bash
source "$(dirname "$0")/req.sh" "$1"
run "login (static)" "/login"
run "account direct, no cookie (expect 302->/login)" "/account"
run "account direct, cookie=alice (expect 200)" "/account" "session=alice"
