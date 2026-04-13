mod bindings {
    wit_bindgen::generate!({
        path: "wit",
        world: "component",
        generate_all,
        additional_derives: [serde::Serialize, serde::Deserialize],
    });
}

use bindings::exports::acme::pet::api::Guest;
use crate::bindings::exports::acme::pet::api::{Message, Pet};

struct Component;

impl Guest for Component {
    fn ask_pet_health(input: Pet) -> Message {
        Message {
            nice_message: format!("How is your {}, {}?", input.pet_type, input.name),
            notso_nice_message: format!(
                "Is your {}, {}, dead already?",
                input.pet_type, input.name
            ),
        }
    }
}

bindings::export!(Component with_types_in bindings);
