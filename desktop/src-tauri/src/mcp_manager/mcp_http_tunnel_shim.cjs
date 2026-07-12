'use strict';
// csp-http-tunnel-shim: fix HTTPS-through-HTTP-proxy for Node MCP connectors
// running inside Claude Science's sandboxed MCP child process.
//
// Why this exists: Science injects an `HTTPS_PROXY`/`https_proxy` pointing at
// its own local "Operon" proxy into every MCP child, and the sandbox's
// network policy only allows that child to reach that one loopback address —
// any other local port (including a proxy CSP might run itself) is denied
// with EPERM at the OS level. Operon's proxy *does* support a real CONNECT
// tunnel (confirmed: `curl -x http://127.0.0.1:<port> https://...` works).
//
// The problem is client-side: many bundled HTTP clients (axios via
// `follow-redirects`/`proxy-from-env`, as used by e.g. `@notionhq/notion-mcp-
// server`) never issue a CONNECT for HTTPS targets when a proxy env var is
// set. They instead relay the request in absolute-form
// (`GET https://host/path HTTP/1.1`) as if it were a plain HTTP forward
// proxy. Operon (like most proxies that expect CONNECT for HTTPS) then
// blindly forwards that as plain-text HTTP to the origin's port 443, and the
// origin (fronted by Cloudflare for many APIs, including Notion's) replies
// `400 The plain HTTP request was sent to HTTPS port`.
//
// This shim monkey-patches `http.request`/`http.get` to detect exactly that
// absolute-form-through-proxy pattern and, instead of letting it go out as
// broken plain HTTP, performs a real CONNECT + TLS handshake to the *same*
// proxy address the client already resolved from its env (so it never tries
// to reach any other loopback port — no new sandbox permission needed) and
// replays the request over that tunnel. Non-matching requests are untouched.
//
// Loaded via `NODE_OPTIONS="--require <this file>"`; safe to load into any
// Node process (including ones that never hit this pattern) since it is a
// no-op unless a request exactly matches the broken absolute-form shape.
const http = require('http');
const https = require('https');
const tls = require('tls');
const net = require('net');
const { URL } = require('url');

const origRequest = http.request;
const origGet = http.get;

function isAbsoluteFormThroughProxy(options) {
  return (
    options &&
    typeof options === 'object' &&
    typeof options.path === 'string' &&
    /^https:\/\//i.test(options.path) &&
    (options.protocol === 'http:' || options.protocol == null)
  );
}

class TunnelAgent extends https.Agent {
  constructor(proxyHost, proxyPort, opts) {
    super(opts);
    this.proxyHost = proxyHost;
    this.proxyPort = proxyPort;
  }

  createConnection(options, callback) {
    const targetHost = options.host;
    const targetPort = options.port || 443;
    const sock = net.connect(this.proxyPort, this.proxyHost);
    const onConnectError = (err) => {
      sock.destroy();
      callback(err);
    };
    sock.once('error', onConnectError);
    sock.once('connect', () => {
      sock.write(
        `CONNECT ${targetHost}:${targetPort} HTTP/1.1\r\n` +
          `Host: ${targetHost}:${targetPort}\r\n\r\n`,
      );
    });
    let buf = '';
    const onData = (chunk) => {
      buf += chunk.toString('latin1');
      const end = buf.indexOf('\r\n\r\n');
      if (end === -1) return;
      sock.removeListener('data', onData);
      sock.removeListener('error', onConnectError);
      const statusLine = buf.slice(0, buf.indexOf('\r\n'));
      if (!/\s200\s/.test(statusLine)) {
        sock.destroy();
        callback(new Error(`csp-http-tunnel-shim: CONNECT failed: ${statusLine}`));
        return;
      }
      const tlsSocket = tls.connect({
        socket: sock,
        servername: options.servername || targetHost,
        host: targetHost,
      });
      tlsSocket.once('secureConnect', () => callback(null, tlsSocket));
      tlsSocket.once('error', (e) => callback(e));
    };
    sock.on('data', onData);
  }
}

function patchedRequest(optionsOrUrl, ...rest) {
  if (typeof optionsOrUrl !== 'object' || optionsOrUrl instanceof URL) {
    return origRequest.call(http, optionsOrUrl, ...rest);
  }
  const options = optionsOrUrl;
  if (!isAbsoluteFormThroughProxy(options)) {
    return origRequest.call(http, options, ...rest);
  }

  const target = new URL(options.path);
  const proxyHost = options.hostname || options.host;
  const proxyPort = options.port || 80;

  const headers = Object.assign({}, options.headers);
  for (const k of Object.keys(headers)) {
    if (k.toLowerCase() === 'host') delete headers[k];
  }
  headers.Host = target.host;

  const patched = Object.assign({}, options, {
    protocol: 'https:',
    hostname: target.hostname,
    host: target.hostname,
    port: target.port || 443,
    path: target.pathname + (target.search || ''),
    headers,
    agent: new TunnelAgent(proxyHost, proxyPort),
  });
  return https.request.call(https, patched, ...rest);
}

http.request = patchedRequest;
http.get = function patchedGet(optionsOrUrl, ...rest) {
  const req = patchedRequest(optionsOrUrl, ...rest);
  req.end();
  return req;
};
// Keep a reference so this module is inert if required twice.
http.get.__cspShimOrig = origGet;
console.error('[csp-http-tunnel-shim] loaded');
