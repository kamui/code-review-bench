#!/usr/bin/env bash
# usage: run.sh <mirror w-graphql-js-3457.git> <unpacked node_modules dir> <work dir>
# Measures c8 coverage of OverlappingFieldsCanBeMergedRule.ts at the base, at the head,
# and at each with only its `/* c8 ignore next */` hints removed (the controls).
set -u
mirror=$1; nm=$2; work=$3
base=730d5af8e933235fd5aa312a00be465db0b8acf5
head=efdbbcfacd92adb78e002f2b8a61f2a6a1504c19
rule=src/validation/rules/OverlappingFieldsCanBeMergedRule.ts
for t in base head; do
  rev=$base; [ "$t" = head ] && rev=$head
  for v in "$t" "$t-nohint"; do mkdir -p "$work/$v"; git -C "$mirror" archive "$rev" | tar x -C "$work/$v"; done
  sed -i 's#/\* c8 ignore next \*/ ##g' "$work/$t-nohint/$rule"
done
echo "hint lines:"
for t in base base-nohint head head-nohint; do echo "  $t:"; grep -n 'arguments ??' "$work/$t/$rule" | sed 's/^/    /'; done
echo "codecov.yml at head:"; sed 's/^/  /' "$work/head/codecov.yml"
for t in base base-nohint head head-nohint; do
  ln -s "$nm" "$work/$t/node_modules"
  echo "=== $t"
  # source-map 0.7 inside c8 7.10 mistakes Node 18's global fetch for a browser; disable it.
  ( set -o pipefail; cd "$work/$t" && node --no-experimental-fetch ./node_modules/.bin/c8 \
      --temp-directory "$work/c8tmp-$t" --report-dir "$work/c8rep-$t" \
      --all false --skip-full false --include "$rule" --reporter text \
      --check-coverage --branches 100 --lines 100 --statements 100 --functions 100 \
      ./node_modules/.bin/mocha 'src/validation/__tests__/OverlappingFieldsCanBeMergedRule-test.ts' 2>&1 \
      | sed -n '/passing/,$p' )
  echo "c8 exit: $?"
done
