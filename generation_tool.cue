package main

import "tool/exec"

import "tool/file"

toolNames: [
	for k, v in Tools {
		v.name
	},
]
lastTool: toolNames[len(toolNames)-1]

outputfilename:          "src/tools.rs"
outputToolCallsFilename: "src/tool_calls.rs"

command: {
	extractTools: {
		"generateJsonSchema": exec.Run & {
			cmd: ["cue", "def", "-o", "jsonschema:_allSchemas.schema.json", ".", "-e", "Schemas"]
		}
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
			$after:   extractTools["append_\(lastTool)"]
		}
		outputToolCallsHeader: file.Create & {
			filename: outputToolCallsFilename
			contents: #RustToolCallHeader
		}
		for k, v in Tools {
			"outputToolCallTraitItem_\(v.name)": file.Append & {
				filename: outputToolCallsFilename
				contents: (#RustToolCallTraitTemplate & {
					inputTool: v
				}).outputRust
				$after: outputToolCallsHeader
			}
		}
		outputToolCallTraitInlay: file.Append & {
			filename: outputToolCallsFilename
			contents: #RustToolCallTraitInlay
			$after:   extractTools["outputToolCallTraitItem_\(lastTool)"]
		}
		for k, v in Tools {
			"outputToolCallItem_\(v.name)": file.Append & {
				filename: outputToolCallsFilename
				contents: (#RustToolCallTemplate & {
					inputTool: v
				}).outputRust
				$after: outputToolCallTraitInlay
			}
		}
		outputToolCallFooter: file.Append & {
			filename: outputToolCallsFilename
			contents: #RustToolCallFooter
			$after:   extractTools["outputToolCallItem_\(lastTool)"]
		}
	}
}
