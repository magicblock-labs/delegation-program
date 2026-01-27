build:
	@cargo build-sbf

test:
	RUST_LOG=off cargo test-sbf --features unit_test_config

fmt:
	cargo +nightly fmt
