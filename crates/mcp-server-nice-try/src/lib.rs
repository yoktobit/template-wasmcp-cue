mod bindings {
    wit_bindgen::generate!({
        path: "wit",
        world: "acmeapp",
        with: {
            "acme:app/api@0.1.0/input": crate::component_api::Input,
            "acme:app/api@0.1.0/output": crate::component_api::Output,
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
    input_schema: "../../_input.schema.json",
    output_schema: "../../_output.schema.json",
    handler: component_api::call,
}
