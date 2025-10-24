#[macro_use] extern crate rocket;

mod schema;

use diesel::prelude::*;
use diesel::sqlite::SqliteConnection;
use rocket::serde::{Serialize, Deserialize, json::Json};
use rocket::State;
use dotenvy::dotenv;
use std::env;
use std::sync::Mutex;

#[derive(Debug, Clone, Serialize, Deserialize, Queryable, Selectable)]
#[diesel(table_name = schema::elus)]
#[serde(crate = "rocket::serde")]
struct Person {
    id: i32,
    name: String,
    email: String,
    mandates: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
struct PersonResponse {
    name: String,
    email: String,
    mandates: Vec<String>,
}

impl From<Person> for PersonResponse {
    fn from(person: Person) -> Self {
        let mandates: Vec<String> = serde_json::from_str(&person.mandates)
            .unwrap_or_else(|_| vec![]);
        PersonResponse {
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
fn elus(db: &State<DbConn>) -> Json<Vec<PersonResponse>> {
    use self::schema::elus::dsl::*;

    let mut connection = db.lock().unwrap();
    let results = elus
        .select(Person::as_select())
        .load(&mut *connection)
        .expect("Error loading persons");

    let responses: Vec<PersonResponse> = results.into_iter()
        .map(PersonResponse::from)
        .collect();

    Json(responses)
}

#[get("/elus/<search_email>")]
fn get_person_by_email(search_email: String, db: &State<DbConn>) -> Option<Json<PersonResponse>> {
    use self::schema::elus::dsl::*;

    let mut connection = db.lock().unwrap();
    let result = elus
        .filter(email.eq(&search_email))
        .select(Person::as_select())
        .first(&mut *connection)
        .ok()?;

    Some(Json(PersonResponse::from(result)))
}

#[launch]
fn rocket() -> _ {
    let connection = establish_connection();
    rocket::build()
        .manage(Mutex::new(connection))
        .mount("/", routes![index, elus, get_person_by_email])
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

        let returned_persons: Vec<PersonResponse> = response.into_json().expect("valid JSON");
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

        let person: PersonResponse = response.into_json().expect("valid JSON");
        assert_eq!(person.name, "Marie Martin");
        assert_eq!(person.email, "marie.martin@example.com");
        assert_eq!(person.mandates.len(), 1);
        assert_eq!(person.mandates[0], "Députée");

        // Test with non-existing email
        let response = client.get("/elus/nonexistent@example.com").dispatch();
        assert_eq!(response.status(), Status::NotFound);
    }
}
