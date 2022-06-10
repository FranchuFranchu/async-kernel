"""
Splits some text to prevent it from exceeding the 80 character limit
"""

import argparse

def main():
	parser = argparse.ArgumentParser()
	
	parser.add_argument("--prefix", "-p", default="//", help="Prefix for each line")
	
	args = parser.parse_args()
	s = input()

	while s:
		split_index = 0
		for index in range(min(len(s) - 1, 60), 0 , -1):
			if s[index] == " ":
				split_index = index
				break
		else:
			split_index = 2 ** 32
			
		slice, s = s[:split_index], s[split_index:]
		if args.prefix.strip():
			print(args.prefix + " " + slice)
		
if __name__ == '__main__':
	main()