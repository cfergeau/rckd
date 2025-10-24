#[macro_use] extern crate rocket;

mod schema;

use diesel::prelude::*;
use diesel::sqlite::SqliteConnection;
use rocket::serde::{Serialize, Deserialize, json::Json};
use rocket::State;
use rocket::http::Status;
use dotenvy::dotenv;
use std::env;
use std::sync::Mutex;

#[derive(Debug, Clone, Serialize, Deserialize, Queryable, Selectable)]
#[diesel(table_name = schema::elus)]
#[serde(crate = "rocket::serde")]
struct PersonDB {
    id: i32,
    name: String,
    email: String,
    mandates: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
struct Person {
    name: String,
    email: String,
    mandates: Vec<String>,
}

impl From<PersonDB> for Person {
    fn from(person: PersonDB) -> Self {
        let mandates: Vec<String> = serde_json::from_str(&person.mandates)
            .unwrap_or_else(|_| vec![]);
        Person {
            name: person.name,
            email: person.email,
            mandates,
        }
    }
}

#[derive(Insertable)]
#[diesel(table_name = schema::elus)]
struct NewPerson {
    name: String,
    email: String,
    mandates: String,
}

type DbConn = Mutex<SqliteConnection>;

fn establish_connection() -> SqliteConnection {
    dotenv().ok();
    let database_url = env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set");
    SqliteConnection::establish(&database_url)
        .unwrap_or_else(|_| panic!("Error connecting to {}", database_url))
}

#[get("/")]
fn index() -> &'static str {
    "hello world"
}

#[get("/elus")]
fn elus(db: &State<DbConn>) -> Json<Vec<Person>> {
    use self::schema::elus::dsl::*;

    let mut connection = db.lock().unwrap();
    let results = elus
        .select(PersonDB::as_select())
        .load(&mut *connection)
        .expect("Error loading persons");

    let responses: Vec<Person> = results.into_iter()
        .map(Person::from)
        .collect();

    Json(responses)
}

#[get("/elus/<search_email>")]
fn get_person_by_email(search_email: String, db: &State<DbConn>) -> Option<Json<Person>> {
    use self::schema::elus::dsl::*;

    let mut connection = db.lock().unwrap();
    let result = elus
        .filter(email.eq(&search_email))
        .select(PersonDB::as_select())
        .first(&mut *connection)
        .ok()?;

    Some(Json(Person::from(result)))
}

#[post("/elus/new", data = "<person_data>")]
fn create_person_new(person_data: Json<Person>, db: &State<DbConn>) -> Result<Json<Person>, Status> {
    create_person(person_data, db)
}

#[post("/elus/create", data = "<person_data>")]
fn create_person_create(person_data: Json<Person>, db: &State<DbConn>) -> Result<Json<Person>, Status> {
    create_person(person_data, db)
}

fn create_person(person_data: Json<Person>, db: &State<DbConn>) -> Result<Json<Person>, Status> {
    use self::schema::elus::dsl::*;

    let mut connection = db.lock().unwrap();

    // Check if email already exists
    let email_exists = elus
        .filter(email.eq(&person_data.email))
        .select(PersonDB::as_select())
        .first(&mut *connection)
        .is_ok();

    if email_exists {
        return Err(Status::Conflict);
    }

    // Check if name already exists
    let name_exists = elus
        .filter(name.eq(&person_data.name))
        .select(PersonDB::as_select())
        .first(&mut *connection)
        .is_ok();

    if name_exists {
        return Err(Status::Conflict);
    }

    // Create new person
    let new_person = NewPerson {
        name: person_data.name.clone(),
        email: person_data.email.clone(),
        mandates: serde_json::to_string(&person_data.mandates).unwrap(),
    };

    diesel::insert_into(elus)
        .values(&new_person)
        .execute(&mut *connection)
        .map_err(|_| Status::InternalServerError)?;

    // Return the created person
    let created = elus
        .filter(email.eq(&person_data.email))
        .select(PersonDB::as_select())
        .first(&mut *connection)
        .map_err(|_| Status::InternalServerError)?;

    Ok(Json(Person::from(created)))
}

#[launch]
fn rocket() -> _ {
    let connection = establish_connection();
    rocket::build()
        .manage(Mutex::new(connection))
        .mount("/", routes![index, elus, get_person_by_email, create_person_new, create_person_create])
}

#[cfg(test)]
mod tests {
    use super::*;
    use rocket::local::blocking::Client;
    use rocket::http::Status;

    fn setup_test_db() -> SqliteConnection {
        let mut connection = SqliteConnection::establish(":memory:")
            .expect("Failed to create in-memory database");

        // Run migrations
        diesel::sql_query("CREATE TABLE elus (
            id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
            name TEXT NOT NULL,
            email TEXT NOT NULL UNIQUE,
            mandates TEXT NOT NULL
        )")
        .execute(&mut connection)
        .expect("Failed to create table");

        connection
    }

    fn insert_test_persons(connection: &mut SqliteConnection) {
        use self::schema::elus;

        let persons = vec![
            NewPerson {
                name: "Jean Dupont".to_string(),
                email: "jean.dupont@example.com".to_string(),
                mandates: serde_json::to_string(&vec!["Maire", "Conseiller régional"]).unwrap(),
            },
            NewPerson {
                name: "Marie Martin".to_string(),
                email: "marie.martin@example.com".to_string(),
                mandates: serde_json::to_string(&vec!["Députée"]).unwrap(),
            },
            NewPerson {
                name: "Pierre Durand".to_string(),
                email: "pierre.durand@example.com".to_string(),
                mandates: serde_json::to_string(&vec!["Sénateur", "Conseiller municipal"]).unwrap(),
            },
        ];

        diesel::insert_into(elus::table)
            .values(&persons)
            .execute(connection)
            .expect("Failed to insert test data");
    }

    #[test]
    fn test_hello_world() {
        let connection = setup_test_db();
        let rocket = rocket::build()
            .manage(Mutex::new(connection))
            .mount("/", routes![index, elus, get_person_by_email]);

        let client = Client::tracked(rocket).expect("valid rocket instance");
        let response = client.get("/").dispatch();

        assert_eq!(response.status(), Status::Ok);
        assert_eq!(response.into_string(), Some("hello world".into()));
    }

    #[test]
    fn test_elus_endpoint() {
        let mut connection = setup_test_db();
        insert_test_persons(&mut connection);

        let rocket = rocket::build()
            .manage(Mutex::new(connection))
            .mount("/", routes![index, elus, get_person_by_email]);

        let client = Client::tracked(rocket).expect("valid rocket instance");
        let response = client.get("/elus").dispatch();

        assert_eq!(response.status(), Status::Ok);

        let returned_persons: Vec<Person> = response.into_json().expect("valid JSON");
        assert_eq!(returned_persons.len(), 3);
        assert_eq!(returned_persons[0].name, "Jean Dupont");
        assert_eq!(returned_persons[0].email, "jean.dupont@example.com");
        assert_eq!(returned_persons[0].mandates.len(), 2);
        assert_eq!(returned_persons[1].name, "Marie Martin");
        assert_eq!(returned_persons[2].name, "Pierre Durand");
    }

    #[test]
    fn test_get_person_by_email() {
        let mut connection = setup_test_db();
        insert_test_persons(&mut connection);

        let rocket = rocket::build()
            .manage(Mutex::new(connection))
            .mount("/", routes![index, elus, get_person_by_email]);

        let client = Client::tracked(rocket).expect("valid rocket instance");

        // Test finding an existing person
        let response = client.get("/elus/marie.martin@example.com").dispatch();
        assert_eq!(response.status(), Status::Ok);

        let person: Person = response.into_json().expect("valid JSON");
        assert_eq!(person.name, "Marie Martin");
        assert_eq!(person.email, "marie.martin@example.com");
        assert_eq!(person.mandates.len(), 1);
        assert_eq!(person.mandates[0], "Députée");

        // Test with non-existing email
        let response = client.get("/elus/nonexistent@example.com").dispatch();
        assert_eq!(response.status(), Status::NotFound);
    }

    #[test]
    fn test_create_person_new() {
        let connection = setup_test_db();
        let rocket = rocket::build()
            .manage(Mutex::new(connection))
            .mount("/", routes![index, elus, get_person_by_email, create_person_new, create_person_create]);

        let client = Client::tracked(rocket).expect("valid rocket instance");

        let new_person = Person {
            name: "Alice Wonderland".to_string(),
            email: "alice@example.com".to_string(),
            mandates: vec!["Conseillère".to_string()],
        };

        let response = client
            .post("/elus/new")
            .json(&new_person)
            .dispatch();

        assert_eq!(response.status(), Status::Ok);

        let created: Person = response.into_json().expect("valid JSON");
        assert_eq!(created.name, "Alice Wonderland");
        assert_eq!(created.email, "alice@example.com");
        assert_eq!(created.mandates.len(), 1);
        assert_eq!(created.mandates[0], "Conseillère");
    }

    #[test]
    fn test_create_person_create_alias() {
        let connection = setup_test_db();
        let rocket = rocket::build()
            .manage(Mutex::new(connection))
            .mount("/", routes![index, elus, get_person_by_email, create_person_new, create_person_create]);

        let client = Client::tracked(rocket).expect("valid rocket instance");

        let new_person = Person {
            name: "Bob Builder".to_string(),
            email: "bob@example.com".to_string(),
            mandates: vec!["Architecte".to_string(), "Ingénieur".to_string()],
        };

        let response = client
            .post("/elus/create")
            .json(&new_person)
            .dispatch();

        assert_eq!(response.status(), Status::Ok);

        let created: Person = response.into_json().expect("valid JSON");
        assert_eq!(created.name, "Bob Builder");
        assert_eq!(created.email, "bob@example.com");
        assert_eq!(created.mandates.len(), 2);
    }

    #[test]
    fn test_create_person_duplicate_email() {
        let mut connection = setup_test_db();
        insert_test_persons(&mut connection);

        let rocket = rocket::build()
            .manage(Mutex::new(connection))
            .mount("/", routes![index, elus, get_person_by_email, create_person_new, create_person_create]);

        let client = Client::tracked(rocket).expect("valid rocket instance");

        let duplicate_email_person = Person {
            name: "Different Name".to_string(),
            email: "jean.dupont@example.com".to_string(),
            mandates: vec!["Some mandate".to_string()],
        };

        let response = client
            .post("/elus/new")
            .json(&duplicate_email_person)
            .dispatch();

        assert_eq!(response.status(), Status::Conflict);
    }

    #[test]
    fn test_create_person_duplicate_name() {
        let mut connection = setup_test_db();
        insert_test_persons(&mut connection);

        let rocket = rocket::build()
            .manage(Mutex::new(connection))
            .mount("/", routes![index, elus, get_person_by_email, create_person_new, create_person_create]);

        let client = Client::tracked(rocket).expect("valid rocket instance");

        let duplicate_name_person = Person {
            name: "Jean Dupont".to_string(),
            email: "different.email@example.com".to_string(),
            mandates: vec!["Some mandate".to_string()],
        };

        let response = client
            .post("/elus/new")
            .json(&duplicate_name_person)
            .dispatch();

        assert_eq!(response.status(), Status::Conflict);
    }
}
