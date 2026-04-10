mod bindings {
    wit_bindgen::generate!({
        path: "wit",
        world: "component",
        generate_all,
        additional_derives: [serde::Serialize, serde::Deserialize],
    });
}

use bindings::exports::acme::app::api::Guest;
pub use bindings::exports::acme::app::api::{Input, Output};

pub fn handle(input: Input) -> Output {
    Output {
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

struct Component;

impl Guest for Component {
    fn run(input: Input) -> Output {
        handle(input)
    }
}

bindings::export!(Component with_types_in bindings);
