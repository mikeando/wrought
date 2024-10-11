# Initial AI Integration

## Status: Backlog - DONE

## Problem 

We want to allow access to AI functions from the CLI and from the scripting.
The queries need to be cached.

## Design

We'll use our exitsing OpenAI library and caching to provide the backend.

The basic function we need to provide at the script level is simply 

```
wrought_ai_query(request:str) -> str
```

Additional functionality will come from using templates and knowledge-base integration, which will come later.

## Issue log

* We still need to consider "tool" support 
    - how we get structured output back to the scripts
    - how we accept definitions of the tools from the scripts
    - For now this is moved to 14-support-ai-tools.md

* This is pretty lame as CLI usage. It really needs access to document content etc. But that can come later?
* The openAI api key is pulled from .wrought/settings.toml.

