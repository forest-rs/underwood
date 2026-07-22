# IME compatibility experiment

This external proof drives Underwood's real Parley-backed composition path in
the two protocol shapes required by Design-0009:

- a Winit-like event feed begins composition, replaces complete preedit
  snapshots, moves the marked selection, cancels, and commits; and
- a host-driven adapter synchronously queries the same epoch for focused text,
  marked and selected ranges, UTF-16 conversion, caret/range geometry, and a
  point-to-offset hit, and maps an explicit host replacement range back into a
  scene-validated semantic selection.

The trace deliberately starts from two independent scene selections. A native
marked region can represent only one composition target, so the session reports
its explicit normalization to the primary extent. This is an adapter constraint,
not a reduction of Underwood's multi-selection scene model.

Run it with:

```sh
cargo run -p underwood_ime_compat_experiment
```

This is correctness evidence, not a product benchmark or a platform adapter.
It adds no dependency to either production crate.
