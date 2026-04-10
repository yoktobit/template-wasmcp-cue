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
			name: val.name
			description: val.description
			inputSchemaName: val.inputSchemaName
			outputSchemaName: val.outputSchemaName
		}
	}
}

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
