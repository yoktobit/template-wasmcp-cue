import "strings"
import "time"
//import "list"

#Tool: {
	name:            string
	description:     string
	inputSchema:     #Schema
	inputSchemaName: inputSchema._$name
	outputSchema:   #Schema
	outputSchemaName: outputSchema._$name
}

#Tools: [N=string]: #Tool & {
	name: N
	...
}

McpTools: {
	for key, val in Tools {
		"\(key)": {
			name:             val.name
			description:      val.description
			inputSchemaName:  val.inputSchemaName
			outputSchemaName: val.outputSchemaName
		}
	}
}

_RustToolLists: {
	specs: [for _, tool in McpTools {
		"ToolSpec { name: \"\(tool.name)\", title: \"\(tool.name)\", description: \"\(tool.description)\", input_schema_name: \"\(tool.inputSchemaName)\", output_schema_name: \"\(tool.outputSchemaName)\" }"
	}]
}

RustToolConstants: """
// Generated from `schema.cue` by `cue export`. Do not edit manually.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy)]
pub struct ToolSpec {
    pub name: &'static str,
    pub title: &'static str,
    pub description: &'static str,
    pub input_schema_name: &'static str,
    pub output_schema_name: &'static str,
}

pub const TOOL_SPECS: &[ToolSpec] = &[
    \(strings.Join(_RustToolLists.specs, ",\n    "))
];
"""

#Schemas: [N=string]: #Schema & {
	_$name: N
	...
}
#Schema: {
	_$name: string
	...
}

Tools: #Tools & {
	"greeter-tool": {
		description:  "greeter-tool"
		inputSchema:  Schemas.PersonalData
		outputSchema: Schemas.Message
	}
}

Schemas: #Schemas & {
	// Die Persönlichen Daten des Antragstellers
	PersonalData: {
		// Der Name
		name: string
		// Der Vorname oder die Vornamen
		forename: string
		// Das Geburtsdatum
		birthdate: time.Time
		wish:      string
	}
	// Die Nachricht, die an den Nutzer ausgegeben wird
	Message: {
		// The Greeting
		NiceMessage:      string
		NotsoNiceMessage: string
	}
}
