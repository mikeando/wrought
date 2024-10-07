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

* [X] Audit of uses of std::fs
* [?] Plan for working with sqlite db in a non-fs way.
* [X] Completed all components (add as part of Audit) to no longer use std::fs
  * [X] component_store.rs
  * [X] event_log.rs
  * [X] backend.rs
  * [X] main.rs
  * [X] scripting_luau.rs
    * Added some tests using xfs , and some failing stub tests 
* [X] Convert xfs to handle .canonicalize
* [X] use fs.canonicalize instead of path.canonicalize or std::fs::canonicalize



* It is possible to hit the filesystem _without_ using std::fs - 
  * [X] In particular std::Path and std::PathBuf contain functions like `exists` etc.
  * [X] Also is_file and is_dir
  * [X] Also canonicalize

* [X] UGH - some points have things like the following which make it hard to see replacement points.
  ```
  use std::{fs, io};
  ```

* For now we'll leave solving the sqlite event_log for later.