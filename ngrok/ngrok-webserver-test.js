var ngrok = require('.');
forward_future = ngrok.session().then((session) => {
  session.startTunnel().then((tunnel) => {
    console.log("established tunnel at: " + tunnel.getUrl())
    tunnel.forwardHttp("localhost:8081");
  })
})
console.log("forward_future: " + forward_future)

// Import the Node.js http module
var http = require('http'); 
  
// req is the request object which is
// coming from the client side
// res is the response object which is going
// to client as response from the server
  
// Create a server object
http.createServer(function (req, res) {
  
// 200 is the status code which means
// All OK and the second argument is
// the object of response header.
res.writeHead(200, {'Content-Type': 'text/html'}); 
  
    // Write a response to the client
    res.write('Congrats you have a created a web server');
  
    // End the response
    res.end();
  
}).listen(8081); // Server object listens on port 8081
  
console.log('Node.js web server at port 8081 is running..');
forward_future.await;
