#[cfg(test)]
pub mod fixtures {
    use serde_json::{json, Value};
    use crate::database::{Config, Db, DbCollection, DbCommon, DbRunner, IdType};

    pub fn create_people(db: &Db) {
        let mut people = db.clone().create("People");
        let rows = json!([
            { "id": 1,  "full_name": "Alice Johnson",    "age": 29, "city": "Porto",    "vip": true  },
            { "id": 2,  "full_name": "Bruno Martins",    "age": 34, "city": "Lisboa",   "vip": false },
            { "id": 3,  "full_name": "Carla Sousa",      "age": 41, "city": "Braga",    "vip": false },
            { "id": 4,  "full_name": "David Pereira",    "age": 25, "city": "Coimbra",  "vip": true  },
            { "id": 5,  "full_name": "Elisa Ramos",      "age": 38, "city": "Aveiro",   "vip": false },
            { "id": 6,  "full_name": "Fernando Lopes",   "age": 47, "city": "Porto",    "vip": false },
            { "id": 7,  "full_name": "Gabriela Costa",   "age": 30, "city": "Lisboa",   "vip": true  },
            { "id": 8,  "full_name": "Hugo Fernandes",   "age": 33, "city": "Guimarães","vip": false },
            { "id": 9,  "full_name": "Inês Almeida",     "age": 27, "city": "Braga",    "vip": false },
            { "id": 10, "full_name": "João Rocha",       "age": 36, "city": "Lisboa",   "vip": false },
            { "id": 11, "full_name": "Katia Figueiredo", "age": 44, "city": "Coimbra",  "vip": true  },
            { "id": 12, "full_name": "Luis Carvalho",    "age": 28, "city": "Porto",    "vip": false },
            { "id": 13, "full_name": "Marta Nunes",      "age": 35, "city": "Faro",     "vip": false },
            { "id": 14, "full_name": "Nuno Teixeira",    "age": 32, "city": "Évora",    "vip": true  },
            { "id": 15, "full_name": "Olga Ferreira",    "age": 39, "city": "Lisboa",   "vip": false }
        ]);
        let _ = people.load_from_json(rows).unwrap();
    }

    pub fn create_products(db: &Db) {
        let mut products = db.clone().create("Products");
        let rows = json!([
            { "id": 1,  "name": "Laptop Pro 15",           "category": "Electronics", "price": 1200.50 },
            { "id": 2,  "name": "Wireless Mouse",          "category": "Electronics", "price": 25.99   },
            { "id": 3,  "name": "Bluetooth Headphones",    "category": "Electronics", "price": 89.90   },
            { "id": 4,  "name": "Smartphone X200",         "category": "Electronics", "price": 699.00  },
            { "id": 5,  "name": "Office Chair Deluxe",     "category": "Furniture",   "price": 230.00  },
            { "id": 6,  "name": "Standing Desk",           "category": "Furniture",   "price": 499.00  },
            { "id": 7,  "name": "Espresso Machine",        "category": "Appliances",  "price": 320.75  },
            { "id": 8,  "name": "Air Fryer Compact",       "category": "Appliances",  "price": 99.90   },
            { "id": 9,  "name": "Electric Kettle",         "category": "Appliances",  "price": 35.50   },
            { "id": 10, "name": "Running Shoes Alpha",     "category": "Sports",      "price": 120.00  },
            { "id": 11, "name": "Yoga Mat Eco",            "category": "Sports",      "price": 45.00   },
            { "id": 12, "name": "Mountain Bike Trailblazer","category": "Sports",     "price": 899.99  },
            { "id": 13, "name": "Fiction Book 'Horizons'", "category": "Books",       "price": 18.50   },
            { "id": 14, "name": "Cookbook 'Mediterranean'", "category": "Books",      "price": 24.00   },
            { "id": 15, "name": "Notebook A5",             "category": "Stationery",  "price": 3.20    }
        ]);
        let _ = products.load_from_json(rows).unwrap();
    }

    pub fn create_orders(db: &Db) {
        let mut orders = db.clone().create("Orders");
        let rows = json!([
            { "id": 1,  "person_id": 1,  "product_id": 4,  "quantity": 1, "order_date": "2025-02-01", "status": "new" },
            { "id": 2,  "person_id": 2,  "product_id": 1,  "quantity": 1, "order_date": "2025-02-02", "status": "processing" },
            { "id": 3,  "person_id": 3,  "product_id": 7,  "quantity": 2, "order_date": "2025-02-03", "status": "shipped" },
            { "id": 4,  "person_id": 4,  "product_id": 10, "quantity": 1, "order_date": "2025-02-04", "status": "delivered" },
            { "id": 5,  "person_id": 5,  "product_id": 12, "quantity": 1, "order_date": "2025-02-05", "status": "delivered" },
            { "id": 6,  "person_id": 6,  "product_id": 8,  "quantity": 1, "order_date": "2025-02-06", "status": "processing" },
            { "id": 7,  "person_id": 7,  "product_id": 2,  "quantity": 3, "order_date": "2025-02-07", "status": "new" },
            { "id": 8,  "person_id": 8,  "product_id": 3,  "quantity": 1, "order_date": "2025-02-08", "status": "shipped" },
            { "id": 9,  "person_id": 9,  "product_id": 5,  "quantity": 1, "order_date": "2025-02-09", "status": "delivered" },
            { "id": 10, "person_id": 10, "product_id": 6,  "quantity": 2, "order_date": "2025-02-10", "status": "delivered" },
            { "id": 11, "person_id": 11, "product_id": 9,  "quantity": 1, "order_date": "2025-02-11", "status": "processing" },
            { "id": 12, "person_id": 12, "product_id": 11, "quantity": 1, "order_date": "2025-02-12", "status": "delivered" },
            { "id": 13, "person_id": 13, "product_id": 14, "quantity": 1, "order_date": "2025-02-13", "status": "shipped" },
            { "id": 14, "person_id": 14, "product_id": 13, "quantity": 2, "order_date": "2025-02-14", "status": "delivered" },
            { "id": 15, "person_id": 15, "product_id": 15, "quantity": 1, "order_date": "2025-02-15", "status": "cancelled" },
            { "id": 16, "person_id": 1,  "product_id": 2,  "quantity": 1, "order_date": "2025-02-16", "status": "delivered" },
            { "id": 17, "person_id": 2,  "product_id": 3,  "quantity": 1, "order_date": "2025-02-17", "status": "delivered" },
            { "id": 18, "person_id": 3,  "product_id": 1,  "quantity": 1, "order_date": "2025-02-18", "status": "processing" },
            { "id": 19, "person_id": 4,  "product_id": 12, "quantity": 1, "order_date": "2025-02-19", "status": "new" },
            { "id": 20, "person_id": 5,  "product_id": 4,  "quantity": 2, "order_date": "2025-02-20", "status": "shipped" },
            { "id": 21, "person_id": 6,  "product_id": 10, "quantity": 1, "order_date": "2025-02-21", "status": "delivered" },
            { "id": 22, "person_id": 7,  "product_id": 6,  "quantity": 1, "order_date": "2025-02-22", "status": "delivered" },
            { "id": 23, "person_id": 8,  "product_id": 7,  "quantity": 1, "order_date": "2025-02-23", "status": "processing" },
            { "id": 24, "person_id": 9,  "product_id": 8,  "quantity": 1, "order_date": "2025-02-24", "status": "delivered" },
            { "id": 25, "person_id": 10, "product_id": 5,  "quantity": 1, "order_date": "2025-02-25", "status": "delivered" },
            { "id": 26, "person_id": 11, "product_id": 11, "quantity": 2, "order_date": "2025-02-26", "status": "shipped" },
            { "id": 27, "person_id": 12, "product_id": 14, "quantity": 1, "order_date": "2025-02-27", "status": "new" },
            { "id": 28, "person_id": 13, "product_id": 9,  "quantity": 1, "order_date": "2025-02-28", "status": "processing" },
            { "id": 29, "person_id": 14, "product_id": 3,  "quantity": 1, "order_date": "2025-03-01", "status": "delivered" },
            { "id": 30, "person_id": 15, "product_id": 2,  "quantity": 4, "order_date": "2025-03-02", "status": "delivered" },
            { "id": 31, "person_id": 1,  "product_id": 1,  "quantity": 1, "order_date": "2025-03-03", "status": "shipped" },
            { "id": 32, "person_id": 2,  "product_id": 5,  "quantity": 1, "order_date": "2025-03-04", "status": "delivered" },
            { "id": 33, "person_id": 3,  "product_id": 6,  "quantity": 1, "order_date": "2025-03-05", "status": "delivered" },
            { "id": 34, "person_id": 4,  "product_id": 7,  "quantity": 3, "order_date": "2025-03-06", "status": "processing" },
            { "id": 35, "person_id": 5,  "product_id": 8,  "quantity": 1, "order_date": "2025-03-07", "status": "delivered" },
            { "id": 36, "person_id": 6,  "product_id": 9,  "quantity": 1, "order_date": "2025-03-08", "status": "delivered" },
            { "id": 37, "person_id": 7,  "product_id": 10, "quantity": 1, "order_date": "2025-03-09", "status": "delivered" },
            { "id": 38, "person_id": 8,  "product_id": 11, "quantity": 1, "order_date": "2025-03-10", "status": "shipped" },
            { "id": 39, "person_id": 9,  "product_id": 12, "quantity": 2, "order_date": "2025-03-11", "status": "delivered" },
            { "id": 40, "person_id": 10, "product_id": 4,  "quantity": 1, "order_date": "2025-03-12", "status": "delivered" }
        ]);
        let _ = orders.load_from_json(rows).unwrap();
    }

    pub fn create_order_items(db: &Db) {
        let mut items = db.clone().create("OrderItems");
        let rows = json!([
            // -- Orders 1..20 -> 3 items each (60 rows) --
            { "id": 1,  "order_id": 1,  "product_id": 4,  "quantity": 1, "unit_price": 699.00  },
            { "id": 2,  "order_id": 1,  "product_id": 2,  "quantity": 2, "unit_price": 25.99   },
            { "id": 3,  "order_id": 1,  "product_id": 9,  "quantity": 1, "unit_price": 35.50   },

            { "id": 4,  "order_id": 2,  "product_id": 1,  "quantity": 1, "unit_price": 1200.50 },
            { "id": 5,  "order_id": 2,  "product_id": 11, "quantity": 1, "unit_price": 45.00   },
            { "id": 6,  "order_id": 2,  "product_id": 15, "quantity": 3, "unit_price": 3.20    },

            { "id": 7,  "order_id": 3,  "product_id": 7,  "quantity": 1, "unit_price": 320.75  },
            { "id": 8,  "order_id": 3,  "product_id": 8,  "quantity": 1, "unit_price": 99.90   },
            { "id": 9,  "order_id": 3,  "product_id": 3,  "quantity": 2, "unit_price": 89.90   },

            { "id": 10, "order_id": 4,  "product_id": 10, "quantity": 1, "unit_price": 120.00  },
            { "id": 11, "order_id": 4,  "product_id": 5,  "quantity": 1, "unit_price": 230.00  },
            { "id": 12, "order_id": 4,  "product_id": 2,  "quantity": 1, "unit_price": 25.99   },

            { "id": 13, "order_id": 5,  "product_id": 12, "quantity": 1, "unit_price": 899.99  },
            { "id": 14, "order_id": 5,  "product_id": 14, "quantity": 1, "unit_price": 24.00   },
            { "id": 15, "order_id": 5,  "product_id": 13, "quantity": 2, "unit_price": 18.50   },

            { "id": 16, "order_id": 6,  "product_id": 8,  "quantity": 1, "unit_price": 99.90   },
            { "id": 17, "order_id": 6,  "product_id": 6,  "quantity": 1, "unit_price": 499.00  },
            { "id": 18, "order_id": 6,  "product_id": 11, "quantity": 2, "unit_price": 45.00   },

            { "id": 19, "order_id": 7,  "product_id": 2,  "quantity": 3, "unit_price": 25.99   },
            { "id": 20, "order_id": 7,  "product_id": 9,  "quantity": 1, "unit_price": 35.50   },
            { "id": 21, "order_id": 7,  "product_id": 15, "quantity": 2, "unit_price": 3.20    },

            { "id": 22, "order_id": 8,  "product_id": 3,  "quantity": 1, "unit_price": 89.90   },
            { "id": 23, "order_id": 8,  "product_id": 4,  "quantity": 1, "unit_price": 699.00  },
            { "id": 24, "order_id": 8,  "product_id": 10, "quantity": 1, "unit_price": 120.00  },

            { "id": 25, "order_id": 9,  "product_id": 5,  "quantity": 1, "unit_price": 230.00  },
            { "id": 26, "order_id": 9,  "product_id": 8,  "quantity": 1, "unit_price": 99.90   },
            { "id": 27, "order_id": 9,  "product_id": 13, "quantity": 1, "unit_price": 18.50   },

            { "id": 28, "order_id": 10, "product_id": 6,  "quantity": 1, "unit_price": 499.00  },
            { "id": 29, "order_id": 10, "product_id": 2,  "quantity": 2, "unit_price": 25.99   },
            { "id": 30, "order_id": 10, "product_id": 11, "quantity": 1, "unit_price": 45.00   },

            { "id": 31, "order_id": 11, "product_id": 9,  "quantity": 1, "unit_price": 35.50   },
            { "id": 32, "order_id": 11, "product_id": 3,  "quantity": 1, "unit_price": 89.90   },
            { "id": 33, "order_id": 11, "product_id": 15, "quantity": 4, "unit_price": 3.20    },

            { "id": 34, "order_id": 12, "product_id": 11, "quantity": 1, "unit_price": 45.00   },
            { "id": 35, "order_id": 12, "product_id": 5,  "quantity": 1, "unit_price": 230.00  },
            { "id": 36, "order_id": 12, "product_id": 1,  "quantity": 1, "unit_price": 1200.50 },

            { "id": 37, "order_id": 13, "product_id": 14, "quantity": 1, "unit_price": 24.00   },
            { "id": 38, "order_id": 13, "product_id": 2,  "quantity": 2, "unit_price": 25.99   },
            { "id": 39, "order_id": 13, "product_id": 8,  "quantity": 1, "unit_price": 99.90   },

            { "id": 40, "order_id": 14, "product_id": 13, "quantity": 2, "unit_price": 18.50   },
            { "id": 41, "order_id": 14, "product_id": 10, "quantity": 1, "unit_price": 120.00  },
            { "id": 42, "order_id": 14, "product_id": 4,  "quantity": 1, "unit_price": 699.00  },

            { "id": 43, "order_id": 15, "product_id": 15, "quantity": 2, "unit_price": 3.20    },
            { "id": 44, "order_id": 15, "product_id": 9,  "quantity": 1, "unit_price": 35.50   },
            { "id": 45, "order_id": 15, "product_id": 3,  "quantity": 1, "unit_price": 89.90   },

            { "id": 46, "order_id": 16, "product_id": 2,  "quantity": 1, "unit_price": 25.99   },
            { "id": 47, "order_id": 16, "product_id": 11, "quantity": 1, "unit_price": 45.00   },
            { "id": 48, "order_id": 16, "product_id": 7,  "quantity": 1, "unit_price": 320.75  },

            { "id": 49, "order_id": 17, "product_id": 3,  "quantity": 1, "unit_price": 89.90   },
            { "id": 50, "order_id": 17, "product_id": 5,  "quantity": 1, "unit_price": 230.00  },
        ]);
        let _ = items.load_from_json(rows).unwrap();

        let rows = json!([
            { "id": 51, "order_id": 17, "product_id": 14, "quantity": 1, "unit_price": 24.00   },

            { "id": 52, "order_id": 18, "product_id": 1,  "quantity": 1, "unit_price": 1200.50 },
            { "id": 53, "order_id": 18, "product_id": 2,  "quantity": 1, "unit_price": 25.99   },
            { "id": 54, "order_id": 18, "product_id": 10, "quantity": 1, "unit_price": 120.00  },

            { "id": 55, "order_id": 19, "product_id": 12, "quantity": 1, "unit_price": 899.99  },
            { "id": 56, "order_id": 19, "product_id": 8,  "quantity": 1, "unit_price": 99.90   },
            { "id": 57, "order_id": 19, "product_id": 15, "quantity": 2, "unit_price": 3.20    },

            { "id": 58, "order_id": 20, "product_id": 4,  "quantity": 1, "unit_price": 699.00  },
            { "id": 59, "order_id": 20, "product_id": 3,  "quantity": 1, "unit_price": 89.90   },
            { "id": 60, "order_id": 20, "product_id": 11, "quantity": 1, "unit_price": 45.00   },

            // -- Orders 21..35 -> 2 items each (30 rows) --
            { "id": 61, "order_id": 21, "product_id": 10, "quantity": 1, "unit_price": 120.00  },
            { "id": 62, "order_id": 21, "product_id": 9,  "quantity": 1, "unit_price": 35.50   },

            { "id": 63, "order_id": 22, "product_id": 6,  "quantity": 1, "unit_price": 499.00  },
            { "id": 64, "order_id": 22, "product_id": 2,  "quantity": 1, "unit_price": 25.99   },

            { "id": 65, "order_id": 23, "product_id": 7,  "quantity": 1, "unit_price": 320.75  },
            { "id": 66, "order_id": 23, "product_id": 3,  "quantity": 1, "unit_price": 89.90   },

            { "id": 67, "order_id": 24, "product_id": 8,  "quantity": 1, "unit_price": 99.90   },
            { "id": 68, "order_id": 24, "product_id": 14, "quantity": 1, "unit_price": 24.00   },

            { "id": 69, "order_id": 25, "product_id": 5,  "quantity": 1, "unit_price": 230.00  },
            { "id": 70, "order_id": 25, "product_id": 13, "quantity": 1, "unit_price": 18.50   },

            { "id": 71, "order_id": 26, "product_id": 11, "quantity": 2, "unit_price": 45.00   },
            { "id": 72, "order_id": 26, "product_id": 2,  "quantity": 1, "unit_price": 25.99   },

            { "id": 73, "order_id": 27, "product_id": 14, "quantity": 1, "unit_price": 24.00   },
            { "id": 74, "order_id": 27, "product_id": 9,  "quantity": 1, "unit_price": 35.50   },

            { "id": 75, "order_id": 28, "product_id": 9,  "quantity": 1, "unit_price": 35.50   },
            { "id": 76, "order_id": 28, "product_id": 3,  "quantity": 1, "unit_price": 89.90   },

            { "id": 77, "order_id": 29, "product_id": 3,  "quantity": 1, "unit_price": 89.90   },
            { "id": 78, "order_id": 29, "product_id": 10, "quantity": 1, "unit_price": 120.00  },

            { "id": 79, "order_id": 30, "product_id": 2,  "quantity": 2, "unit_price": 25.99   },
            { "id": 80, "order_id": 30, "product_id": 11, "quantity": 1, "unit_price": 45.00   },

            { "id": 81, "order_id": 31, "product_id": 1,  "quantity": 1, "unit_price": 1200.50 },
            { "id": 82, "order_id": 31, "product_id": 15, "quantity": 2, "unit_price": 3.20    },

            { "id": 83, "order_id": 32, "product_id": 5,  "quantity": 1, "unit_price": 230.00  },
            { "id": 84, "order_id": 32, "product_id": 8,  "quantity": 1, "unit_price": 99.90   },

            { "id": 85, "order_id": 33, "product_id": 6,  "quantity": 1, "unit_price": 499.00  },
            { "id": 86, "order_id": 33, "product_id": 2,  "quantity": 1, "unit_price": 25.99   },

            { "id": 87, "order_id": 34, "product_id": 7,  "quantity": 1, "unit_price": 320.75  },
            { "id": 88, "order_id": 34, "product_id": 15, "quantity": 4, "unit_price": 3.20    },

            { "id": 89, "order_id": 35, "product_id": 8,  "quantity": 1, "unit_price": 99.90   },
            { "id": 90, "order_id": 35, "product_id": 13, "quantity": 1, "unit_price": 18.50   },

            // -- Orders 36..40 -> 2 items each (10 rows) --
            { "id": 91, "order_id": 36, "product_id": 9,  "quantity": 1, "unit_price": 35.50   },
            { "id": 92, "order_id": 36, "product_id": 10, "quantity": 1, "unit_price": 120.00  },

            { "id": 93, "order_id": 37, "product_id": 10, "quantity": 1, "unit_price": 120.00  },
            { "id": 94, "order_id": 37, "product_id": 3,  "quantity": 1, "unit_price": 89.90   },

            { "id": 95, "order_id": 38, "product_id": 11, "quantity": 1, "unit_price": 45.00   },
            { "id": 96, "order_id": 38, "product_id": 2,  "quantity": 1, "unit_price": 25.99   },

            { "id": 97, "order_id": 39, "product_id": 12, "quantity": 1, "unit_price": 899.99  },
            { "id": 98, "order_id": 39, "product_id": 9,  "quantity": 1, "unit_price": 35.50   },

            { "id": 99, "order_id": 40, "product_id": 4,  "quantity": 1, "unit_price": 699.00  },
            { "id": 100,"order_id": 40, "product_id": 13, "quantity": 1, "unit_price": 18.50   }
        ]);
        let _ = items.load_from_json(rows).unwrap();
    }

    pub fn seed_db() -> Db {
        let db = Db::new_db_with_config(Config {
            id_type: IdType::None,
            id_key: "id".into(),
        });

        create_people(&db);
        create_products(&db);
        create_orders(&db);
        create_order_items(&db);

        db
    }

        fn ids_in(coll: &crate::database::MemoryCollection) -> std::collections::HashSet<i64> {
        coll.get_all()
            .into_iter()
            .filter_map(|v| {
                v.get("id")
                    .and_then(|id| match id {
                        Value::Number(n) => n.as_i64(),
                        Value::String(s) => s.parse::<i64>().ok(),
                        _ => None,
                    })
            })
            .collect()
    }

    fn product_price_map(coll: &crate::database::MemoryCollection) -> std::collections::HashMap<i64, f64> {
        coll.get_all()
            .into_iter()
            .filter_map(|v| {
                let id = v.get("id")
                    .and_then(|id| match id {
                        Value::Number(n) => n.as_i64(),
                        Value::String(s) => s.parse::<i64>().ok(),
                        _ => None
                    })?;
                let price = v.get("price")
                    .and_then(|p| match p {
                        Value::Number(n) => n.as_f64(),
                        Value::String(s) => s.parse::<f64>().ok(),
                        _ => None,
                    })?;
                Some((id, price))
            })
            .collect()
    }

    #[test]
    fn seed_creates_all_collections_and_counts() {
        let db = seed_db();

        let people   = db.get("People").expect("People collection missing");
        let products = db.get("Products").expect("Products collection missing");
        let orders   = db.get("Orders").expect("Orders collection missing");
        let items    = db.get("OrderItems").expect("OrderItems collection missing");

        assert_eq!(people.count(), 15, "expected 15 people");
        assert_eq!(products.count(), 15, "expected 15 products");
        assert_eq!(orders.count(), 40, "expected 40 orders");
        assert_eq!(items.count(), 100, "expected 100 order items");

        // Presence checks against your actual seed:
        let people_names: std::collections::HashSet<String> = people.get_all().into_iter()
            .filter_map(|v| v.get("full_name").and_then(|n| n.as_str()).map(|s| s.to_string()))
            .collect();
        assert!(people_names.contains("Alice Johnson"));
        assert!(people_names.contains("Nuno Teixeira"));

        // Bonus: sanity check VIP count (ids 1,4,7,11,14 in your seed)
        let vip_count = people.get_all().into_iter()
            .filter(|v| v.get("vip").and_then(|b| b.as_bool()).unwrap_or(false))
            .count();
        assert_eq!(vip_count, 5, "expected 5 VIPs");
    }

    #[test]
    fn referential_integrity_orders_and_items() {
        let db = seed_db();

        let people = db.get("People").unwrap();
        let products = db.get("Products").unwrap();
        let orders = db.get("Orders").unwrap();
        let items = db.get("OrderItems").unwrap();

        let person_ids = ids_in(&people);
        let product_ids = ids_in(&products);
        let order_ids = ids_in(&orders);

        // Orders.person_id must exist in People
        for o in orders.get_all() {
            let pid = o.get("person_id")
                .and_then(|v| v.as_i64())
                .expect("Orders.person_id must be i64");
            assert!(person_ids.contains(&pid), "Orders.person_id {} not found in People", pid);
        }

        // OrderItems.order_id must exist in Orders
        // OrderItems.product_id must exist in Products
        for it in items.get_all() {
            let oid = it.get("order_id")
                .and_then(|v| v.as_i64())
                .expect("OrderItems.order_id must be i64");
            assert!(order_ids.contains(&oid), "OrderItems.order_id {} not found in Orders", oid);

            let pid = it.get("product_id")
                .and_then(|v| v.as_i64())
                .expect("OrderItems.product_id must be i64");
            assert!(product_ids.contains(&pid), "OrderItems.product_id {} not found in Products", pid);
        }
    }

    #[test]
    fn order_item_unit_prices_match_product_prices() {
        let db = seed_db();

        let products = db.get("Products").unwrap();
        let items = db.get("OrderItems").unwrap();

        let price_by_product = product_price_map(&products);

        for it in items.get_all() {
            let pid = it.get("product_id")
                .and_then(|v| v.as_i64())
                .expect("OrderItems.product_id must be i64");
            let item_price = it.get("unit_price")
                .and_then(|v| v.as_f64())
                .expect("OrderItems.unit_price must be f64");

            let expected = price_by_product.get(&pid)
                .unwrap_or_else(|| panic!("Product {} must exist to compare price", pid));

            // allow tiny float noise, though we seeded exact values
            assert!((item_price - *expected).abs() < 1e-9,
                "unit_price {} does not match product {} price {}", item_price, pid, expected);
        }
    }

    fn get_f64(v: &Value, k: &str) -> f64 {
        v.get(k)
            .and_then(|x| x.as_f64())
            .unwrap_or_else(|| panic!("missing/invalid numeric field {k} in row: {v:?}"))
    }

    fn get_str<'a>(v: &'a Value, k: &str) -> &'a str {
        v.get(k)
            .and_then(|x| x.as_str())
            .unwrap_or_else(|| panic!("missing/invalid string field {k} in row: {v:?}"))
    }

    #[test]
    fn report_sold_items_by_person_via_sql() {
        // Seed
        let db = seed_db();

        // The report: items sold per person (line count and total quantity).
        // We use a FROM-list cross join People×Orders and an INNER JOIN to OrderItems,
        // with a WHERE correlating Orders.person_id to People.id.
        let sql = r#"
            SELECT
              p.full_name AS person,
              SUM(oi.quantity) AS items_sold,
              COUNT(*)        AS lines
            FROM People p, Orders o
            INNER JOIN OrderItems oi ON oi.order_id = o.id
            WHERE o.person_id = p.id
            GROUP BY p.full_name
            HAVING SUM(oi.quantity) > 0
            ORDER BY items_sold DESC, person ASC
        "#;

        let rows = db.query(sql).expect("report query should run");
        assert!(!rows.is_empty(), "report should return at least one person");

        // Shape + monotonic ordering + positivity checks
        let mut last_items: Option<f64> = None;
        for r in &rows {
            let _obj = r.as_object().expect("each row should be an object");
            // Required fields
            let _person = get_str(r, "person");
            let items_sold = get_f64(r, "items_sold");
            let lines = get_f64(r, "lines");

            assert!(items_sold >= 0.0, "items_sold must be non-negative");
            assert!(lines >= 1.0, "each grouped person should have at least 1 line");

            // DESC items_sold, then ASC person — check DESC part
            if let Some(prev) = last_items {
                assert!(
                    items_sold <= prev + 1e-9,
                    "items_sold must be non-increasing (DESC). prev={prev}, curr={items_sold}"
                );
            }
            last_items = Some(items_sold);
        }

        // Cross-check a global invariant:
        // Sum of per-person items_sold should equal SUM(quantity) from OrderItems
        // (because the FROM/WHERE picks orders joined to their owner).
        let total_from_report: f64 = rows.iter().map(|r| get_f64(r, "items_sold")).sum();

        let grand_sql = r#"SELECT SUM(quantity) AS total_qty FROM OrderItems"#;
        let grand_rows = db.query(grand_sql).expect("grand total query should run");
        assert_eq!(grand_rows.len(), 1, "grand total query should return one row");
        let total_qty = grand_rows[0]
            .get("total_qty")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);

        // They should match (tiny float epsilon).
        assert!(
            (total_from_report - total_qty).abs() < 1e-9,
            "report total ({total_from_report}) must equal SUM(quantity) from OrderItems ({total_qty})"
        );
    }

}
