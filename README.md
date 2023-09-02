# Sea of Stars Demo Auto-splitter

Archived! Use the regular autosplitter: [knutwalker/sea-of-stars-autosplitter](https://github.com/knutwalker/sea-of-stars-autosplitter)

An auto splitter for the demo of Sea Of Stars.

## Download

The latest version of the auto splitter can be downloaded from:

    https://github.com/knutwalker/sea-of-stars-demo-autosplitter/releases/download/latest/sea_of_stars_demo_autosplitter.wasm


## Compilation

This auto splitter is written in Rust. In order to compile it, you need to
install the Rust compiler: [Install Rust](https://www.rust-lang.org/tools/install).

Afterward install the WebAssembly target:
```sh
rustup target add wasm32-unknown-unknown --toolchain stable
```

The auto splitter can now be compiled:
```sh
cargo b
```

The auto splitter is then available at:
```
target/wasm32-unknown-unknown/release/sea_of_stars_demo_autosplitter.wasm
```

Make sure to look into the [API documentation](https://livesplit.org/asr/asr/) for the `asr` crate.

You can use the [debugger](https://github.com/CryZe/asr-debugger) while
developing the auto splitter to more easily see the log messages, statistics,
dump memory and more.
