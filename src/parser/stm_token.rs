// SELECT b.*, a.full_name as name, COUNT(*) as TotBy, * V
// FROM TableA A V
// INNER JOIN TableB B ON A.id = B.id V
// INNER JOIN (query...) Q ON Q.id = B.q_id V
// WHERE A.Age > 16 AND (B.city = 'Porto' OR B.city like "Matosinhos") V
// GROUP BY a.full_name V
// HAVING COUNT(*) > 3
// ORDER BY b.description DESC V


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


