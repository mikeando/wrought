# Integrate Xfa to allow testing without hitting the filesystem

## Status: Backlog - P1

## Problem 

Most of the code currently directly accesses the filesystem, which makes testing components without actually hitting the filesystem tricky.
Instead we should use the existing Xfs library to abstract this away.

## Design

Convert each component to use a non-filesystem implementation.
In most cases this will mean using Xfs, but in some cases this may be "trickier".

We can get partway by auditing for usage of std::fs. 
However we also use creation of sqlite db - so we need to be careful about that too,

## Issue log

* [ ] Audit of uses of std::fs
* [ ] Plan for working with sqlite db in a non-fs way.
* [ ] Completed all components (add as part of Audit)
  * [X] component_store.rs
  * [ ] event_log.rs
  * [ ] backend.rs
  * [ ] main.rs
  * [X] scripting_luau.rs
    * Added some tests using xfs , and some failing stub tests 


