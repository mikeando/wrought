#! /usr/bin/env bash
set -euo pipefail

cargo build
wrought=./target/debug/wrought
PROJECT_DIR=test_projects/luau_test

echo "CLEANING PROJECT_DIR=${PROJECT_DIR}"
rm -rf "$PROJECT_DIR"
echo "RUNNING init"
${wrought} init --package=test "$PROJECT_DIR"

echo "RUNNING SIMPLE LUAU SCRIPT"
${wrought} --project-root=${PROJECT_DIR} run-script test/dummy.luau