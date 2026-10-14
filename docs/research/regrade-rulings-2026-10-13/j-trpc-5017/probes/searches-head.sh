#!/bin/sh
# Usage: searches-head.sh <checkout-of-head> <upstream-dir>
# <checkout-of-head> holds www/docs, packages/tests, examples and the READMEs of trpc/trpc at
# 7dc04a7e94654dfad6ef1289dfe01a0a206fff3b (git archive). Runs the documentation and test
# searches of each group with ripgrep and saves each query with its output.
set -u
head=$(cd "$1" && pwd); out=$(cd "$2" && pwd)
cd "$head"
DOCS="www/docs README.md packages/server/README.md packages/client/README.md"
q() { file=$1; shift; { echo "\$ $*"; "$@"; echo "[exit $?; $("$@" 2>/dev/null | wc -l) line(s)]"; echo; } >> "$out/$file"; }
start() { : > "$out/$1"; echo "# Searches of trpc/trpc at the head 7dc04a7e, run with ripgrep $(rg --version | head -1 | cut -d' ' -f2) from the root of the checkout." >> "$out/$1"; echo >> "$out/$1"; }

# ---- M1: an input that is a union of objects, followed by a middleware
start M1-project-docs.txt
q M1-project-docs.txt rg -n -i "discriminatedUnion|z\.union|\bunion\b" $DOCS
q M1-project-docs.txt rg -n -i "infer the types of inputs|properly typed|first class support" www/docs/server
q M1-project-docs.txt rg -n -U "\.input\([^;]*?\)\s*\.use\(" www/docs
q M1-project-docs.txt rg -n -i "can be stacked|common input" www/docs
q M1-project-docs.txt rg -c "\.input\(" www/docs
start M1-project-tests.txt
q M1-project-tests.txt rg -n "discriminatedUnion|z\.union\(|\.or\(" packages/tests examples
q M1-project-tests.txt rg -n -U "\.input\([^;]*?\)\s*\.use\(" packages/tests

# ---- M2: two optional object inputs merged by unstable_concat or a standalone middleware
start M2-project-docs.txt
q M2-project-docs.txt rg -n "unstable_concat|concat" $DOCS
q M2-project-docs.txt rg -n -i "unstable_|experimental" www/docs/further/faq.mdx www/docs/server
q M2-project-docs.txt rg -n -A 12 "Am I safe with using features marked as as" www/docs/further/faq.mdx
q M2-project-docs.txt rg -n "\.optional\(\)\)|\.nullish\(\)\)|\.optional\(\),\s*\)|\.nullish\(\),\s*\)" www/docs
q M2-project-docs.txt rg -n -i "can be stacked|Cannot chain an optional" www/docs packages/server/src
start M2-project-tests.txt
q M2-project-tests.txt rg -n "unstable_concat" packages/tests examples packages/server/src
q M2-project-tests.txt rg -n -B 3 "unstable_concat<" packages/server/src/core/internals/procedureBuilder.ts
q M2-project-tests.txt rg -n "experimental_standaloneMiddleware" packages/tests examples
q M2-project-tests.txt rg -n -U "\.input\(\s*z\s*\.object\([^;]*?\)\s*\.(optional|nullish)\(\)" packages/tests examples

# ---- M3: a root context typed `any`, read inside a t.middleware() function
start M3-project-docs.txt
q M3-project-docs.txt rg -n "context<any>|Promise<any>|: any\b|as any\b" www/docs/server www/docs/main www/docs/client
q M3-project-docs.txt rg -n "initTRPC\.context<" www/docs
q M3-project-docs.txt rg -n -i "properly typed|explicitly defined|inferred from a function" www/docs/server/context.md
q M3-project-docs.txt rg -n "t\.middleware\(" www/docs
start M3-project-tests.txt
q M3-project-tests.txt rg -n "context<any>|context<\{\s*\[|createContext[^\n]*Promise<any>" packages/tests examples
q M3-project-tests.txt rg -n "initTRPC\s*\.context<" packages/tests examples --count-matches

# ---- C1n: two .input() calls in a row, and optional context keys
start C1n-project-docs.txt
q C1n-project-docs.txt rg -n -i "can be stacked" -A 30 www/docs/server/validators.md
q C1n-project-docs.txt rg -n "OverwriteIfDefined" -B 2 -A 4 packages/server/src/core/internals/procedureBuilder.ts
q C1n-project-docs.txt rg -n -i "readonly" www/docs/server
