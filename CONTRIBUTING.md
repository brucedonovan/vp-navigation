# Contributing

The core is Rust and has no rendering dependencies. The demo is a Vite app that talks to it through WebAssembly.

## Tests

```bash
cargo test -p vp-navigation
```

Projection and motion changes belong in `crates/vp-navigation`. If a behavior is something a host should be able to tune, add it to `Params` in `crates/vp-navigation/src/params.rs` and document the field in [docs/configuration.md](docs/configuration.md). The config JSON and `setProjection` pick it up from there. Missing fields keep their default.

## The demo

Rebuild the wasm package after a core change, then run the demo:

```bash
wasm-pack build crates/vp-navigation-wasm --target web --out-dir ../../packages/dom/pkg
cd demo
npm install
npm run dev
```

`packages/dom/pkg` is build output. Do not commit it.

The demo's content is one example, The Long Room. Library code should not assume those stop ids, labels, or colors. A new behavior has to work for any graph a host builds.
