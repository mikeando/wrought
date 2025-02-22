use std::collections::BTreeMap;

use super::document::DocumentId;

#[derive(PartialEq, Debug, Clone)]
pub struct Connection {}
pub trait GraphIndex {
    fn connections_for_doc_id(&self, doc_id: &DocumentId) -> anyhow::Result<Vec<Connection>>;
    fn add_connections(
        &mut self,
        doc_id: &DocumentId,
        connections: &[Connection],
    ) -> anyhow::Result<()>;
}

#[derive(Default)]
pub struct InMemoryGraphIndex {
    connections: BTreeMap<DocumentId, Vec<Connection>>,
}

impl GraphIndex for InMemoryGraphIndex {
    fn connections_for_doc_id(&self, doc_id: &DocumentId) -> anyhow::Result<Vec<Connection>> {
        match self.connections.get(doc_id) {
            Some(v) => Ok(v.clone()),
            None => Ok(vec![]),
        }
    }

    fn add_connections(
        &mut self,
        doc_id: &DocumentId,
        connections: &[Connection],
    ) -> anyhow::Result<()> {
        let v = self.connections.entry(doc_id.clone()).or_default();
        for c in connections {
            v.push(c.clone())
        }
        Ok(())
    }
}
