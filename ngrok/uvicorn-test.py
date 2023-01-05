#!/usr/bin/env python

import asyncio
from uvicorn import Config, Server
import ngrok
import os

UNIX_SOCKET = "/tmp/http.socket"
if os.path.exists(UNIX_SOCKET):
    os.remove(UNIX_SOCKET)

# start tunnel
async def create_tunnel():
  session = await ngrok.connect()
  tunnel = await session.start_tunnel()
  print("tunnel: {}".format(tunnel))
  await tunnel.forward_unix(UNIX_SOCKET)

loop = asyncio.new_event_loop()
loop.create_task(create_tunnel())

# start web server
async def app(scope, receive, send):
  assert scope['type'] == 'http'

  await send({
    'type': 'http.response.start',
    'status': 200,
    'headers': [
      [b'content-type', b'text/plain'],
    ],
  })
  await send({
    'type': 'http.response.body',
    'body': b'Hello, world!',
  })

print("running app")
# cannot pass in our loop like aiohttp lets us :(
server = Server(Config(app=app, uds=UNIX_SOCKET, loop="none"))
loop.run_until_complete(server.serve())
