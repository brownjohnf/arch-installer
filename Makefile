.DEFAULT_GOAL := build

t ?=

.PHONY: build
build: target/doc
	cargo build

.PHONY: test
test:
	cargo test --no-fail-fast $(t) -- --nocapture

.PHONY: release
release: test
	cargo build --release

.PHONY: doc
target/doc:
	cargo doc

.PHONY: clean
clean:
	cargo clean

.PHONY: version
version:
	verto
