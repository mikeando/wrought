#! /usr/bin/env bash
set -euo pipefail

cargo build
wrought=./target/debug/wrought
PROJECT_DIR=test_projects/simple_ai

echo "CLEANING PROJECT_DIR=${PROJECT_DIR}"
rm -rf "$PROJECT_DIR"
echo "RUNNING init"
${wrought} init --package=test "$PROJECT_DIR"

echo "INSTALLING SIMPLE AI SCRIPT"

cat << "EOF" > "${PROJECT_DIR}/.wrought/packages/test/test_ai.luau"
content = ai_query("Tell me a fun story")
print("AI SAYS: ", content)
EOF

echo "INSTALLING THE AI CACHE"
# This means we dont actually need to hit openAI as the query is cached.

cp -r test_resources/ai_test/llm_cache/* ${PROJECT_DIR}/.wrought/llm_cache/

echo "RUNNING SIMPLE AI SCRIPT"
${wrought} --project-root=${PROJECT_DIR} run-script test/test_ai.luau