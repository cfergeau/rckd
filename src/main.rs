#[macro_use] extern crate rocket;

use rocket::serde::{Serialize, Deserialize, json::Json};
use rocket::State;
use std::sync::Mutex;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
struct Person {
    name: String,
    email: String,
    mandates: Vec<String>,
}

type PersonList = Mutex<Vec<Person>>;

#[get("/")]
fn index() -> &'static str {
    "hello world"
}

#[get("/elus")]
fn elus(persons: &State<PersonList>) -> Json<Vec<Person>> {
    let persons = persons.lock().unwrap();
    Json(persons.clone())
}

#[get("/elus/<email>")]
fn get_person_by_email(email: String, persons: &State<PersonList>) -> Option<Json<Person>> {
    let persons = persons.lock().unwrap();
    persons.iter()
        .find(|p| p.email == email)
        .cloned()
        .map(Json)
}

#[launch]
fn rocket() -> _ {
    rocket::build()
        .manage(Mutex::new(Vec::<Person>::new()))
        .mount("/", routes![index, elus, get_person_by_email])
}

#[cfg(test)]
mod tests {
    use super::{rocket, Person};
    use rocket::local::blocking::Client;
    use rocket::http::Status;
    use std::sync::Mutex;

    #[test]
    fn test_hello_world() {
        let client = Client::tracked(rocket()).expect("valid rocket instance");
        let response = client.get("/").dispatch();

        assert_eq!(response.status(), Status::Ok);
        assert_eq!(response.into_string(), Some("hello world".into()));
    }

    #[test]
    fn test_elus_endpoint() {
        // Create test persons
        let mut persons = Vec::new();
        persons.push(Person {
            name: "Jean Dupont".to_string(),
            email: "jean.dupont@example.com".to_string(),
            mandates: vec!["Maire".to_string(), "Conseiller régional".to_string()],
        });
        persons.push(Person {
            name: "Marie Martin".to_string(),
            email: "marie.martin@example.com".to_string(),
            mandates: vec!["Députée".to_string()],
        });
        persons.push(Person {
            name: "Pierre Durand".to_string(),
            email: "pierre.durand@example.com".to_string(),
            mandates: vec!["Sénateur".to_string(), "Conseiller municipal".to_string()],
        });

        // Create rocket instance with test data
        let rocket = rocket::build()
            .manage(Mutex::new(persons.clone()))
            .mount("/", routes![super::index, super::elus]);

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
        // Create test persons
        let mut persons = Vec::new();
        persons.push(Person {
            name: "Jean Dupont".to_string(),
            email: "jean.dupont@example.com".to_string(),
            mandates: vec!["Maire".to_string(), "Conseiller régional".to_string()],
        });
        persons.push(Person {
            name: "Marie Martin".to_string(),
            email: "marie.martin@example.com".to_string(),
            mandates: vec!["Députée".to_string()],
        });

        // Create rocket instance with test data
        let rocket = rocket::build()
            .manage(Mutex::new(persons.clone()))
            .mount("/", routes![super::index, super::elus, super::get_person_by_email]);

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
}
