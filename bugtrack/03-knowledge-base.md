# Support accessing a knowledgebase

## Status: P2 - Large - Needs shaping

## Problem 

We want some kind of queryable knowwldegebase that scripts can access.

Presumably this will be some kind of graph database, and some kind of vector-db.

## Design

### API???

Do we provide graph-db and vector-db apis. And/or do we provide something higher-level?

What about for the ingestion of raw documents - what input formats do we accept?
How do we transform them?

I suspect for now we might need to just accept text/markdown and add format conversion
tools as a later issue.

### High level API

retrieval

```.rust
let docs = kb.all_docs().like("what is the theme").tagged("research).find();
for doc in docs {
    pritn(doc.content());
}
```

ingestion

```.rust
// From text
let d1 = ai.generate("create a document on the theme of AI");
kb.ingest(Doc::FromRawContent(d1).tagged("ai_slop"));

// From file
kb.ingest_doc(Doc::FromFile("some_file").tagged("research"));
```


### Graph Dabatase

### Vector Database

## Issue log

### Initial Shaping
 * [ ] Add details for vector-db and graph-db core ideas
 * [ ] Add details for vector-db API
 * [ ] Add details for graph-db API
 