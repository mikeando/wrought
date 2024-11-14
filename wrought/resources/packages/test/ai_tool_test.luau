
local outline = read_file("outline.md")
local tool_schema = read_file(".wrought/packages/test/ai_tool_test_schema.json")

local mesg = wrought_ai_message()
mesg:set_system_message("You are an expert book authoring AI.")
mesg:add_user_message("Generate a overview or high level description for the following book, including a rough outline of the contents:\n\n" .. outline)
mesg:add_tool(
    "Create the overview for a new book.",
    "generate_overview",
    tool_schema
)

-- Make this a member function?
local response = mesg:request()
print(response)
local description = response["description"]
write_file("overview.md", description)
