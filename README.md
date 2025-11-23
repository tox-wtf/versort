# Versort

A small utility to sort semantic(ish) versions.

## Building
Versort can be built and installed like so:
```bash
make
make PREFIX=/usr/local install
```

Tests can be run with:
```bash
make test
```

## Usage
Versort reads newline-delimited versions from stdin.

```bash
git ls-remote --tags --refs https://github.com/tox-wtf/vagrant |
    sed 's,.*/,,' | shuf |
    versort
```

```bash
git ls-remote --tags --refs https://github.com/python/cpython |
    sed -e 's,.*/,,' -e 's,^v,,' | shuf |
    versort -i # ignore semvers that couldn't be parsed
```

```bash
git ls-remote --tags --refs https://github.com/tmux/tmux |
    sed -e 's,.*/,,' | shuf |
    versort -c # treat a single char at the end as a counter
```
