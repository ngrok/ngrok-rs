// Import the ngrok module
var ngrok = require('.'); 
ngrok.session().then((result) => {
  result.startTunnel().then((r2) => {
    console.log("got r2: " + r2)
    console.log("got r2: " + r2.getId())
  })
})

