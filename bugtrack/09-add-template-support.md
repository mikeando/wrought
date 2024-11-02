# Support templating in scripts

## Status: P1

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

## Issue log

* [ ] what templatingv library(s) should we support?
  * [ ] ? Tera - https://crates.io/crates/tera
     - Mostly built around directories of templates?
       - Means it is probably going to work better with the template object way, as we could probably do something like
         `template.add_template("some_name", "...template_content...");
  * [ ] ? Handlebars - https://crates.io/crates/handlebars 

