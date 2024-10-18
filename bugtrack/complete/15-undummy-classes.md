# Fix naming of "Dummy" classes 

At the moment several classes are named DummyXYZ, but have over time 
switched to being fully-fledged classes - for Example DummyEventLog is 
the "actual" SQLite based event log, DummyBackend is the real FS backed
backend. These should be named correctly 

## Status: P2 - easy

## Problem 

## Design

* Classes should have names representing their implementations/usage.

## Issue log

* [X] Identify all the dummy classes
* [X] Rename each class.
  * [X] DummyBackend -> SimpleBackend
  * [X] DummyEventLog -> SQLiteEventLog
  * [X] DummyBridge -> SimpleBridge
  * [X] DummyContentStore -> FileSystemContentStore