#!/bin/bash

if [ "$#" -lt 1 ]; then
    echo "Usage: $0 <github_tag_to_clone>"
    exit 1
fi

# Make errors fatal, print commands
set -ex

rm -rf /tmp/dist_dir /tmp/swyh_rs_tarball_"$1"_for_flathub_build.tar.gz

git clone --depth 1 --branch "$1" https://github.com/dheijl/swyh-rs /tmp/dist_dir

cd /tmp/dist_dir


# Fetch dependency sources to be bundled with the applicaiton
mkdir -p .cargo
cargo vendor --locked vendor | sed 's/^directory = ".*"/directory = "vendor"/g' > .cargo/config.toml

rm -rf .git

cd /tmp/dist_dir
tar zcvf ../swyh_rs_tarball_"$1"_for_flathub_build.tar.gz .

