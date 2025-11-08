all: build

build:
	cargo build --release

test: all
	@tests/test.sh

install:
	install -Dm755 target/release/versort -t /usr/bin/
