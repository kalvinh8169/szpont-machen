#!/bin/sh
set -e

cd "$(dirname "$0")/.."

pnpm --dir docs install
pnpm --dir docs build

STAGING=.tmp/cf-site
rm -rf "$STAGING"
mkdir -p "$STAGING/szpont-machen"
cp -R docs/out/. "$STAGING/szpont-machen/"
printf '/ /szpont-machen/ 302\n' > "$STAGING/_redirects"

npx -y wrangler@latest pages deploy "$STAGING" --project-name=szpont-machen --commit-dirty=true
