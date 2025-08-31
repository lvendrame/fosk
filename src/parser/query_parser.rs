use crate::parser::{QueryComparers};

#[derive(Debug, Default)]
pub struct TokenPosition {
    pub pivot: usize,
    pub end: usize,
}

#[derive(Debug, Default, PartialEq, PartialOrd)]
pub enum Phase {
    #[default]
    Projection = 0,
    Collections = 1,
    Joins = 2,
    Criteria = 3,
    Aggregates = 4,
    Having = 5,
    OrderBy = 6,
    EOF = 7,
}

#[derive(Debug, Default)]
pub struct QueryParser {
    pub position: usize,
    pub length: usize,
    pub text_v: Vec<char>,
    pub phase: Phase,
    pub text: String,
    pub token_position: TokenPosition,
    pub parentheses_depth: usize,

    pub comparers: QueryComparers,

    dbg: String,
}

impl QueryParser {
    pub fn new(query: &str) -> Self {
        Self {
            position: 0,
            length: query.len(),
            text_v: query.chars().collect(),
            text: query.to_string(),
            comparers: QueryComparers::new(),
            dbg: query.chars().next().unwrap_or('\0').to_string(),
            ..Default::default()
        }
    }

    pub fn eof(&self) -> bool {
        self.position >= self.length
    }

    pub fn current(&self) -> char {
        if self.position < self.length {
            return self.text_v[self.position];
        }

        '\0'
    }

    pub fn peek(&self, ahead: usize) -> char {
        self.text_v[self.position + ahead]
    }

    pub fn next(&mut self) {
        self.position += 1;

        self.dbg = self.current().to_string();
    }

    pub fn next_non_whitespace(&mut self) {
        while self.current().is_whitespace() {
            self.next();
        }

        self.dbg = self.current().to_string();
    }

    pub fn jump(&mut self, ahead: usize) {
        if self.position + ahead < self.length {
            self.position += ahead;
        } else {
            self.position = self.length - 1;
        }

        self.dbg = self.current().to_string();
    }

    pub fn text_from_range(&self, start: usize, end: usize) -> String {
        let mut end = end;
        if end > self.length {
            end = self.length;
        }
        self.text_v[start..end].iter().collect()
    }

    pub fn text_from_pivot(&self, pivot: usize) -> String {
        self.text_from_range(pivot, self.position)
    }

    pub fn parse(&mut self) -> Result<String, String> {
        while self.phase != Phase::EOF {
            self.parse_current()?;
            // self.next();
        }
        Ok("".into())
    }

    pub fn check_next_phase(&mut self) -> bool {
        self.next_non_whitespace();
        // Projection = 0,
        // Collections = 1,
        // Inners = 2,
        // Criteria = 3,
        // Aggregates = 4,
        // Having = 5,
        // OrderBy = 6
        if self.eof() {
            self.phase = Phase::EOF;
            return true;
        }

        if self.phase < Phase::OrderBy && self.comparers.order_by.compare(self) {
            self.phase = Phase::OrderBy;
            return true;
        }

        if self.phase < Phase::Having && self.comparers.having.compare(self) {
            self.phase = Phase::Having;
            return true;
        }

        if self.phase < Phase::Aggregates && self.comparers.group_by.compare(self) {
            self.phase = Phase::Aggregates;
            return true;
        }

        if self.phase < Phase::Criteria && self.comparers.r#where.compare(self) {
            self.phase = Phase::Criteria;
            return true;
        }

        if self.phase <= Phase::Joins &&
            (self.comparers.inner_join.compare(self) || self.comparers.left_join.compare(self) ||
                self.comparers.right_join.compare(self) || self.comparers.full_join.compare(self)) {
            self.phase = Phase::Joins;
            return true;
        }

        if self.phase < Phase::Collections && self.comparers.from.compare(self) {
            self.phase = Phase::Collections;
            return true;
        }

        false
    }

    fn parse_current(&mut self) -> Result<(), String> {
        let current = self.current();
        if current.is_whitespace() || current == '\r' || current == '\n' {
            self.next();
            return Ok(());
        }

        match self.phase {
            Phase::Projection => {
                //Proje?;
            },
            Phase::Collections => {
                // let coll =  self.parse_collection()?;
                // self.query.collections.push(coll);
            },
            Phase::Joins => {self.next();},
            Phase::Criteria => {},
            Phase::Aggregates => {},
            Phase::Having => {},
            Phase::OrderBy => {},
            Phase::EOF => {},
        };

        Ok(())
    }

}

#[cfg(test)]
mod test {
    use crate::parser::*;

    #[test]
    pub fn dummy() {
        let query = r#"SELECT b.*, a.full_name as name, COUNT(*) as TotBy, *, AVG(a.sum), MyFn(a.sum, 2, 3), one as alias, field, other_field
FROM TableA a, TableB b, TableC,
     TableD
INNER JOIN TableB B ON A.id = B.id
INNER JOIN (query...) Q ON Q.id = B.q_id
WHERE A.Age > 16 AND (B.city = 'Porto' OR B.city like "Matosinhos")
GROUP BY a.full_name
HAVING COUNT(*) > 3
ORDER BY b.description DESC"#;

        let mut _parser = QueryParser::new(query);

        //let result = parser.parse();
        //println!("{:?}", parser.query);
    }

}

// QueryToken
//     ProjectionToken
//         ProjectionFieldToken
//             - FieldNameToken
//             - FieldTableToken
//             - FieldAliasToken
//     CollectionToken
//         - CollectionNameToken
//         - CollectionAlias
//     CollectionJoinToken
//         - CollectionNameToken
//         - CollectionAlias
//         JoinConstraintToken
//             LeftSideToken
//             OperatorToken
//             RightSideToken
//     CriteriaToken
//         LeftSideToken
//         OperatorToken
//         RightSideToken
//     AggregatorToken
//         AggregatorFieldToken
//     AggregatorConstraintToken
//         ...Constraints
//     OrderToken
//         OrderFieldTableToken
//         OrderFieldNameToken
//         OrderDirectionToken

