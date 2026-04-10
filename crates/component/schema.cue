import "time"

// Die Persönlichen Daten des Antragstellers
#PersonalData: {
	// Der Name
	name:      string
	// Der Vorname oder die Vornamen
	forename:  string
	// Das Geburtsdatum
	birthdate: time.Time
    wish: string
}

// Die Nachricht, die an den Nutzer ausgegeben wird
#Message: {
	// The Greeting
	NiceMessage: string
    NotsoNiceMessage: string
}

// Die Eingabewerte für MCP
Input: {} & #PersonalData

// Die Ausgabewerte für MCP
Output: #Message
