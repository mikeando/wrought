# Ability to get project and directory status

We already have the ability to get file-history, but we want ways to get a summary of
directory and/or overall project status

## Status: P1 - medium

## Problem 

## Design

### Project Status

The call is as simple as `wrought project-status`.

The output should include:
  * what files are stale, or uncommitted.
  * the status of each of the packages.
  * what the next actions the user is expected to perform are (is that part of the previous bit)

### Directory Status

## Issue log

### Shaping

* [ ] what does the CLI calls for getting directory status look like?
  * [ ] what is included, what is excluded? What does the output look like?
* [ ] what does the CLI calls for getting project status look like?
   * [ ] how do we get package status and expected next actions for the user?
* [ ] should we split this bugtrack into two, one for directory status, one for project status?


