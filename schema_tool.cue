package main

import "tool/exec"

import "tool/file"

toolNames: [
	for k, v in Tools {
		v.name
	}
]

outputfilename: "src/tools.rs"

command: {
	extractTools: {
		"aFirstStep": file.Create & {
			filename: outputfilename
			contents: #RustToolHeader
		}
		for k, v in Tools {
			"extractTool_\(v.name)": exec.Run & {
				cmd: ["cue", "def", "-o", "jsonschema:-", ".", "-e", "Schemas.\(v.inputSchemaName)"]
				stdout: string
			}
			"append_\(v.name)": file.Append & {
				filename: outputfilename
				contents: (#ToolToRustTool & {
					inputTool:   v
					inputSchema: extractTools["extractTool_\(v.name)"].stdout
				}).outputRust
			}
		}
		"zeLastStep": file.Append & {
			filename: outputfilename
			contents: #RustToolFooter
			lastTool: toolNames[len(toolNames)-1]
			$after: extractTools["append_\(lastTool)"]
		}
	}
}
