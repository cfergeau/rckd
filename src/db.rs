use diesel::prelude::*;
use diesel::sqlite::SqliteConnection;
use rocket::serde::{Serialize, Deserialize};
use dotenvy::dotenv;
use std::env;

use crate::schema;

#[derive(Debug, Clone, Serialize, Deserialize, Queryable, Selectable)]
#[diesel(table_name = schema::elus)]
#[serde(crate = "rocket::serde")]
pub struct Person {
    pub id: i32,
    pub name: String,
    pub email: String,
    pub mandates: String,
}

#[derive(Insertable)]
#[diesel(table_name = schema::elus)]
pub struct NewPerson {
    pub name: String,
    pub email: String,
    pub mandates: String,
}

impl Person {
    pub fn new(name: String, email: String, mandates: String) -> NewPerson {
        NewPerson { name, email, mandates }
    }
}

pub fn establish_connection() -> SqliteConnection {
    dotenv().ok();
    let database_url = env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set");
    SqliteConnection::establish(&database_url)
        .unwrap_or_else(|_| panic!("Error connecting to {}", database_url))
}
