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
	names:             [for _, tool in McpTools { "\"\(tool.name)\"" }]
	titles:            [for _, tool in McpTools { "\"\(tool.name)\"" }]
	descriptions:      [for _, tool in McpTools { "\"\(tool.description)\"" }]
	inputSchemaNames:  [for _, tool in McpTools { "\"\(tool.inputSchemaName)\"" }]
	outputSchemaNames: [for _, tool in McpTools { "\"\(tool.outputSchemaName)\"" }]
}

RustToolConstants: """
// Generated from `schema.cue` by `cue export`. Do not edit manually.
pub const TOOL_NAMES: &[&str] = &[\(strings.Join(_RustToolLists.names, ", "))];
pub const TOOL_TITLES: &[&str] = &[\(strings.Join(_RustToolLists.titles, ", "))];
pub const TOOL_DESCRIPTIONS: &[&str] = &[\(strings.Join(_RustToolLists.descriptions, ", "))];
pub const INPUT_SCHEMA_NAMES: &[&str] = &[\(strings.Join(_RustToolLists.inputSchemaNames, ", "))];
pub const OUTPUT_SCHEMA_NAMES: &[&str] = &[\(strings.Join(_RustToolLists.outputSchemaNames, ", "))];
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
