#!/bin/bash

set -e -o pipefail

export RUST_BACKTRACE=1

cargo build --release
# cargo build

rm -rf "$(pwd)/"'test data'
cp -r "$(pwd)/"'test data 2' "$(pwd)/"'test data'

echo
echo '### Before run'
echo 'stats'
ls -lia './test data/'
echo

echo
echo '### run 1'
./target/release/hardlink-deduplicator "$(pwd)/""test data"

echo
echo '### after run 1'
echo 'index csv'
cat './test data/.index_file.csv' |column -tns,
echo
echo 'stats'
ls -lia './test data/'


echo
echo '### run 2'
./target/release/hardlink-deduplicator "$(pwd)/""test data"

echo
echo '### after run 2'
echo 'index csv'
cat './test data/.index_file.csv' |column -tns,
echo
echo 'stats'
ls -lia './test data/'

