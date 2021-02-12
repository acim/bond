.PHONY : lint
lint:
	cargo fmt --all -- --check
	cargo clippy -- -D warnings
