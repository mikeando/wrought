use std::collections::HashMap;

use super::document::DocumentId;

pub struct DocSet<T> {
    values: Vec<(DocumentId, T)>,
}
