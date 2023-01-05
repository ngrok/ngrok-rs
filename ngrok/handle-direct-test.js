// NODE_DEBUG=http,net,stream node ./handle-test.js
var http = require('http'); 

// start server
var server = http.createServer(function (req, res) {
  res.writeHead(200, {'Content-Type': 'text/html'}); 
  res.write('Congrats you have a created an ngrok web server');
  res.end();
});

// look at me. i am the listener now.
console.log("listenercount: " + server.listenerCount('connection'));
oldListeners = server.listeners('connection');
server.removeAllListeners('connection');
server.addListener('bobconnection', oldListeners[0]);
server.on('connection', function(sock) {
  console.log("---- connection");
  // investigate live objects
  console.log("- intercept connection. sock: " + sock);
  console.log(Object.getOwnPropertyNames(sock));
  console.log("sock methods: " + getMethods(sock));
  
  let sh = sock._handle;
  console.log("- intercept connection. handle: " + sh);
  console.log(Object.getOwnPropertyNames(sh));
  console.log("handle methods: " + getMethods(sh));

  // FROM TUNNEL TO WEBSERVER:
  // simulate a read from the handle to the stream in socket
  let buf = Buffer.from("GET / HTTP/1.0\n\n");
  console.log("<=== pushing to webserver: " + buf);
  sock.push(buf);

  // push an event to run the regular connection
  server.emit('bobconnection', sock);
});

// start the webserver listening
server.listen(2023);
console.log('Node.js web server is running..');

// get the handle from the listening socket the server has now 
let webHandle = server._handle;
console.log("listen webHandle: " + webHandle);

// call onconnection with our handle
setTimeout(function(){onconn(webHandle)}, 500);

// on connection
function onconn(handle) {
  // set up client handle
  const clientHandle = {}
  clientHandle.readStart = function() {
    console.log("clientHandle: readstart");
    // wanted this to work, but the number of bytes read (nread) is
    // set within c code, and doesn't appear to be settable from
    // javascript. so we can't just make our own handle. a workaround
    // for this is above with the replacement of 'connection' handler
    // doing a direct 'push' on the socket's stream.
    //
    // CallJSOnreadMethod in stream_base.cc sets kReadBytesOrError. That's called by OnStreamRead,
    //   which is called by EmitRead, which is all internal to c code.
    // onStreamRead in stream_base_commons.js checks kReadBytesOrError > 0.
    // The socket's onread points to that onStreamRead in net.js.
    //
    // https://github.com/nodejs/node/blob/34af1f69b9de15aa58a5e4ce4e790b4500bc0c8d/src/stream_base.cc#L349
    // https://github.com/nodejs/node/blob/34af1f69b9de15aa58a5e4ce4e790b4500bc0c8d/lib/internal/stream_base_commons.js#L167
    // https://github.com/nodejs/node/blob/34af1f69b9de15aa58a5e4ce4e790b4500bc0c8d/lib/net.js#L294
    //
    // this.onread(Buffer.from("GET / HTTP/1.0\n\n"));
  }
  clientHandle.writev = function(req, chunks, allBuffers) {
    console.log("clientHandle: writev");
    // FROM WEBSERVER TO TUNNEL:
    // this would be where we read out the response to send back to the tunnel
    console.log("===> webserver data chunks: " + chunks);
    return 0; // return that there was no error
  }

  console.log("clientHandle: calling onconnection");
  handle.onconnection(undefined, clientHandle)
}

// liveness check
setInterval(() => {
    console.log('async loop still running');
}, 5000)

// trace the inheritance for all methods
const getMethods = (obj) => {
  let properties = new Set()
  let currentObj = obj
  do {
    Object.getOwnPropertyNames(currentObj).map(item => properties.add(item))
  } while ((currentObj = Object.getPrototypeOf(currentObj)))
  return [...properties.keys()].filter(item => typeof obj[item] === 'function')
}

// this is here to shutdown gracefully when doing tracing:
// NODE_DEBUG=http,net,stream node --trace-events-enabled ./handle-test.js
process.on('SIGINT', function onSigint() {
  console.info('Received SIGINT.');
  process.exit(130);  // Or applicable exit code depending on OS and signal
});
