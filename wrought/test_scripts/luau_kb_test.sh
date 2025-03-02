#! /usr/bin/env bash
set -euo pipefail

cargo build
wrought=./target/debug/wrought
PROJECT_DIR=test_projects/luau_kb_test

echo "CLEANING PROJECT_DIR=${PROJECT_DIR}"
rm -rf "$PROJECT_DIR"
echo "RUNNING init"

# TODO: Should we use a different package here?
${wrought} init --package=test "$PROJECT_DIR"

echo "INSTALLING SOME DOCS"
cp -r test_resources/kb_test/research ${PROJECT_DIR}/research

echo "RUNNING SIMPLE LUAU SCRIPT"
${wrought} --project-root=${PROJECT_DIR} run-script test/kb_test.luau