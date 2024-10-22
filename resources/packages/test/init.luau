print("In the init script")

write_file("outline.md", [[
Fill this in with an outline of your document.

* SOMETHING

* SOMETHING ELSE
]])

write_file(".wrought/packages/test/status/01_init.toml", [[
title="Setup the init"
status="""
Setup the init
"""
next_steps=["setup the init then do the thing"]
]])