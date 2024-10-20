# Ability to get directory status

We already have the ability to get file-history, but we want ways to get a summary of
directory status

## Status: P1 - medium

## Problem

## Design

The call should be as simple as `wrought status dir`.
It should probably give the status for each file in the directory.
The status is just clean/dirty/untracked/stale.
Can there be combinations of these?

* dirty means the local file has chnaged since the last log entry.
* stale means the inputs to the last log event that changes the file have changed.
* clean means neither dirty or stale
* untracked means there are no log entries for the file

A file can be both dirty and stale.

Are there other states? Should we distinguish recreated from stale etc?
Do these match the statuses returned by the file status?


```
> wrough status some_dir
S  some_stale_file
 D some_dirty_file
SD a_dirty_stale_file
   a_clean_file
?? an_untracked_file
```

We may need to add flags to suppress some of these options. Or support coloured output.
We may want a recursive version.

But all of these can come later.

## Issue log

### Shaping

* [X] what does the CLI calls for getting directory status look like?
  * [X] what is included, what is excluded? What does the output look like?


