#!/usr/bin/env bash

pwd = ${shell pwd}
app-name = wayshot
version = `git describe --tags --abbrev=0`

# windows, wayland-portal, wayland-wlr
features ?= wayland-wlr
linux-app-postfix = $(if $(filter wayland-portal,$(features)), portal, wlr)
run-env = RUST_LOG=debug
build-env = SLINT_STYLE=fluent CMAKE_POLICY_VERSION_MINIMUM=3.5
proj-features = --features=${features},database,qrcode,center-window

all: build-release

build:
	$(build-env) cargo build --bin ${app-name} --no-default-features --features=${features}

build-release:
	$(build-env) cargo build --release --bin ${app-name} --no-default-features --features=${features}

debug:
	$(build-env) $(run-env) cargo run --bin ${app-name} --no-default-features --features=${features}

debug-winit:
	SLINT_BACKEND=winit-femtovg $(build-env) $(run-env) cargo run --bin ${app-name} --no-default-features --features=${features}

run-release:
	$(build-env) RUST_LOG=info cargo run --release --bin ${app-name} --no-default-features --features=${features}

run-release-winit:
	SLINT_BACKEND=winit-femtovg $(build-env) RUST_LOG=info cargo run --release --bin ${app-name} --no-default-features --features=${features}

cursor-debug:
	$(run-env) cargo run -p wayshot-cursor --bin wayshot-cursor

cursor-release:
	cargo build --release -p wayshot-cursor --bin wayshot-cursor

tr:
	cargo run -p tr-helper --bin tr-helper

icon:
	cargo run -p icon-helper --bin icon-helper -- -i ${app-name}/ui/images -o ${app-name}/ui/base

icon-strip:
	cargo run -p icon-helper --bin icon-helper -- -i ${app-name}/ui/images -o ${app-name}/ui/base --strip

slint-viewer:
	$(build-env) slint-viewer --auto-reload -I $(app-name)/ui ${app-name}/ui/desktop-window.slint

clippy:
	cargo clippy $(proj-features)

check:
	$(build-env) cargo check --no-default-features $(proj-features) --bin ${app-name}

clean:
	cargo clean

packing-windows:
	cp -f target/release/${app-name}.exe target/${app-name}-${version}-x86_64-windows.exe

packing-linux: linux-bin deb appimage flatpak

linux-bin:
	- rm -f target/${app-name}-${version}-x86_64-linux-*.tar.gz
	tar -zcf target/${app-name}-${version}-x86_64-linux-${linux-app-postfix}.tar.gz -C target/release ${app-name}

deb:
	- rm -f target/${app-name}-${version}-x86_64-linux-*.deb
	cd package/deb && bash -e "./pkg-deb.sh"
	mv package/deb/$(app-name).deb target/${app-name}-${version}-x86_64-linux-${linux-app-postfix}.deb

appimage:
	- rm -f target/${app-name}-${version}-x86_64-linux-*.AppImage
	cd package/appimage && bash -e "./pkg-appimage.sh"
	mv package/appimage/$(app-name).AppImage target/${app-name}-${version}-x86_64-linux-${linux-app-postfix}.AppImage

flatpak:
	- rm -f target/${app-name}-${version}-x86_64-linux-*.flatpak
	cd package/flatpak && bash -e "./pkg-flatpak.sh"
	mv package/flatpak/$(app-name).flatpak target/${app-name}-${version}-x86_64-linux-${linux-app-postfix}.flatpak

app-name:
	echo "$(app-name)" > target/app-name

get-font-name:
	fc-scan ./${app-name}/ui/fonts/*.{ttf,otf} | grep "fullname:"

