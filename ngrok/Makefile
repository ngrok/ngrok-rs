all: install run

install:
	yarn install

build:
	yarn build

run: build
	node ./ngrok-webserver-test.js

clean:
	rm -rf ngrok.*.node .yarn
