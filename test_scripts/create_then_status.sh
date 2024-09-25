#! /usr/bin/env bash
set -euo pipefail
cargo build
wrought=./target/debug/wrought
PROJECT_DIR=test_projects/dummy01

echo "CLEANING PROJECT_DIR=${PROJECT_DIR}"
rm -rf "$PROJECT_DIR"
echo "RUNNING init"
${wrought} init --package=test "$PROJECT_DIR"
echo "RUNNING status"
${wrought} --project-root="$PROJECT_DIR" status
echo "DONE"