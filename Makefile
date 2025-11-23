.PHONY: all clean test install

PREFIX=/usr/local
BINDIR=$(PREFIX)/bin

all: target/release/versort

target/release/versort: Cargo.toml
	@cargo build --release

clean:
	rm -rf target

test: target/release/versort
	@tests/test.sh

install: target/release/versort
	install -Dm755 target/release/versort -t $(DESTDIR)$(BINDIR)
