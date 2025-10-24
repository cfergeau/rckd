use diesel::prelude::*;
use diesel::sqlite::SqliteConnection;
use rocket::serde::{Serialize, Deserialize};
use rocket::http::Status;
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

pub fn establish_connection() -> SqliteConnection {
    dotenv().ok();
    let database_url = env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set");
    SqliteConnection::establish(&database_url)
        .unwrap_or_else(|_| panic!("Error connecting to {}", database_url))
}

pub fn email_exists(email_to_check: &str, connection: &mut SqliteConnection) -> bool {
    use self::schema::elus::dsl::*;

    elus
        .filter(email.eq(email_to_check))
        .select(Person::as_select())
        .first(connection)
        .is_ok()
}

pub fn name_exists(name_to_check: &str, connection: &mut SqliteConnection) -> bool {
    use self::schema::elus::dsl::*;

    elus
        .filter(name.eq(name_to_check))
        .select(Person::as_select())
        .first(connection)
        .is_ok()
}

pub fn insert_person(person_name: String, person_email: String, person_mandates: String, connection: &mut SqliteConnection) -> Result<(), Status> {
    use self::schema::elus::dsl::*;

    let new_person = NewPerson {
        name: person_name,
        email: person_email,
        mandates: person_mandates,
    };

    diesel::insert_into(elus)
        .values(&new_person)
        .execute(connection)
        .map_err(|_| Status::InternalServerError)?;

    Ok(())
}

pub fn elus(connection: &mut SqliteConnection) -> Result<Vec<Person>, Status> {
    use self::schema::elus::dsl::*;

    elus
        .select(Person::as_select())
        .load(connection)
        .map_err(|_| Status::InternalServerError)
}

pub fn get_elu_by_email(email_to_find: &str, connection: &mut SqliteConnection) -> Result<Person, Status> {
    use self::schema::elus::dsl::*;

    elus
        .filter(email.eq(email_to_find))
        .select(Person::as_select())
        .first(connection)
        .map_err(|_| Status::NotFound)
}
