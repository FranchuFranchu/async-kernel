import json

with open("tools/diagnostics.json") as f:
	k = json.load(f)
	print(k)