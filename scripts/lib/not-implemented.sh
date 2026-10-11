# Sourced by scripts that belong to a later prompt. They fail loudly rather than pretend to pass.
# usage: not_implemented "<script name>" "<prompt number and title>"
not_implemented() {
  echo "$1 is not implemented yet. It arrives in Prompt $2 of docs/implementation/vellum-implementation-guide.md." >&2
  exit 1
}
