use crate::parser::{tokens::clean_one::{BoolParser, Column, NullParser, NumberParser, StringParser}, ParseError, QueryParser};

#[derive(Debug, Clone)]
pub enum Literal {
    String(String),
    Int(i32),
    Float(f32),
    Bool(bool),
    Null,
    Column { column: Column, alias: Option<String> }
}

// impl Literal {
//     pub fn parse(parser: &mut QueryParser) -> Result<Literal, ParseError> {
//         if NumberParser::is_number(parser) {
//             return NumberParser::parse(parser);
//         }
//         if StringParser::is_string_delimiter(parser) {
//             return StringParser::parse(parser);
//         }
//         if BoolParser::is_bool(parser) {
//             return BoolParser::parse(parser);
//         }

//         if NullParser::is_null(parser) {
//             return  NullParser::parse(parser);
//         }

//         let column = Column::parse_column_or_function(parser)?;

//         let alias = if parser.current().is_whitespace() && !parser.eof() {
//             parser.next();
//             if parser.comparers.alias.compare(parser) {
//                 parser.jump(parser.comparers.alias.length);
//                 let alias = Column::parse_column_or_function(parser)?;
//                 match alias {
//                     Column::Name { name } => Some(name),
//                     Column::WithCollection { collection: _, name: _ } =>
//                         return Err(ParseError::new("Invalid identifier for alias", parser.position, parser)),
//                 }
//             } else {
//                 return Err(ParseError::new("Invalid identifier for alias", parser.position, parser));
//             }
//         } else {
//             None
//         };

//         Ok(Literal::Column { column, alias })
//     }
// }
