# Underwood

`underwood` is the dependency-free, renderer-independent foundation for
Underwood's semantic document, preparation, flow, and scene contracts.

The crate is `no_std`. It owns no shaping engine, platform host policy, graphics
backend, or renderer. Parley integration will live behind the separately gated
`underwood_parley` adapter crate.

The package boundary is ratified, but its first public product contract is not.
No product item is exported until the complete API call site, ownership model,
errors, rustdoc, and migration posture pass the human gate in Design-0001.
