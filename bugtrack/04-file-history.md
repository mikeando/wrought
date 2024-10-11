# File history

## Status: P1

## Problem 

The user should be able to see the history of a file. 
I think this was partly implemented in a previous change for 
the content store.

previous change was

'''
>>> wrought history myfile.md
+ cefasefs-sdfssfs 'Init from template'
+ das1asdj-dgfgasd 'Manual update'
+ degfgdh-ushdhfsu 'Manual update'
- sdfsghs-sdfkjsdf 'Local Changes'
'''

Think what we need to do here is primarily around testing all the various cases.

And this probably requires mocking the event log, and extracting the printing function core to something that returns a lost of entries. or something that takes a Writer.

cases to consider are...


## Design

## Issue log
