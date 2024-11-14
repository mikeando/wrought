#! /usr/bin/env bash
set -euo pipefail

cargo build
wrought=./target/debug/wrought
PROJECT_DIR=test_projects/wasm_ai_tools
WASM_SCRIPT_DIR=../example-wasm-script-ai-tool

echo "CLEANING PROJECT_DIR=${PROJECT_DIR}"
rm -rf "$PROJECT_DIR"
echo "RUNNING init"
${wrought} init --package=test "$PROJECT_DIR"

echo "BUILDING THE WASM SCRIPT"
( cd "$WASM_SCRIPT_DIR" && cargo build  --target wasm32-wasip1 )

echo "INSTALLING SIMPLE WASM SCRIPT"
cp "${WASM_SCRIPT_DIR}/target/wasm32-wasip1/debug/example_wasm_script_ai_tool.wasm" "${PROJECT_DIR}/.wrought/packages/test"

echo "INSTALLING A DUMMY OPEN AI KEY"
echo 'openai_api_key = "NO-SUCH-KEY"' >> ${PROJECT_DIR}/.wrought/settings.toml
# cp ai_settings_no_commit.toml ${PROJECT_DIR}/.wrought/settings.toml

echo "INSTALLING THE AI CACHE"
# This means we dont actually need to hit openAI as the query is cached.
cp -r test_resources/ai_test/llm_cache/* ${PROJECT_DIR}/.wrought/llm_cache/

echo "CREATING THE OUTLINE"
cp -r test_resources/ai_test/example_outline.md ${PROJECT_DIR}/outline.md

echo "RUNNING SIMPLE AI SCRIPT"
${wrought} --project-root=${PROJECT_DIR} run-script test/example_wasm_script_ai_tool.wasm