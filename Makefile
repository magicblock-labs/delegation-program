build:
	@cargo build-sbf

test:
	RUST_LOG=off cargo test-sbf --features unit_test_config

lint:
	cargo clippy -- -D warnings

fmt:
	cargo +nightly fmt
