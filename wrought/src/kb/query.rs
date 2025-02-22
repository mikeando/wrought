use super::filter::Filter;

#[derive(Clone)]
pub enum Query {
    AllDocs,
    EmptyStream,
    Filter(Filter, Box<Query>),
    Sequence(Vec<Query>),
    WithTextQuery(String, Box<Query>),
    WithVectorQuery(Vec<f32>, Box<Query>),
}

impl Query {
    pub fn filter(&self, f: Filter) -> Query {
        let query = match self {
            Query::EmptyStream => Some(Query::EmptyStream),
            Query::Filter(f2, info) => Some(Query::Filter(
                Filter::and(f2.clone(), f.clone()),
                info.clone(),
            )),
            Query::Sequence(infos) => {
                // f(seq(a,b,c)) = seq(f(a), f(b), f(c))
                let infos: Vec<_> = infos
                    .iter()
                    .map(|v| Query::Filter(f.clone(), Box::new(v.clone())))
                    .collect();
                if infos.is_empty() {
                    Some(Query::EmptyStream)
                } else {
                    Some(Query::Sequence(infos))
                }
            }
            Query::AllDocs => None,
            Query::WithTextQuery(_, _) => None,
            Query::WithVectorQuery(_, _) => None,
        };
        let query = query.unwrap_or_else(|| Query::Filter(f, Box::new(self.clone())));
        query
    }

    pub fn sequence(queries: &[Query]) -> Query {
        let mut non_empty_queries = vec![];
        for query in queries {
            match query {
                Query::EmptyStream => {}
                _ => non_empty_queries.push(query.clone()),
            }
        }

        if non_empty_queries.is_empty() {
            return Query::EmptyStream;
        }
        if non_empty_queries.len() == 1 {
            return non_empty_queries[0].clone();
        }
        Query::Sequence(non_empty_queries)
    }

    pub fn with_text_query(&self, query: &str) -> Query {
        Query::WithTextQuery(query.to_string(), Box::new(self.clone()))
    }

    pub fn with_vector_query(&self, key: Vec<f32>) -> Query {
        Query::WithVectorQuery(key, Box::new(self.clone()))
    }
}
