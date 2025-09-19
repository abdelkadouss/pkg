set shell := ["pkgx", "+nushell.sh", "nu@0.107.0", "-c"]

_default:
    @just --list

dev-build:
    docker build -t pkg:dev -f Dockerfile.dev .

dev:
    #!/usr/bin/env nu
    (
      docker run -it --rm
      -v (pwd)/src:/app/src          # Mount your source code
      -v (pwd)/Cargo.toml:/app/Cargo.toml
      -v (pwd)/Cargo.lock:/app/Cargo.lock
      -v (pwd)/docs:/app/docs        # Mount your docs
      -v (pwd)/examples/docker:/root/.config/pkg # Mount your examples
      pkg:dev                          # Use the dev image
    )

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
