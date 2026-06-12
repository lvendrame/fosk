use std::error::Error;

use fosk::Db;
use serde_json::json;

use crate::{
    helpers::{app_file, boxed_debug, pretty},
    load_save::collection_fixtures,
};

pub fn run() -> Result<(), Box<dyn Error>> {
    println!("== Queries over complex collection files ==");

    let db = Db::new();
    for fixture in collection_fixtures() {
        let collection = db.create_with_config(fixture.name, fixture.config);
        let path = app_file(fixture.file).into_os_string();
        collection.load_from_file(&path)?;
    }
    println!("Loaded query collections: {:?}", db.list_collections());

    // Boolean filtering, comparison, ordering, and LIMIT against UUID-id rows.
    let private_companies = db
        .query(
            r#"
            SELECT name, industry, employees, revenue
            FROM Companies
            WHERE isPublic = false AND employees >= 90
            ORDER BY employees DESC
            LIMIT 2
        "#,
        )
        .map_err(boxed_debug)?;
    println!(
        "Private companies with 90+ employees: {}",
        pretty(&private_companies)
    );

    // LIKE is case-insensitive. OFFSET/LIMIT make the result page-shaped.
    let product_page = db
        .query(
            r#"
            SELECT name, category, brand, price, rating
            FROM Products
            WHERE name LIKE '%o%' AND inStock = true
            ORDER BY rating DESC, price ASC
            OFFSET 0
            LIMIT 3
        "#,
        )
        .map_err(boxed_debug)?;
    println!("Product search page: {}", pretty(&product_page));

    // Parameterized IN receives an array wrapped as the single argument value.
    let selected_cities = db
        .query_with_args(
            r#"
            SELECT name, country, population
            FROM Cities
            WHERE country IN (?)
            ORDER BY population DESC
        "#,
            json!([["USA", "Portugal"]]),
        )
        .map_err(boxed_debug)?;
    println!(
        "Cities selected with IN parameter: {}",
        pretty(&selected_cities)
    );

    // NULL predicates are useful for fixture data that intentionally has gaps.
    let orders_with_notes = db
        .query(
            r#"
            SELECT id, customerId, status, notes
            FROM Orders
            WHERE notes IS NOT NULL
            ORDER BY id
        "#,
        )
        .map_err(boxed_debug)?;
    println!("Orders with notes: {}", pretty(&orders_with_notes));

    // OR and NOT IN combine normal scalar predicates.
    let support_focus = db
        .query(
            r#"
            SELECT ticket_id, priority, status, subject
            FROM Support_Tickets
            WHERE priority = 'high' OR status NOT IN ('closed')
            ORDER BY ticket_id
        "#,
        )
        .map_err(boxed_debug)?;
    println!(
        "Support tickets needing attention: {}",
        pretty(&support_focus)
    );

    // Join the separately loaded order and order item files, then aggregate.
    let order_totals = db
        .query(
            r#"
            SELECT
                o.status AS status,
                COUNT(DISTINCT o.id) AS orders,
                SUM(oi.quantity) AS items,
                AVG(oi.price) AS avg_line_price
            FROM Orders o
            JOIN Order_Items oi ON oi.order_id = o.id
            GROUP BY o.status
            HAVING SUM(oi.quantity) >= 1
            ORDER BY items DESC, status ASC
        "#,
        )
        .map_err(boxed_debug)?;
    println!("Order item totals by status: {}", pretty(&order_totals));

    // Join custom None:String IDs with an explicit foreign-key-like column.
    let inventory_by_warehouse = db
        .query(
            r#"
            SELECT
                w.name AS warehouse,
                w.country AS country,
                COUNT(*) AS movements,
                SUM(im.delta) AS net_delta
            FROM Warehouses w
            JOIN Inventory_Movements im ON im.warehouse_id = w.code
            GROUP BY w.name, w.country
            ORDER BY net_delta DESC
        "#,
        )
        .map_err(boxed_debug)?;
    println!(
        "Inventory movement totals by warehouse: {}",
        pretty(&inventory_by_warehouse)
    );

    // Mixed explicit IDs and nullable user data: user rows joined to token rows.
    let token_owners = db
        .query(
            r#"
            SELECT u.username, u.email, t.scope, t.revoked
            FROM Users u
            JOIN Auth_Tokens t ON t.user_id = u.id
            WHERE t.revoked = false
            ORDER BY u.username
        "#,
        )
        .map_err(boxed_debug)?;
    println!("Active token owners: {}", pretty(&token_owners));

    // Blog/comments demonstrates string IDs and a second join predicate shape.
    let comment_report = db
        .query(
            r#"
            SELECT
                bp.title AS post,
                COUNT(c.id) AS comments
            FROM Blog_Posts bp
            JOIN Comments c ON c.post_slug = bp.slug
            GROUP BY bp.title
            ORDER BY comments DESC
        "#,
        )
        .map_err(boxed_debug)?;
    println!("Comments per blog post: {}", pretty(&comment_report));

    assert!(!private_companies.is_empty());
    assert!(!product_page.is_empty());
    assert!(selected_cities.len() >= 3);
    assert_eq!(orders_with_notes.len(), 1);
    assert!(!order_totals.is_empty());
    assert!(!inventory_by_warehouse.is_empty());
    assert_eq!(token_owners.len(), 2);
    assert_eq!(comment_report.len(), 1);
    println!();

    Ok(())
}
