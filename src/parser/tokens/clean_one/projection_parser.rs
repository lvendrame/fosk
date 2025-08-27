use crate::parser::{tokens::clean_one::Literal, ParseError, QueryParser};

pub struct ProjectionParser;

impl ProjectionParser {
    pub fn is_projection_start(parser: &QueryParser) -> bool {
        parser.comparers.select.compare(parser)
    }

    pub fn is_projection_end(parser: &QueryParser) -> bool {
        parser.comparers.from.compare(parser)
    }

    pub fn parse(parser: &mut QueryParser) -> Result<Vec<Literal>, ParseError> {
        // let pivot = parser.position;
        // let mut args: Vec<Literal> = vec![];
        // let mut can_consume = true;

        // if !ProjectionParser::is_projection_start(parser) {
        //     return Err(ParseError::new("Invalid projection value", pivot, parser));
        // }
        // parser.jump(parser.comparers.select.length);
        // // pivot = parser.position;

        // while !parser.eof() && !ProjectionParser::is_projection_end(parser) {
        //     if parser.current().is_whitespace() || parser.current() == ',' {
        //         parser.next();
        //     } else {
        //         args.push(Literal::parse(parser)?);
        //     }
        // }

        // if parser.eof() {
        //     return Err(ParseError::new("Invalid args value", pivot, parser));
        // }

        // Ok(args)
        panic!()
    }
}
