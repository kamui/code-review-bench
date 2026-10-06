print -r -- "unbraced_stack=$funcstack[1]"
print -r -- "unbraced_compdef_presence=$+functions[compdef]"
print -r -- "braced_compdef_presence=${+functions[compdef]}"
if (( ! $+functions[compdef] )); then
  print -r -- THEN_BRANCH
else
  print -r -- ELSE_BRANCH
fi
