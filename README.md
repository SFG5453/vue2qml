# vue2qml

`vue2qml` converts Vue 3 single-file components into a self-contained QML component tree. It is written in dependency-free Rust 2024 and is driven by real Vue syntax rather than regular-expression replacement.

The generated tree includes a small QML compatibility runtime, preserves component composition and Vue expressions, and can be checked with Qt's own tooling. Project conversion mirrors the source directory structure and resolves local component imports automatically.

## Usage

```sh
cargo run --release -- <input.vue-or-project> <output-directory>
cargo run --release -- check <input.vue-or-project>
cargo run --release -- verify <input.vue-or-project> <output-directory>
```

`verify` performs the conversion, parses every output with `qmlformat`, runs `qmllint` without accepting diagnostics, and smoke-tests `src/App.qml` offscreen when that entrypoint exists. It requires Qt 6 command-line tools on `PATH`.

This converts all 57 Orchard SFCs into 57 QML components plus the two-file compatibility runtime.

## Translation contract

The converter currently handles:

- top-level `<template>`, normal `<script>`, `<script setup>`, `<style>`, and custom SFC blocks;
- nested and multi-root Vue templates, comments, Unicode, and interpolated text;
- Options API and `defineProps` property declarations, defaults, required metadata, and `.vue` imports;
- custom components, including globally used components resolved from the project corpus;
- `v-if`, `v-else-if`, `v-else`, `v-show`, `v-for`, `v-model`, `v-html`, slots, refs, keys, and custom directives;
- shorthand and longhand bindings, events, event modifiers, and multiple handlers for one event;
- QML-incompatible JavaScript syntax through an explicit source-expression fallback rather than invalid output;
- a warning-free Rust build and a hard 500-line ceiling for every Rust source file.

Generated elements retain their original tag, static attributes, dynamic bindings, directive metadata, event behavior, and source component identity. Local Vue component props become typed QML properties. State returned through `...props.app` is bridged through the runtime and Vue refs are unwrapped on access.

The converter does not attempt to make browser-only APIs such as the DOM, Electron preload objects, Web Audio, or CSS layout engines native Qt APIs. Those services must be supplied through the `app` model or ported separately. Embedded SFC styles are preserved as component metadata; project-global CSS remains an application integration concern.

## Quality gates

```sh
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets
```

The project intentionally has no third-party crates, so there are no dependency versions that can become stale. `Cargo.toml` selects edition 2024 and denies Rust warnings and Clippy's default lint group.

## AI
- Yes this is ai generated.