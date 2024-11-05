# Support templating in scripts

## Status: DONE

## Problem

It is painful to create text in the scripts, especially structured text.

There are many libraries to allow better handling of this in rust - such as Terra, or handlebars.

WASM plugins can use these by comiling those libraries into WASM, but that results in a lot of
code bloat for the plugins. (And is a cost incurred by each plugin too.)

Ideally we would provide templating support from within the application itself.


## Design

What does this look like to the scripts?

### Option 1: Explicit template objects

```rust
let template_content = wrought.read_file("some_file.template");
let template: WroughtTemplate = wrought.template_from_string(template_content);
let values: serde_json::Value = json!({"key":"value"});
let rendered = template.render(values);
...
```

### Option 2: Just template render functions

```rust
let template_content = wrought.read_file("some_file.template");
let values = json!({"key":"value"});
let renderd = wrought.render_template(template_content, values);
...
```

NOTE: We dont really want to allow specifying a template from file path as this 
leads to issues with (a) ensuring the file is within the project, (b) the posibility
of missing it as a dependency it the file tracking. If we use the usual `read_file`, `write_file`
functions then we avoid this issue.

### Final decision

In the end we went with Tera and the template object approach - in lua this looks like:

```lua
local templater = wrought_template()
templater:add_template("hello", "Hello, {{ name }}: zip+1 = {{ zip + 1 }}")
local values = {name="World", zip=1}
local result = templater:render_template("hello", values)
print("TEMPLATE:", result);
```

And in rust-wasm it looks like

```rust
let mut templater = wrought.template().map_err(|e| anyhow::anyhow!("unable to create templater: {}", e))?;
templater.add_template("hello", "Hello, {{ name }}: zip+1 = {{ zip + 1 }}").map_err(|e| anyhow::anyhow!("unable to add template: {}", e))?;
let values = serde_json::json!({ "name": "World", "zip":1 });
let result = templater.render_template("hello", &values).map_err(|e| anyhow::anyhow!("unable to render template: {}", e))?;
println!("TEMPLATE: {}", result);
```

## Issue log

* [X] what templatingv library(s) should we support?
  * [X] ? Tera - https://crates.io/crates/tera
     - Mostly built around directories of templates?
       - Means it is probably going to work better with the template object way, as we could probably do something like
         `template.add_template("some_name", "...template_content...");
  * [ ] ? Handlebars - https://crates.io/crates/handlebars 
     - Not for now... tera is enough for now.

