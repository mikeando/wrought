# Running `wrought update`

## Status: P2

## Problem

There is a awy for packages to register the next steps that the author might take,
to trigger additional script actions.

This ticket is about creating a `wrought update` command that will actually allow
these extra steps to run.

## Design

* Hooks.
  * Little scripts that run on `wrought update`.
  * (maybe) do we want other hooks - scritps that can run on project status etc?
    * Maybe - but if so we should have that as another bugtrack entry.
  * I guess a hook subdirectory of each of the  packages?

## Issue log
