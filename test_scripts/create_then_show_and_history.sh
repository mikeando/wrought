#! /usr/bin/env bash
set -euo pipefail
cargo build
wrought=./target/debug/wrought
PROJECT_DIR=test_projects/dummy02

echo "CLEANING PROJECT_DIR=${PROJECT_DIR}"
rm -rf "$PROJECT_DIR"
echo "RUNNING init"
${wrought} init --package=test "$PROJECT_DIR"

echo "New content" > "$PROJECT_DIR/outline.md"

${wrought} history "$PROJECT_DIR/outline.md"

${wrought} --project-root="$PROJECT_DIR" content-store-show 3Je0-2b3pjMftS96fYVwJw

