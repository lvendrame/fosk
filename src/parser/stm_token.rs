// SELECT b.*, a.full_name as name, COUNT(*) as TotBy, *
// FROM TableA A
// INNER JOIN TableB B ON A.id = B.id
// INNER JOIN (query...) Q ON Q.id = B.q_id
// WHERE A.Age > 16 AND (B.city = 'Porto' OR B.city like "Matosinhos")
// GROUP BY a.full_name
// HAVING COUNT(*) > 3
// ORDER BY b.description DESC


pub enum StmToken {

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


