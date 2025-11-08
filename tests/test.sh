#!/usr/bin/env bash

set +e -u

failed=0
passed=0
i=0

SCRIPT_DIR=$(dirname "$(readlink -f "$0")")
VERSORT="${VERSORT:-"$SCRIPT_DIR/../target/release/versort"}"

teststatus() {
    case "$1" in
        0)
            printf "PASS\n"
            ((passed++))
        ;;
        *)
            printf "FAIL\n"
            ((failed++))
        ;;
    esac
}

runtest() {
    < "$1" "$VERSORT" $(<"$1.flags") | diff - --color=always -u "$1.out" >&2
}

for t in "$SCRIPT_DIR"/*.t; do
    ((i++))
    testname="$(echo "$t" | sed -e 's,.*/,,' -e 's,\.t,,')"
    printf "$i. %-37s" "$testname"
    runtest "$t"
    teststatus "$?"
done

if [ $failed -gt 0 ]; then
    echo "$failed tests failed" >&2
    exit 1
fi

echo "All tests passed"
