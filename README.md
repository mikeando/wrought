# Wrought
Wrought is a tool for text generation and preparation.

Unlike most text generation tools it is not designed to be 
automatic, and non-iteractive. The idea is to run stages of wrought, edit its inputs and rerun until you get output you are happy with, then use those outputs as inputs to the next stage - possilby after editing them too.

A lot was learned from prototyping in booker/inscenerator, and it is hoped/likely that this
work will be folded back into that code-base at a later time.

# Key ideas

files: These come in two types - user and internal. Really the only difference is that
internal files live inside the `.brain/internal_files` directory. Otherwise they are identical.
The idea being that "scripts" will often create internal files to store state that does not
form part of the target output. Or often scripts will move files from a user location
to an internal location once it is no-longer expected to be edited by the user.

**metadata**: Each file has associated metadata. The metadata for a given file lives in
`.brain/metadata/path_to_file`.

**content store**: This maps file hashes to actual content. `.brain/content_store`

**scripts & history**: Running a command in wrought generates a log of what it did that gets written into `.brain/actions/actionid`. The log contains the name of the script, the parameters it was run with and the list of "microactions" it performed. 

**microactions**: These detail any IO performed and a hash of the associated content. This allows us to see how/why any file has a given state.


Imagine a piece of code that generates the initial index:

```rust
pub fn create_initial_index(&mut self, path:&str, content:&u8 ) {
    self.begin_script("create initial index", |m| {
        if !m.file_exists("index.md") {
            m.create_file("index.md", "An initial index file");
        }
    });
}
```

On first run it might generate a `000000_action.json` like this:

```json
{
    "name": "create initial index",
    "parameters": {},
    "microactions": [
        {"action":"FileExists", "path":"index.md", "result":false},
        {"action":"WriteFile", "path":"index.md", "prev":null, "hash":"cfgsh-dm#,s"},
    ]
}
```

Later if we want to find out where "index.md" came from we can parse all the action files looking
for "WriteFile" entries with a path of "index.md". If we want to see if a file has
changed since it was generated we can find the most recent "WriteFile" entry and compare its
hash to the files current hash. If they differ - the user has edited the file.

Similarly we can find out if a file is "stale" by checking to find the WriteFile that last wrote to it - then finding all the inputs to that script (before it in the list too) and then checking the hashes of those inputs against the current index.

To allow us to reverse the hash values, whenever we write a file, we need to store the content, not just to the .brain file, but also into the content store. It is possible and allowed for people to edit files in the `.brain/internal_files` directory - but they should not change the `.brain/content_store` directory, as those files should be _fixed_ - but we can easily check for any file that has a content that does not match its content hash.

We need a similar tracking of metadata - but I think that should be mostly possible on a per-key basis and short responses need not be hashed, but stored directly in the microaction?

```rust
pub fn hello_world() {
    self.begin_script("hello world", |m| {
        if let Some(md) m.get_metadata("index.md", "name") {
            m.write_file("hello.txt", format!("greetings, {}", md.as_string());
        } else {
            m.set_metadata("index.md", "name", "Unknown");
            m.write_file("hello.txt", "greetings!");
        }
    }
}
```

On it's first run might output

```json
{
    "name":"hello world",
    "parameters":{},
    "microactions":[
        {"action":"GetMetadata", "path":"index.md", "key":"name", "value":null},
        {"action":"SetMetadata", "path":"index.md", "key":"name", "value":"Unknown"},
        {"action":"WriteFile", "path":"hello.txt", "prev":null, "hash":"1e#a%f"},
    ]
}
```

And on the second run

```json
{
    "name":"hello world",
    "parameters":{},
    "microactions":[
        {"action":"GetMetadata", "path":"index.md", "key":"name", "value":"Unknown"},
        {"action":"WriteFile", "path":"hello.txt", "prev":"1e#a%f", "hash":"3dc&2eX$"},
    ]
}
```

Now when we check the state of "hello.txt", we'd first check if it has been edited by comparing it's hash with
`3dc&2eX$` and then check for staleness by checking whether the "name" metadata for `index.md` was still `"Unknown"`.

---

What about a plan for things that have not been executed yet. 

When I coome back to a project that I was part way through setting up, I often don't know what the last things that were
run were, nor what I should be running next. 

I would be nice if there were a "plan" that could tell me, these are the things you've just done. And these are the
things you're likely to want to do next. 

So the questions are

* What does a plan look like?
* How do we trigger transitions in the plan?
* How do we know what we can/should do?
* Where is the plan state stored

Chat GPT Says: 

---

Here are some structured thoughts on how to introduce "plans" into Wrought:

### 1. **Plan Structure**
   - **Plan File**: Each plan could be a JSON or TOML file stored in `.brain/plans/`. The plan file should include:
     - **Plan Name**: A unique identifier or name for the plan.
     - **Stages**: An ordered list of stages or tasks within the plan. Each stage should have:
       - **Stage Name**: A short descriptor of the task.
       - **Description**: Detailed information on what the stage involves.
       - **Dependencies**: Optional, a list of other stages that must be completed before this one can be executed.
       - **Status**: Current status (`Pending`, `In Progress`, `Completed`).
       - **Next Actions**: Suggested actions based on the current state of the project.
     - **Triggers**: Conditions that automatically advance or modify the plan.

### 2. **Triggering Transitions**
   - **Manual Triggering**: Users can manually mark stages as `In Progress` or `Completed`. 
   - **Automatic Triggers**: Integrate with the microactions log to automatically transition a stage to `Completed` when related microactions are detected. For example, if a microaction indicates a file has been created, the corresponding stage could be marked as `Completed`.
   - **Conditional Transitions**: Use metadata and file hashes to detect changes or updates that should advance the plan. For instance, if certain metadata is updated, the plan might automatically suggest the next stage.

### 3. **Determining What to Do Next**
   - **Plan Navigation**: Each plan file should include a `Next Suggested Stage` field, which updates based on completed stages. This could be determined by checking the `Status` and `Dependencies` of each stage.
   - **User Prompts**: The system can prompt the user with suggested actions when they open the project. These suggestions could be based on the `Next Suggested Stage` or triggered by recent changes in files or metadata.
   - **Dependency Graph**: Visualize the stages in a dependency graph, allowing users to see at a glance what’s been done and what’s left to do.

### 4. **Plan State Storage**
   - **Plan Metadata**: Store the plan's current state (e.g., current stage, completed stages, etc.) in `.brain/plan_states/`. This could be a separate file or embedded within the plan file itself.
   - **State Synchronization**: Ensure the plan state is synchronized with the microactions log. For instance, if a stage is marked `Completed`, it should be reflected both in the plan file and the plan state file.

These structures would allow for a flexible and adaptive planning system within Wrought, helping users to manage complex, multi-step workflows efficiently.

---

In some ways it feels like a plans state is an output from the plan - and depends on various inputs - almost exactly
like the scripts idea above. The difference being that we might check many of them in one hit and auto-update their content.

---

Another thought is that the action logs are probably quite slow to parse as they grow.
We probably want to maintain files that contain the current dependency information. 

I think this could just be a list of known paths, then a a list of `id: [id]` type things. 
If we binary encode this it could be pretty fast to load? (But maybe even just JSON would be fast enough for now?)



# Core commands

## Creating a new project

> `wrought init <path> --package=<package>`

Creates a new wrought project at the specified path, and installs the specified packages (script bundles).

## Project status

### Get file status.

### Get packages steps.



