// @generated automatically by Diesel CLI.

diesel::table! {
    elus (id) {
        id -> Integer,
        name -> Text,
        email -> Text,
        mandates -> Text,
    }
}
