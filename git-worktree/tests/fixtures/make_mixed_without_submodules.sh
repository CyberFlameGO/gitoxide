#!/bin/bash
set -eu -o pipefail

git init -q
git config commit.gpgsign false

touch empty
echo -n "content" > executable
chmod +x executable

mkdir dir
echo -n "other content" > dir/content
mkdir dir/sub-dir
(cd dir/sub-dir && ln -sf ../content symlink)

git add -A
git commit -m "Commit"
