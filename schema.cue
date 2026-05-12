package main

#Tool: {
	name:             string
	title:            string | *name
	description:      string
	inputSchema:      #Schema
	outputSchema:     #Schema
	inputSchemaName:  inputSchema._$name
	outputSchemaName: outputSchema._$name
}

#Tools: [N=string]: #Tool & {
	name: N
}

#RustToolHeader: """
use crate::bindings::wasmcp::mcp_v20251125::mcp::{Tool, ToolOptions};
use once_cell::sync::Lazy;

pub static TOOLS: Lazy<Vec<Tool>> = Lazy::new(|| vec![
"""

#RustToolFooter: """
]);
"""

#ToolToRustTool: {
	inputTool:   #Tool
	inputSchema: string
	outputRust:  string &
		"""
            Tool {
                name: "\(inputTool.name)".to_string(),
                input_schema: r###"\(inputSchema)"###.to_string(),
                options: Some(ToolOptions {
                    meta: None,
                    icons: None,
                    annotations: None,
                    description: Some("\(inputTool.description)".to_string()),
                    output_schema: None,
                    title: Some("\(inputTool.title)".to_string()),
                }),
            },
            """
}

#Schema: {
	_$name: string
	...
}

#Schemas: [N=string]: #Schema & {
	_$name: N
}

Schemas: #Schemas
Tools:   #Tools

Schemas: #Schemas & {
	TestInput: {
		name: string
		pet:  "dog" | "cat"
	}
	TestOutput: {
		message: string
	}
}

Tools: #Tools & {
	"greeter": {
		description:  "Greets somebody"
		inputSchema:  Schemas.TestInput
		outputSchema: Schemas.TestOutput
	}
}
