var UNIX_SOCKET = "/tmp/http.socket";
const fs = require('fs');
fs.unlinkSync(UNIX_SOCKET);

// make webserver
var http = require('http'); 
http.createServer(function (req, res) {
  res.writeHead(200, {'Content-Type': 'text/html'}); 
  res.write('Congrats you have a created an ngrok web server');
  res.end();
})
// .listen(8081); // Server object listens on port 8081
//console.log('Node.js web server at port 8081 is running..');
.listen(UNIX_SOCKET); // Server object listens on unix socket
console.log('Node.js web server at ' + UNIX_SOCKET + ' is running..');

// setup ngrok
var ngrok = require('.');
builder = new ngrok.SessionBuilder();
builder.authtokenFromEnv().metadata("this is so fun")

var global_session; // don't let this get garbage collected
var global_tunnel;
builder.connect().then((session) => {
  global_session = session
  session.tcpEndpoint()
    .metadata("node tunnel")
    //.remoteAddr("<n>.tcp.ngrok.io:<ppppp>")
    .listen().then((tunnel) => {
      global_tunnel = tunnel;
      console.log("established tunnel at: " + tunnel.getUrl())
      // tunnel.forwardHttp("localhost:8081");
      tunnel.forwardUnix(UNIX_SOCKET);
  })
}).await;

setInterval(() => {
    console.log('async loop still running');
}, 5000)
