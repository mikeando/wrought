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

# OUTLINE is pretty boring - it is creaeted during init, and so has no inputs.
# We just check it initially shows as OK, 
# Then if we change it's content we get CHANGED.

echo "RUNNING file-status with path to file rather than using project root"
${wrought} file-status "${PROJECT_DIR}/outline.md"

echo "CHANGING outline.md CONTENT"
echo "New Content" > $PROJECT_DIR/outline.md

echo "Untracked Content" > $PROJECT_DIR/untracked.md

${wrought} --project-root="$PROJECT_DIR" status --color


echo "RERUNNING file-status on outline.md"
${wrought} file-status "${PROJECT_DIR}/outline.md"

echo "DONE"