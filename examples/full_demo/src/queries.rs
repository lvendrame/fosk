use fosk::Db;
use serde_json::json;

use crate::helpers::pretty;

pub fn run(db: &Db) {
    println!("== SQL queries and reports ==");

    // Simple query: filter and order.
    let people = db
        .query("SELECT full_name, age FROM People WHERE age >= 30 ORDER BY age ASC")
        .unwrap();
    println!("People age >= 30: {}", pretty(&people));

    // Parameterized query: one ? placeholder receives one JSON value.
    let city_rows = db
        .query_with_args(
            "SELECT full_name, city FROM People WHERE city = ? ORDER BY full_name",
            json!("Lisboa"),
        )
        .unwrap();
    println!("People from Lisboa: {}", pretty(&city_rows));

    // IN (?) accepts an array wrapped as the parameter value.
    let selected_products = db
        .query_with_args(
            "SELECT name, price FROM Products WHERE id IN (?) ORDER BY id",
            json!([[1, 3]]),
        )
        .unwrap();
    println!(
        "Products selected with IN (?): {}",
        pretty(&selected_products)
    );

    // Aggregate report inspired by the executor tests.
    let city_report = db
        .query(
            r#"
            SELECT city,
                   COUNT(*) AS people,
                   AVG(age) AS avg_age,
                   MIN(age) AS min_age,
                   MAX(age) AS max_age
            FROM People
            GROUP BY city
            HAVING COUNT(*) >= 1
            ORDER BY people DESC, city ASC
            LIMIT 3
        "#,
        )
        .unwrap();
    println!("Top city report: {}", pretty(&city_report));

    // Join report across four collections.
    let sales_report = db
        .query(
            r#"
            SELECT
                p.city AS city,
                pr.category AS category,
                SUM(oi.quantity) AS items
            FROM People p
            JOIN Orders o ON o.person_id = p.id
            JOIN OrderItems oi ON oi.order_id = o.id
            JOIN Products pr ON pr.id = oi.product_id
            GROUP BY p.city, pr.category
            HAVING SUM(oi.quantity) >= 1
            ORDER BY city ASC, items DESC, category ASC
        "#,
        )
        .unwrap();
    println!("Sales by city/category: {}", pretty(&sales_report));

    assert!(!people.is_empty());
    assert_eq!(city_rows.len(), 2);
    assert!(!sales_report.is_empty());
    println!();
}
