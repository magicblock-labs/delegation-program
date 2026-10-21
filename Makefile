build:
	@cargo build-sbf --manifest-path program/Cargo.toml

test:
	RUST_LOG=off cargo test-sbf --manifest-path program/Cargo.toml --features unit_test_config

integration-test: build
	./scripts/run-anchor-tests.sh

lint:
	cargo clippy --workspace -- -D warnings

fmt:
	cargo +nightly fmt
