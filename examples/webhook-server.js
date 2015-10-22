'use strict';

const http = require('http');
const https = require('https');
const querystring = require('querystring');
const urilib = require('url');
const Buffer = require('buffer').Buffer;

const TASK_MAP = new Map();

function processMessage(buffer) {
  let message;
  try {
    message = JSON.parse(buffer);
  } catch (e) {
    return console.log("could not parse message");
  }

  const fullyQualifiedBranch = `${message.owner}.${message.repo}.${message.branch}`;
  const shortJobId = message.task_id.slice(0, 6);
  const prelude = `[${fullyQualifiedBranch}] <${shortJobId}>`;
  const messageMap = new Map();
  messageMap.set('started', `Starting build...`);
  messageMap.set('success', `Success!`);
  messageMap.set('failed', `Failed, see job details page: ${message.job_url}`);

  const titleMap = new Map();
  titleMap.set('started', '📦 Hookshot Received 📦');
  titleMap.set('success', '🎊 Hookshot Complete 🎊');
  titleMap.set('failed', '🚨 Hookshot Failed 🚨');

  const colorMap = new Map();
  colorMap.set('started', '#187ac0');
  colorMap.set('success', 'good');
  colorMap.set('failed', 'danger');

  const status = message.status.toLowerCase();
  const url = process.env.SLACK_URL;
  const fields = [
    {
      short: true,
      title: 'Job ID',
      value: `<${message.job_url}|${message.task_id}>`,
    },
    {
      short: true,
      title: 'Repository',
      value: fullyQualifiedBranch,
    },
  ];

  if (status === 'started') {
    let startTime = new Date();
    TASK_MAP.set(message.task_id, {startTime});
    fields.push({
      short: true,
      title: 'Started',
      value: `${startTime.toLocaleString()}`,
    });
  } else {
    let endTime = new Date();
    let startTime = TASK_MAP.get(message.task_id).startTime;
    fields.push({
      short: true,
      title: 'Duration',
      value: `${(endTime-startTime)/1000|0} seconds`
    });
    TASK_MAP.delete(message.task_id);
  }

  const payload = {
    channel: process.env.SLACK_CHANNEL || '#botplayground',
    username: 'hookshotbot',
    icon_emoji: ':shipit:',
    attachments: [{
      fallback: `${prelude} ${messageMap.get(status)}`,
      color: colorMap.get(status),
      title: titleMap.get(status),
      text: `${messageMap.get(status)}`,
      fields:  fields,
    }],
  };

  const postData = querystring.stringify({
    payload: JSON.stringify(payload),
  });

  const request = https.request(Object.assign(urilib.parse(url), {
    method: 'POST',
    headers: {
      'Content-Type': 'application/x-www-form-urlencoded',
      'Content-Length': postData.length,
    },
  }), (response) => {
    console.log('STATUS: ' + response.statusCode);
    console.log('HEADERS: ' + JSON.stringify(response.headers));
    response.setEncoding('utf8');
    response.pipe(process.stdout, {end: false});
  });
  request.write(postData);
  request.end();

}

const server = http.createServer((req, res) => {
  process.stdout.write(JSON.stringify(req.headers));
  process.stdout.write('\n');

  let buffer = '';
  req.on('data', (incoming) => buffer += incoming);
  req.once('end', () => {
    console.log(buffer);
    processMessage(buffer);
  });

  res.end('done');
});

server.listen(5600, () => console.error('listening on %s', server.address().port));
