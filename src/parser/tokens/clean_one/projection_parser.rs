use crate::parser::{tokens::clean_one::{Identifier, Literal}, ParseError, QueryComparers, QueryParser};

pub struct ProjectionParser;

impl ProjectionParser {
    pub fn is_projection_start(parser: &QueryParser) -> bool {
        parser.comparers.select.compare(parser)
    }

    pub fn is_projection_end(parser: &QueryParser) -> bool {
        parser.comparers.from.compare(parser)
    }

    pub fn parse(parser: &mut QueryParser) -> Result<Vec<Identifier>, ParseError> {
        let mut result: Vec<Identifier> = vec![];
        let mut can_consume = true;
        while !parser.eof() && !parser.comparers.from.compare(parser) {
            let current = parser.current();
            if current == ',' {
                if can_consume {
                    return ParseError::new("Invalid projection", parser.position, parser).err();
                }

                can_consume = true;
            }

            if !current.is_whitespace() && !QueryComparers::is_block_delimiter(current) {
                if can_consume {
                    result.push(Identifier::parse(parser)?);
                } else {
                    return ParseError::new("Invalid projection", parser.position, parser).err();
                }
            }

            parser.next();
        }

        Ok(result)
    }
}
