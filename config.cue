package main

#TestInput: {
	// The name of the person to greet
	name: string
	// The type of pet of the person
	pet: "dog" | "cat"
}

#TestOutput: {
	message: string
}

Schemas: {
	TestInput:  #TestInput
	TestOutput: #TestOutput
}

Tools: #Tools & {
	"greeter": {
		description:      "Greets somebody and asks how his pet is."
		inputSchemaName:  SchemaNames.TestInput
		outputSchemaName: SchemaNames.TestOutput
	}
}
