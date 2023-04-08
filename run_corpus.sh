#!/usr/bin/env bash

cargo build --example parse

executable=target/debug/examples/parse

file_count=$(find fuzz/corpus -type f | wc -l)
current=0

bar_length=50

make_bar() {
	local progress=$((current * bar_length / file_count))
	local spaces=$((bar_length - progress))

	seq -s# $progress | tr -d '[:digit:]' | tr -d '\n'
	seq -s' ' $spaces | tr -d '[:digit:]'
}

echo -ne "[$(make_bar)] $current/$file_count \r"

fail=0

while IFS= read -r -d '' file; do
	echo -ne "[$(make_bar)] $current/$file_count \r"
	$executable "$file" >/dev/null || {
		echo -e "$file FAILED"
		fail=1
	}
	current=$((current + 1))
done < <(find fuzz/corpus -type f -print0)

if [[ $fail = 1 ]]; then
	echo -e "\nTEST FAILURE"
fi

exit $fail
