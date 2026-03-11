build:
	@cargo build-sbf

test:
	RUST_LOG=off cargo test-sbf --features unit_test_config

lint:
	cargo clippy --features sdk,program -- -D warnings

fmt:
	cargo +nightly fmt
