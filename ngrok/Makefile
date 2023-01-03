all: install run

install: venv
	: # Activate venv
	. .env/bin/activate && pip install -r requirements.txt

venv:
	: # Create venv if it doesn't exist
	test -d venv || python3 -m venv .env

build:
	. .env/bin/activate && maturin develop

run: build
	. .env/bin/activate && ./ngrok-test.py

clean:
	rm -rf .env
