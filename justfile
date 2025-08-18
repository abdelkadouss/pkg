set shell := ["nu", "-c"]

_default:
    @just --list

run:
    cargo run

build:
    cargo build --release

test:
    cargo test

clean:
    cargo clean

fmt:
    cargo fmt

watch_script script:
    #!/usr/bin/env nu

    watchexec -w src Cargo.toml -r just {{ script }}

lint *watch="false":
    #!/usr/bin/env nu
    if {{ watch }} {
      just watch_script lint

    } else {
      cargo clippy

    }

watch:
    just watch_script run
