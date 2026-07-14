#!/bin/bash

docker run -it --rm \
    -e HOME=/home \
    -e XDG_CONFIG_HOME=/home/.config \
    -v $PWD/src:/app/src \
    -v $PWD/example:/app/example \
    -v $PWD/Cargo.toml:/app/Cargo.toml \
    -v $PWD/Cargo.lock:/app/Cargo.lock \
    -v $PWD/example/usege:/home/.config/pkg \
    pkg:latest
