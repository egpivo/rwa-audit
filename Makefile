.PHONY: sync build test collect activity flow exchange-freeze freeze figures audit

sync:
	./scripts/git-sync.sh

build:
	cargo build --release

test:
	cargo test

collect:
	cargo run --bin rwa-audit -- run registry --mode live

activity:
	cargo run --bin rwa-audit -- run activity --mode live

flow-panel:
	cargo run --bin rwa-audit -- run flow-panel --mode live

flow-quotes:
	cargo run --bin rwa-audit -- run flow-quotes --mode live

exchange-freeze:
	cargo run --bin rwa-exchange-freeze

freeze:
	cargo run --bin rwa-audit -- run exchange

audit:
	cargo run --bin rwa-audit -- help

figures:
	python scripts/plot/fig4_xstocks_surface.py
