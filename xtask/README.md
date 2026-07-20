# Underwood xtask

`xtask` owns dependency-free repository policy validation. It explicitly does
not own product behavior or architectural decisions.

Run every check:

```sh
cargo xtask check
```

Individual checks are available as `proof`, `repo`, and `beads`.
