# src-new

Temporary CLI wiring area for pipeline cutover.

Current handoff:
- `extract` crawls source pages and emits `load_seeds` queue rows.
- `load` consumes pending `load_seeds` rows when `--url` is omitted.
