use std::error::Error;

mod collection_crud;
mod complex_queries;
mod database_and_ids;
mod helpers;
mod load_save;
mod metadata;
mod queries;
mod references;
mod sales_data;
mod schema_loading;

fn main() -> Result<(), Box<dyn Error>> {
    println!("FOSK full feature demo");
    println!("======================");
    println!("This example is executable documentation. It prints what each feature does");
    println!("and keeps a few assertions only to make sure the demo stays correct.\n");

    database_and_ids::run()?;
    collection_crud::run()?;
    load_save::run()?;
    complex_queries::run()?;

    let sales_db = sales_data::seed()?;
    queries::run(&sales_db)?;
    references::run(&sales_db)?;
    schema_loading::run()?;
    metadata::run()?;

    println!("\nDemo completed successfully.");
    Ok(())
}
