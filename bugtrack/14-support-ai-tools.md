# Support AI "tools"

This means getting structured JSON out of the AI.

## Status: []

## Problem 

Without getting structured JSON out of the AI, it becomes very difficult to perform a structured workflow of actions.

## Design

Typically a tool is defined using a JSON schema object - which is just JSON, and the return value is just json.

For WASM we can just use text based transfer of the JSON, since we already require serde::json on the plugin.

For lua we need to convert to a lua table - but thankfully we already those conversion functions due to
the work on templating.

Much of the tool work is already supported in the library we use, other parts can possibly by cribbed from the 
initial integration in inscenerator.

## Notes

At the moment we do this inside `OpenAILLM::query()`

```rust
let messages = vec![SystemMessage::new(query).into()];
let request = ChatRequest::new(rust_openai::types::ModelId::Gpt4oMini, messages);
let (response, _) = self.llm.make_request(&request).await?;
Ok(response.choices[0]
    .message
    .as_assistant_message()
    .as_ref()
    .unwrap()
    .content
    .as_ref()
    .unwrap()
    .clone()
)
```

An example of tool use from inscenerator is

```rust
    let schema = JSONSchema(serde_json::to_value(schema_for!(Overview)).unwrap());
    let message = format!("Generate a overview or high level description for the following book, including a rough outline of the contents:\n\n{}", outline);
    let request: ChatRequest = ChatRequest::new(
        ModelId::Gpt35Turbo,
        vec![
            Message::system_message("You are an expert book authoring AI."),
            Message::user_message(message),
        ],
    ).with_tools(vec![
        Tool{
            description: Some("Create the overview for a new book.".to_string()),
            name: "generate_overview".to_string(),
            parameters: Some(schema),
        }]
    ).with_max_tokens(Some(ai_options.max_tokens));

    let (response, _) = llm.make_request(&request).await?;

    let overview: Overview = serde_json::from_str(
        &response.choices[0]
            .message
            .as_assistant_message()
            .as_ref()
            .unwrap()
            .tool_calls
            .as_ref()
            .unwrap()[0]
            .function
            .arguments,
    )
    .unwrap();
```

The question is whether we provide a structured way of building a request and getting the results, or just a single
function. 

i.e. does the API look like

```lua
local mesg = wrought_ai_message()
mesg:set_system_message("You are an expert book authoring AI.")
mesg:add_user_message("Generate a overview or high level description for the following book, including a rough outline of the contents:\n\n" + outline)
mesg:add_tool(
    "Create the overview for a new book.",
    "generate_overview",
    tool_schema,
)

local response = mesg:request()
print(response)
```

or is it

```lua
local response = wrought_ai_tool_request(
    "You are an expert book authoring AI.",
    "Generate a overview or high level description for the following book, including a rough outline of the contents:\n\n" + outline,
    "Create the overview for a new book",
    "generate_overview",
    tool_schema
)
print(response)
```

The first seems like it will be more extensible going forward, but the second quicker to implement.
But the first also follows the way the templating works - so we should be able to copy much of the way that one works.

## Issue log