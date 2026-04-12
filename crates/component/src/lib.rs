mod bindings {
    wit_bindgen::generate!({
        path: "wit",
        world: "component",
        generate_all,
        additional_derives: [serde::Serialize, serde::Deserialize],
    });
}

use bindings::exports::acme::app::api::Guest;
use crate::bindings::exports::acme::app::api::{Message, PersonalData, Pet};

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

    fn ask_pet_health(input: Pet) -> Message {
        Message {
            nice_message: format!(
                "How is your {}, {}?", input.pet_type, input.name
            ),
            notso_nice_message: format!(
                "Is your {}, {}, dead already?", input.pet_type, input.name
            ),
        }
    }
}

bindings::export!(Component with_types_in bindings);
