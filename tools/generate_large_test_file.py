# Fibonacci to a file
with open("drive-loopback/fibonacci.txt", "w") as f:
	a = 1
	b = 1
	c = 0
	for _ in range(0, 1000):
		if c != 0:
			a = b
			b = c
		c = a + b
		f.write(str(a) + "\n")