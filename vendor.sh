#!/bin/sh
set -e
mkdir -p vendor
if [ ! -f vendor/pulsar/src/bwt_ans.rs ]; then
  git clone --depth 1 https://github.com/ceedot-rock/pulsar-best.git vendor/pulsar
  rm -rf vendor/pulsar/.git
fi
echo "put Combined GC at vendor/combined-gc (lab zip). crate will not build without it."
