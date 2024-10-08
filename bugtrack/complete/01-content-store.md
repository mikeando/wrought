# Integrate content store into file management

## Status: Implemented

## Problem 

We want to be able to show content from the content store based on the content hash,
and restore/change files based on this hash.

This functionailty only need be implemented for the CLI, not made accessible to scripts. 

## Design

The user should be able to do something like the following:

```
>>> wrought history myfile.md
+ cefasefs-sdfssfs 'Init from template'
+ das1asdj-dgfgasd 'Manual update'
+ degfgdh-ushdhfsu 'Manual update'
- sdfsghs-sdfkjsdf 'Local Changes'
>>> wrought content-store-show das1asdj-dgfgasd
This is some file content
```

## Issue log

