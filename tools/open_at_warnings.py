import json
from dict import dict
from os import system

with open("kernel/diagnostics.json") as f:
	lines = list(iter(f))
	warning_count = len(list(filter(lambda k: json.loads(k)["reason"] == "compiler-message", lines)))
	for idx, line in enumerate(lines):
		if not line.strip():
			continue
		k = dict(json.loads(line))
		if k.reason == "compiler-message" and k.message and dict(k.message).spans:
			for i in dict(k.message).spans:
				i = dict(i)
				l = i.line_start
				c = i.column_start
				f = "kernel/" + i.file_name
				cmd = f"nano +{l},{c} {f}"
			print(dict(k.message).message, f"{idx} / {warning_count}")
			input()
			system(cmd)
			