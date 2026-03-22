mod bindings {
    wit_bindgen::generate!({
        world: "acmeapp",
        generate_all,
    });
}

#[macro_use]
mod mcp;

fn handle(input: input::PersonalData) -> output::Message {
    output::Message {
        nice_message: format!("Hello, {}", input.name),
    }
}

mcp_tool! {
    name: "acme_app",
    title: "AcmeApp",
    description: "Some cool tool",
    input: input::PersonalData,
    handler: handle,
}
