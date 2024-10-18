# Implement an in memory or mock EventLog

At the moment the EventLog requires access to a SQLite DB.
This makes it tricky to test comopnents that interact with an EventLog.

We just need an in-memory version or mock version for testing

This was resolved as part of 04-file-history - since it needed a mock EventLog.

## Status: CLOSED

## Problem 

## Design

## Issue log

* Descide to go with mocking the EventLog using mockall.