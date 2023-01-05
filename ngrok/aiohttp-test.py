#!/usr/bin/env python

import asyncio
from aiohttp import web
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
async def hello(request):
  return web.Response(text="Hello, world")

app = web.Application()
app.add_routes([web.get('/', hello)])
print("running app")
web.run_app(app, path=UNIX_SOCKET, loop=loop)
