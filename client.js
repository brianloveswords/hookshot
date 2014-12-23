const net = require('net');

const client = net.connect({
  port: 1469,
  host: process.argv[2]
}, function (conn) {
  client.end(JSON.stringify({
    secret: process.argv[3],
    ansible: {
      hostname: process.argv[4],
      version: process.argv[5],
    }}));
})

client.setEncoding('utf8');
client.pipe(process.stderr, {end: false});
