#! /usr/bin/env bash
set -euo pipefail

cargo build
wrought=./target/debug/wrought
PROJECT_DIR=test_projects/luau_ai_tools

echo "CLEANING PROJECT_DIR=${PROJECT_DIR}"
rm -rf "$PROJECT_DIR"
echo "RUNNING init"
${wrought} init --package=test "$PROJECT_DIR"

echo "INSTALLING A DUMMY OPEN AI KEY"
echo 'openai_api_key = "NO-SUCH-KEY"' >> ${PROJECT_DIR}/.wrought/settings.toml

echo "INSTALLING THE AI CACHE"
# This means we dont actually need to hit openAI as the query is cached.
cp -r test_resources/ai_test/llm_cache/* ${PROJECT_DIR}/.wrought/llm_cache/

echo "CREATING THE OUTLINE"
cp -r test_resources/ai_test/example_outline.md ${PROJECT_DIR}/outline.md

echo "RUNNING SIMPLE LUAU SCRIPT"
${wrought} --project-root=${PROJECT_DIR} run-script test/ai_tool_test.luau
