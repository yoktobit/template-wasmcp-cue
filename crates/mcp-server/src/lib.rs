mod bindings {
    wit_bindgen::generate!({
        path: "wit",
        world: "acmeapp",
        with: {
            "acme:app/api@0.1.0/personal-data": crate::component_api::Input,
            "acme:app/api@0.1.0/message": crate::component_api::Output,
        },
        generate_all,
    });
}

mod component_api {
    include!(concat!(env!("OUT_DIR"), "/component_api.rs"));
}

#[macro_use]
mod mcp;

mcp_tool! {
    name: "acme_app",
    title: "AcmeApp",
    description: "Some cool tool",
    input_schema: "../component/_input.schema.json",
    output_schema: "../component/_output.schema.json",
    handler: component_api::call,
}
