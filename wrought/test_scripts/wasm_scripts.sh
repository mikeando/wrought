#! /usr/bin/env bash
set -euo pipefail

cargo build
wrought=./target/debug/wrought
PROJECT_DIR=test_projects/wasm_test
WASM_SCRIPT_DIR=../example-wasm-script

echo "CLEANING PROJECT_DIR=${PROJECT_DIR}"
rm -rf "$PROJECT_DIR"
echo "RUNNING init"
${wrought} init --package=test "$PROJECT_DIR"

echo "BUILDING THE WASM SCRIPT"
( cd "$WASM_SCRIPT_DIR" && cargo build  --target wasm32-wasip1 )

echo "INSTALLING SIMPLE WASM SCRIPT"
cp "${WASM_SCRIPT_DIR}/target/wasm32-wasip1/debug/example_wasm_script.wasm" "${PROJECT_DIR}/.wrought/packages/test"

echo "INSTALLING A DUMMY OPEN AI KEY"
echo 'openai_api_key = "NO-SUCH-KEY"' >> ${PROJECT_DIR}/.wrought/settings.toml

echo "INSTALLING THE AI CACHE"
# This means we dont actually need to hit openAI as the query is cached.
cp -r test_resources/ai_test/llm_cache/* ${PROJECT_DIR}/.wrought/llm_cache/

echo "RUNNING SIMPLE AI SCRIPT"
${wrought} --project-root=${PROJECT_DIR} run-script test/example_wasm_script.wasm