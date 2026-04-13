mod bindings {
    wit_bindgen::generate!({
        path: "wit",
        world: "component",
        generate_all,
        additional_derives: [serde::Serialize, serde::Deserialize],
    });
}

use bindings::exports::acme::greeter::api::Guest;
use crate::bindings::exports::acme::greeter::api::{Message, PersonalData};

struct Component;

impl Guest for Component {
    fn greeter_tool(input: PersonalData) -> Message {
        Message {
            nice_message: format!(
                "Hello, {} {}. Best wishes for '{}' on {}!",
                input.forename, input.name, input.wish, input.birthdate
            ),
            notso_nice_message: format!(
                "If '{}' takes longer than expected, you'll still have to do some work yourself, {}.",
                input.wish, input.name
            ),
        }
    }
}

bindings::export!(Component with_types_in bindings);
