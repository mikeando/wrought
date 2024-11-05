print("This is just a test")

write_file("some_file.md", "SOME CONTENT")

print("READING FILE...")
local content = read_file("other_file.md")
print("GOT CONTENT...", content)
if content == nil then
    content = ''
end

local md = get_metadata("other_file.md", "a_key")
if md == nil then
    md = 'none'
end

write_file("another_file.md", content .. "metadata = '" .. md .. "'")
set_metadata("another_file.md", "a_key", "Some Value")

local templater = wrought_template()
templater:add_template("hello", "Hello, {{ name }}: zip+1 = {{ zip + 1}}")
local values = {name="World", zip=1}
local result = templater:render_template("hello", values)
print("TEMPLATE:", result);
