PREFIX ?= $(HOME)/.local/bin

all: build

build:
	cargo build --release

test:
	cargo test

clean:
	cargo clean

install: build
	mkdir -p $(PREFIX)
	cp target/release/keyboard-server $(PREFIX)/

.PHONY: all build test clean install
