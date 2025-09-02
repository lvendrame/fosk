use crate::parser::WordComparer;

#[derive(Debug)]
pub struct QueryComparers {
    pub select: WordComparer,
    pub alias: WordComparer,
    pub from: WordComparer,
    pub inner_join: WordComparer,
    pub left_join: WordComparer,
    pub right_join: WordComparer,
    pub full_join: WordComparer,
    pub on: WordComparer,
    pub r#where: WordComparer,
    pub group_by: WordComparer,
    pub asc: WordComparer,
    pub desc: WordComparer,
    pub having: WordComparer,
    pub order_by: WordComparer,
    pub limit: WordComparer,
    pub offset: WordComparer,
    pub and: WordComparer,
    pub or: WordComparer,
    pub equal: WordComparer,
    pub not_equal_b: WordComparer, // basic
    pub not_equal_c: WordComparer, // c
    pub greater_than: WordComparer,
    pub greater_than_or_equal: WordComparer,
    pub less_than: WordComparer,
    pub less_than_or_equal: WordComparer,
    pub like: WordComparer,
    pub not_like: WordComparer,
    pub is_null: WordComparer,
    pub is_not_null: WordComparer,
    pub r#in: WordComparer,
    pub not_in: WordComparer,
    pub b_true: WordComparer,
    pub b_false: WordComparer,
    pub null: WordComparer,
}

impl Default for QueryComparers {
    fn default() -> Self {
        Self::new()
    }
}

impl QueryComparers {
    pub fn new() -> Self {
        Self {
            select: WordComparer::new("SELECT").with_whitespace_postfix(),
            alias: WordComparer::new("AS").with_whitespace_postfix(),
            from: WordComparer::new("FROM").with_whitespace_postfix(),
            inner_join: WordComparer::new("INNER JOIN").with_whitespace_postfix(),
            left_join: WordComparer::new("LEFT JOIN").with_whitespace_postfix(),
            right_join: WordComparer::new("RIGHT JOIN").with_whitespace_postfix(),
            full_join: WordComparer::new("FULL JOIN").with_whitespace_postfix(),
            on: WordComparer::new("ON").with_whitespace_postfix(),
            r#where: WordComparer::new("WHERE").with_whitespace_postfix(),
            group_by: WordComparer::new("GROUP BY").with_whitespace_postfix(),
            asc: WordComparer::new("ASC").with_whitespace_postfix().with_eof().with_optional_postfix(','),
            desc: WordComparer::new("DESC").with_whitespace_postfix().with_eof().with_optional_postfix(','),
            having: WordComparer::new("HAVING").with_whitespace_postfix(),
            order_by: WordComparer::new("ORDER BY").with_whitespace_postfix(),
            limit: WordComparer::new("LIMIT").with_whitespace_postfix(),
            offset: WordComparer::new("OFFSET").with_whitespace_postfix(),
            and: WordComparer::new("AND").with_whitespace_postfix(),
            or: WordComparer::new("OR").with_whitespace_postfix(),
            equal: WordComparer::new("=").with_whitespace_postfix(),
            not_equal_b: WordComparer::new("<>").with_whitespace_postfix(),
            not_equal_c: WordComparer::new("!=").with_whitespace_postfix(),
            greater_than: WordComparer::new(">").with_whitespace_postfix(),
            greater_than_or_equal: WordComparer::new(">=").with_whitespace_postfix(),
            less_than: WordComparer::new("<").with_whitespace_postfix(),
            less_than_or_equal: WordComparer::new("<=").with_whitespace_postfix(),
            like: WordComparer::new("LIKE").with_whitespace_postfix(),
            not_like: WordComparer::new("NOT LIKE").with_whitespace_postfix(),
            is_null: WordComparer::new("IS NULL").with_whitespace_postfix().with_eof(),
            is_not_null: WordComparer::new("IS NOT NULL").with_whitespace_postfix().with_eof(),
            r#in: WordComparer::new("IN").with_delimiter('('),
            not_in: WordComparer::new("NOT IN").with_delimiter('('),
            b_true: WordComparer::new("TRUE").with_any_delimiter_postfix().with_eof(),
            b_false: WordComparer::new("FALSE").with_any_delimiter_postfix().with_eof(),
            null: WordComparer::new("NULL").with_any_delimiter_postfix().with_eof(),
        }
    }
}
