.PHONY: build test test-fast fmt lint install doctor e2e verify-demo check clean

build:
	cargo build

test:
	cargo test --all-targets

test-fast:
	cargo nextest run --all-targets

fmt:
	cargo fmt --all --check

lint:
	cargo clippy --all-targets -- -D warnings

check:
	cargo check --all-targets

install:
	cargo install --path . --locked

doctor:
	cargo run -- doctor

e2e:
	cargo test --test e2e_fixture -- --nocapture

verify-demo:
	./scripts/verify-demo.sh

clean:
	cargo clean
