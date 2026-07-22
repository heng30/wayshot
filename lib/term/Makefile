#!/usr/bin/env bash

pwd = ${shell pwd}
app-name = slint-term

run-env = RUST_LOG=debug
build-env = SLINT_STYLE=fluent

all: build-release

build:
	$(build-env) cargo build

build-release:
	$(build-env) cargo build --release

debug:
	$(build-env) $(run-env) cargo run --example ${app-name}

run-release:
	$(build-env) $(run-env) cargo run --release --example ${app-name}

slint-viewer:
	$(build-env) slint-viewer --auto-reload -I examples/ui examples/ui/app.slint

check:
	$(build-env) cargo check

clean:
	cargo clean

