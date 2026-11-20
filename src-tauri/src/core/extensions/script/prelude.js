// Script-extension API (EXT-02), evaluated before index.js in a fresh QuickJS context.
// Rust provides `__host` (http, cookies, log, digests, randomness) and `__unisonInit`
// (settings, storage, credentials as JSON). Everything an extension sees is defined here.
// Unison gives extensions no file system, ffmpeg or download access.
(function (g) {
  "use strict";
  const H = g.__host;
  const init = JSON.parse(g.__unisonInit || "{}");
  delete g.__host;
  delete g.__unisonInit;
  const state = { storage: init.storage || {}, credentials: init.credentials || {} };
  const settings = init.settings || {};
  g.window = g;
  g.self = g;
  g.global = g;

  const own = (o, k) => Object.prototype.hasOwnProperty.call(o, k);
  const clone = (v) => (v === undefined ? null : JSON.parse(JSON.stringify(v)));
  const unsupported = (name) => ({ success: false, error: name + " is not available in Unison" });

  // ---- Bytes: "binary strings" (one char per byte) cross the Rust boundary. ----
  const utf8 = (s) => unescape(encodeURIComponent(String(s)));
  const fromUtf8 = (b) => {
    try {
      return decodeURIComponent(escape(b));
    } catch (e) {
      return b;
    }
  };
  const toBin = (x) => {
    if (typeof x === "string") return utf8(x);
    if (x instanceof ArrayBuffer) x = new Uint8Array(x);
    else if (x && ArrayBuffer.isView(x)) x = new Uint8Array(x.buffer, x.byteOffset, x.byteLength);
    if (x && typeof x.length === "number") {
      let s = "";
      for (let i = 0; i < x.length; i++) s += String.fromCharCode(x[i] & 255);
      return s;
    }
    return utf8(String(x));
  };
  const binToBytes = (b) => {
    const a = new Array(b.length);
    for (let i = 0; i < b.length; i++) a[i] = b.charCodeAt(i) & 255;
    return a;
  };
  const hex = (b) => {
    let s = "";
    for (let i = 0; i < b.length; i++) s += (b.charCodeAt(i) & 255).toString(16).padStart(2, "0");
    return s;
  };

  const B64 = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
  g.btoa = function (s) {
    s = String(s);
    let out = "";
    for (let i = 0; i < s.length; i += 3) {
      const a = s.charCodeAt(i), b = s.charCodeAt(i + 1), c = s.charCodeAt(i + 2);
      if (a > 255 || b > 255 || c > 255) throw new Error("btoa: character out of range");
      const n = (a << 16) | ((b || 0) << 8) | (c || 0);
      out += B64[(n >> 18) & 63] + B64[(n >> 12) & 63];
      out += i + 1 < s.length ? B64[(n >> 6) & 63] : "=";
      out += i + 2 < s.length ? B64[n & 63] : "=";
    }
    return out;
  };
  g.atob = function (s) {
    s = String(s).replace(/[\s=]/g, "").replace(/-/g, "+").replace(/_/g, "/");
    let out = "", buf = 0, bits = 0;
    for (let i = 0; i < s.length; i++) {
      const v = B64.indexOf(s[i]);
      if (v < 0) throw new Error("atob: invalid character");
      buf = ((buf << 6) | v) & 0xffffff;
      bits += 6;
      if (bits >= 8) {
        bits -= 8;
        out += String.fromCharCode((buf >> bits) & 255);
      }
    }
    return out;
  };

  g.TextEncoder = class TextEncoder {
    get encoding() {
      return "utf-8";
    }
    encode(s) {
      return Uint8Array.from(binToBytes(utf8(s === undefined ? "" : s)));
    }
  };
  g.TextDecoder = class TextDecoder {
    constructor() {
      this.encoding = "utf-8";
    }
    decode(b) {
      return b === undefined ? "" : fromUtf8(toBin(b));
    }
  };

  // ---- URL / URLSearchParams ----
  const formEnc = (s) =>
    encodeURIComponent(s)
      .replace(/%20/g, "+")
      .replace(/[!'()~]/g, (c) => "%" + c.charCodeAt(0).toString(16).toUpperCase());
  const formDec = (s) => {
    try {
      return decodeURIComponent(s.replace(/\+/g, " "));
    } catch (e) {
      return s;
    }
  };
  class URLSearchParams {
    constructor(init) {
      this._l = [];
      this._u = null;
      if (init instanceof URLSearchParams) this._l = init._l.map((p) => p.slice());
      else if (typeof init === "string") this._parse(init);
      else if (init && typeof init[Symbol.iterator] === "function")
        for (const [k, v] of init) this._l.push([String(k), String(v)]);
      else if (init && typeof init === "object") for (const k of Object.keys(init)) this._l.push([k, String(init[k])]);
    }
    _parse(s) {
      this._l = [];
      s = String(s).replace(/^\?/, "");
      for (const part of s.split("&")) {
        if (!part) continue;
        const i = part.indexOf("=");
        this._l.push(i < 0 ? [formDec(part), ""] : [formDec(part.slice(0, i)), formDec(part.slice(i + 1))]);
      }
    }
    _ch() {
      if (this._u) this._u._search = this._l.length ? "?" + this.toString() : "";
    }
    append(k, v) {
      this._l.push([String(k), String(v)]);
      this._ch();
    }
    delete(k) {
      this._l = this._l.filter((p) => p[0] !== String(k));
      this._ch();
    }
    get(k) {
      const p = this._l.find((q) => q[0] === String(k));
      return p ? p[1] : null;
    }
    getAll(k) {
      return this._l.filter((p) => p[0] === String(k)).map((p) => p[1]);
    }
    has(k) {
      return this._l.some((p) => p[0] === String(k));
    }
    set(k, v) {
      k = String(k);
      const i = this._l.findIndex((p) => p[0] === k);
      if (i < 0) this._l.push([k, String(v)]);
      else {
        this._l[i][1] = String(v);
        this._l = this._l.filter((p, j) => j <= i || p[0] !== k);
      }
      this._ch();
    }
    sort() {
      this._l.sort((a, b) => (a[0] < b[0] ? -1 : a[0] > b[0] ? 1 : 0));
      this._ch();
    }
    forEach(f, t) {
      for (const [k, v] of this._l) f.call(t, v, k, this);
    }
    keys() {
      return this._l.map((p) => p[0])[Symbol.iterator]();
    }
    values() {
      return this._l.map((p) => p[1])[Symbol.iterator]();
    }
    entries() {
      return this._l.map((p) => p.slice())[Symbol.iterator]();
    }
    [Symbol.iterator]() {
      return this.entries();
    }
    get size() {
      return this._l.length;
    }
    toString() {
      return this._l.map((p) => formEnc(p[0]) + "=" + formEnc(p[1])).join("&");
    }
  }
  g.URLSearchParams = URLSearchParams;

  const URL_RE =
    /^([a-zA-Z][a-zA-Z0-9+.-]*:)(?:\/\/(?:([^:@\/?#]*)(?::([^@\/?#]*))?@)?([^:\/?#]*)(?::(\d*))?)?([^?#]*)(\?[^#]*)?(#.*)?$/;
  const dots = (path) => {
    const segs = path.split("/");
    const out = [];
    segs.forEach((s, i) => {
      const last = i === segs.length - 1;
      if (s === ".") {
        if (last) out.push("");
      } else if (s === "..") {
        if (out.length > 1) out.pop();
        if (last) out.push("");
      } else out.push(s);
    });
    const r = out.join("/");
    return r.startsWith("/") ? r : "/" + r;
  };
  class URL {
    constructor(url, base) {
      url = String(url).trim();
      let m = URL_RE.exec(url);
      if (!m) {
        if (base === undefined) throw new TypeError("Invalid URL: " + url);
        const b = base instanceof URL ? base : new URL(base);
        const noHash = b.href.replace(/#.*$/, "");
        let abs;
        if (url.startsWith("//")) abs = b.protocol + url;
        else if (url.startsWith("/")) abs = b.origin + url;
        else if (url.startsWith("?")) abs = b.origin + b.pathname + url;
        else if (url.startsWith("#")) abs = noHash + url;
        else if (url === "") abs = noHash;
        else abs = b.origin + b.pathname.replace(/[^/]*$/, "") + url;
        m = URL_RE.exec(abs);
        if (!m) throw new TypeError("Invalid URL: " + url);
      }
      this._protocol = m[1].toLowerCase();
      this.username = m[2] || "";
      this.password = m[3] || "";
      this._auth = m[4] !== undefined;
      this._hostname = (m[4] || "").toLowerCase();
      this._port = m[5] || "";
      this._pathname = (this._auth ? dots(m[6] || "/") : m[6] || "").replace(/ /g, "%20");
      this._search = m[7] && m[7] !== "?" ? m[7].replace(/ /g, "%20") : "";
      this.hash = m[8] && m[8] !== "#" ? m[8] : "";
      this._sp = new URLSearchParams(this._search);
      this._sp._u = this;
      this._normPort();
    }
    _normPort() {
      const d = { "http:": "80", "https:": "443", "ws:": "80", "wss:": "443" }[this._protocol];
      if (d && this._port === d) this._port = "";
    }
    get protocol() {
      return this._protocol;
    }
    set protocol(v) {
      this._protocol = String(v).replace(/:?$/, ":").toLowerCase();
      this._normPort();
    }
    get hostname() {
      return this._hostname;
    }
    set hostname(v) {
      this._hostname = String(v).toLowerCase();
    }
    get port() {
      return this._port;
    }
    set port(v) {
      this._port = String(v);
      this._normPort();
    }
    get host() {
      return this._hostname + (this._port ? ":" + this._port : "");
    }
    set host(v) {
      const [h, p] = String(v).split(":");
      this._hostname = h.toLowerCase();
      this._port = p || "";
      this._normPort();
    }
    get origin() {
      return this._auth ? this._protocol + "//" + this.host : "null";
    }
    get pathname() {
      return this._pathname;
    }
    set pathname(v) {
      v = String(v);
      this._pathname = this._auth ? dots(v.startsWith("/") ? v : "/" + v) : v;
    }
    get search() {
      return this._search;
    }
    set search(v) {
      v = String(v);
      this._search = v && v !== "?" ? (v.startsWith("?") ? v : "?" + v) : "";
      this._sp._parse(this._search);
    }
    get searchParams() {
      return this._sp;
    }
    get href() {
      const auth = this.username ? this.username + (this.password ? ":" + this.password : "") + "@" : "";
      return this._protocol + (this._auth ? "//" + auth + this.host : "") + this._pathname + this._search + this.hash;
    }
    set href(v) {
      const u = new URL(v);
      Object.assign(this, u);
      this._sp._u = this;
    }
    toString() {
      return this.href;
    }
    toJSON() {
      return this.href;
    }
  }
  g.URL = URL;

  // ---- HTTP (synchronous, allowlisted in Rust) ----
  const hasHeader = (h, name) => Object.keys(h || {}).some((k) => k.toLowerCase() === name);
  const plainHeaders = (h) => {
    if (!h) return {};
    if (Array.isArray(h)) return Object.fromEntries(h);
    if (typeof h.forEach === "function" && typeof h.get === "function") {
      const out = {};
      h.forEach((v, k) => (out[k] = v));
      return out;
    }
    return Object.assign({}, h);
  };
  function request(method, url, headers, body) {
    headers = plainHeaders(headers);
    if (body !== undefined && body !== null && typeof body !== "string") {
      if (body instanceof URLSearchParams) {
        if (!hasHeader(headers, "content-type"))
          headers["Content-Type"] = "application/x-www-form-urlencoded;charset=UTF-8";
        body = body.toString();
      } else if (body instanceof ArrayBuffer || ArrayBuffer.isView(body)) body = fromUtf8(toBin(body));
      else body = JSON.stringify(body);
    }
    const hasBody = body !== undefined && body !== null;
    if (hasBody && !hasHeader(headers, "content-type")) headers["Content-Type"] = "application/json";
    for (const k of Object.keys(headers)) headers[k] = String(headers[k]);
    const out = JSON.parse(
      H.http(
        JSON.stringify({
          method: String(method || "GET").toUpperCase(),
          url: String(url instanceof URL ? url.href : url),
          headers,
          body: hasBody ? String(body) : null,
        }),
      ),
    );
    if (out.throw) throw new Error(out.throw);
    out.status = out.statusCode;
    return out;
  }
  g.http = {
    get: (u, h) => request("GET", u, h),
    post: (u, b, h) => request("POST", u, h, b),
    put: (u, b, h) => request("PUT", u, h, b),
    patch: (u, b, h) => request("PATCH", u, h, b),
    delete: (u, h) => request("DELETE", u, h),
    request: (u, o) => {
      o = o || {};
      return request(o.method || "GET", u, o.headers, o.body);
    },
    clearCookies: () => {
      H.clearCookies();
      return true;
    },
  };

  class Headers {
    constructor(h) {
      this._h = {};
      const src = plainHeaders(h);
      for (const k of Object.keys(src)) this._h[k.toLowerCase()] = src[k];
    }
    get(k) {
      const v = this._h[String(k).toLowerCase()];
      return v === undefined ? null : Array.isArray(v) ? v.join(", ") : String(v);
    }
    has(k) {
      return own(this._h, String(k).toLowerCase());
    }
    set(k, v) {
      this._h[String(k).toLowerCase()] = String(v);
    }
    append(k, v) {
      const cur = this.get(k);
      this.set(k, cur === null ? v : cur + ", " + v);
    }
    delete(k) {
      delete this._h[String(k).toLowerCase()];
    }
    forEach(f, t) {
      for (const k of Object.keys(this._h)) f.call(t, this.get(k), k, this);
    }
    entries() {
      return Object.keys(this._h).map((k) => [k, this.get(k)])[Symbol.iterator]();
    }
    keys() {
      return Object.keys(this._h)[Symbol.iterator]();
    }
    [Symbol.iterator]() {
      return this.entries();
    }
  }
  g.Headers = Headers;

  // Synchronous like the documented polyfill; also thenable so `fetch(u).then(...)` works.
  g.fetch = function (input, opts) {
    opts = opts || {};
    const url = typeof input === "string" ? input : (input && (input.href || input.url)) || String(input);
    const r = request(opts.method || "GET", url, opts.headers, opts.body);
    if (r.statusCode === 0 && r.error) throw new TypeError("fetch failed: " + r.error);
    const res = {
      ok: r.ok,
      status: r.statusCode,
      statusText: r.statusText || "",
      url: r.url,
      redirected: false,
      headers: new Headers(r.headers),
      text: () => r.body,
      json: () => JSON.parse(r.body),
      arrayBuffer: () => Uint8Array.from(binToBytes(utf8(r.body))).buffer,
      clone: () => res,
    };
    const thenable = Object.assign({}, res);
    Object.defineProperty(thenable, "then", {
      enumerable: false,
      value: (ok, fail) => Promise.resolve(res).then(ok, fail),
    });
    return thenable;
  };

  // ---- Storage, credentials (kept in the OS keychain), logging ----
  g.storage = {
    get: (k, d) => (own(state.storage, k) ? clone(state.storage[k]) : d === undefined ? null : d),
    set: (k, v) => {
      state.storage[String(k)] = clone(v);
      return true;
    },
    remove: (k) => {
      delete state.storage[k];
      return true;
    },
    clear: () => {
      state.storage = {};
      return true;
    },
    keys: () => Object.keys(state.storage),
  };
  g.credentials = {
    store: (k, v) => {
      state.credentials[String(k)] = clone(v);
      return true;
    },
    get: (k) => (own(state.credentials, k) ? clone(state.credentials[k]) : null),
    has: (k) => own(state.credentials, k),
    remove: (k) => {
      delete state.credentials[k];
      return true;
    },
  };
  const show = (v) => {
    if (typeof v === "string") return v;
    if (v instanceof Error) return v.message;
    try {
      return JSON.stringify(v);
    } catch (e) {
      return String(v);
    }
  };
  const logAt = (level) => (msg, data) => {
    H.log(level, show(msg) + (data === undefined ? "" : " " + show(data)));
  };
  g.log = { debug: logAt("debug"), info: logAt("info"), warn: logAt("warn"), error: logAt("error") };
  const consoleAt = (level) => (...a) => H.log(level, a.map(show).join(" "));
  g.console = {
    log: consoleAt("info"),
    info: consoleAt("info"),
    debug: consoleAt("debug"),
    warn: consoleAt("warn"),
    error: consoleAt("error"),
    trace: consoleAt("debug"),
  };

  // ---- utils / crypto ----
  const randomBin = (n) => H.random(Math.max(0, Math.min(1024, n | 0)));
  const uuid = () => {
    const b = binToBytes(randomBin(16));
    b[6] = (b[6] & 0x0f) | 0x40;
    b[8] = (b[8] & 0x3f) | 0x80;
    const h = hex(String.fromCharCode.apply(null, b));
    return h.slice(0, 8) + "-" + h.slice(8, 12) + "-" + h.slice(12, 16) + "-" + h.slice(16, 20) + "-" + h.slice(20);
  };
  // Common desktop browser user agents (some services answer only browsers).
  const USER_AGENTS = [
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36",
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:133.0) Gecko/20100101 Firefox/133.0",
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.1 Safari/605.1.15",
  ];
  g.utils = {
    randomUserAgent: () => USER_AGENTS[Math.floor(Math.random() * USER_AGENTS.length)],
    /** Blocks for up to 10 s (never past the call's deadline). */
    sleep: (ms) => {
      H.sleep(Math.max(0, Math.min(10000, Number(ms) || 0)));
    },
    /** True once Unison no longer wants this call's answer (e.g. a newer search started). */
    isRequestCancelled: () => H.cancelled(),
    parseJSON: (s) => {
      try {
        return JSON.parse(s);
      } catch (e) {
        return null;
      }
    },
    stringifyJSON: (o) => JSON.stringify(o),
    base64Encode: (s) => g.btoa(toBin(s)),
    base64Decode: (s) => fromUtf8(g.atob(s)),
    md5: (s) => hex(H.digest("md5", toBin(s))),
    sha1: (s) => hex(H.digest("sha1", toBin(s))),
    sha256: (s) => hex(H.digest("sha256", toBin(s))),
    hmacSHA256: (message, key) => hex(H.hmac("sha256", toBin(key), toBin(message))),
    hmacSHA256Base64: (message, key) => g.btoa(H.hmac("sha256", toBin(key), toBin(message))),
    hmacSHA1: (key, message) => binToBytes(H.hmac("sha1", toBin(key), toBin(message))),
    encrypt: () => unsupported("utils.encrypt"),
    decrypt: () => unsupported("utils.decrypt"),
    generateKey: (n) => {
      const b = randomBin(n || 32);
      return { success: true, key: g.btoa(b), hex: hex(b) };
    },
    randomUUID: uuid,
  };
  g.crypto = {
    randomUUID: uuid,
    getRandomValues: (arr) => {
      const b = binToBytes(randomBin(arr.length * (arr.BYTES_PER_ELEMENT || 1)));
      const bytes = new Uint8Array(arr.buffer, arr.byteOffset, arr.byteLength);
      for (let i = 0; i < bytes.length; i++) bytes[i] = b[i];
      return arr;
    },
  };

  // ---- gobackend / matching ----
  g.gobackend = {
    getLocalTime: () => {
      const d = new Date();
      let tz = "UTC";
      try {
        tz = Intl.DateTimeFormat().resolvedOptions().timeZone || "UTC";
      } catch (e) {
        /* QuickJS has no Intl */
      }
      return {
        year: d.getFullYear(),
        month: d.getMonth() + 1,
        day: d.getDate(),
        hour: d.getHours(),
        minute: d.getMinutes(),
        second: d.getSeconds(),
        weekday: d.getDay(),
        offsetMinutes: -d.getTimezoneOffset(),
        timezone: tz,
        timestamp: Math.floor(d.getTime() / 1000),
      };
    },
    sanitizeFilename: (s) => String(s).replace(/[\\/:*?"<>|\x00-\x1f]/g, "_").trim(),
    buildFilename: (template, meta) =>
      String(template).replace(/\{(\w+)\}/g, (m, k) => (meta && meta[k] !== undefined ? String(meta[k]) : "")),
    getAudioQuality: () => ({ error: "gobackend.getAudioQuality is not available in Unison" }),
  };
  const normalize = (s) =>
    String(s || "")
      .toLowerCase()
      .normalize("NFKD")
      .replace(/[̀-ͯ]/g, "")
      .replace(/[^a-z0-9]+/g, " ")
      .trim();
  g.matching = {
    normalizeString: normalize,
    compareDuration: (a, b, tol) => Math.abs((a || 0) - (b || 0)) <= (tol === undefined ? 2000 : tol),
    compareStrings: (a, b) => {
      a = normalize(a);
      b = normalize(b);
      if (!a.length && !b.length) return 1;
      const prev = Array.from({ length: b.length + 1 }, (_, i) => i);
      for (let i = 1; i <= a.length; i++) {
        let diag = prev[0];
        prev[0] = i;
        for (let j = 1; j <= b.length; j++) {
          const tmp = prev[j];
          prev[j] = Math.min(prev[j] + 1, prev[j - 1] + 1, diag + (a[i - 1] === b[j - 1] ? 0 : 1));
          diag = tmp;
        }
      }
      return 1 - prev[b.length] / Math.max(a.length, b.length);
    },
  };

  // ---- Not available in Unison: files, ffmpeg, browser sign-in ----
  g.file = {
    download: () => unsupported("file.download"),
    exists: () => false,
    read: () => null,
    write: () => unsupported("file.write"),
    delete: () => unsupported("file.delete"),
    move: () => unsupported("file.move"),
  };
  g.ffmpeg = {
    execute: () => unsupported("ffmpeg.execute"),
    getInfo: () => unsupported("ffmpeg.getInfo"),
    convert: () => unsupported("ffmpeg.convert"),
  };
  const b64url = (b) => g.btoa(b).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
  g.auth = {
    openAuthUrl: () => unsupported("auth.openAuthUrl"),
    startOAuthWithPKCE: () => unsupported("auth.startOAuthWithPKCE"),
    exchangeCodeWithPKCE: () => unsupported("auth.exchangeCodeWithPKCE"),
    getAuthCode: () => null,
    setAuthCode: (o) => {
      state.credentials.__auth = clone(o || {});
      return true;
    },
    isAuthenticated: () => !!(state.credentials.__auth && state.credentials.__auth.access_token),
    getTokens: () => {
      const a = state.credentials.__auth || {};
      return {
        access_token: a.access_token || null,
        refresh_token: a.refresh_token || null,
        is_authenticated: !!a.access_token,
        expires_at: a.expires_at || null,
        is_expired: !!(a.expires_at && a.expires_at * 1000 < Date.now()),
      };
    },
    clearAuth: () => {
      delete state.credentials.__auth;
      return true;
    },
    generatePKCE: (len) => {
      const n = Math.max(43, Math.min(128, len || 64));
      const verifier = b64url(randomBin(96)).slice(0, n);
      const pkce = { verifier, challenge: b64url(H.digest("sha256", verifier)), method: "S256" };
      state.credentials.__pkce = pkce;
      return pkce;
    },
    getPKCE: () => state.credentials.__pkce || null,
  };

  // ---- Timers run on the next tick (no real delays inside one call) ----
  let timerId = 0;
  g.setTimeout = (fn, ms, ...args) => {
    const id = ++timerId;
    if (typeof fn === "function") Promise.resolve().then(() => fn(...args));
    return id;
  };
  g.setInterval = () => ++timerId;
  g.clearTimeout = () => {};
  g.clearInterval = () => {};
  g.queueMicrotask = (fn) => Promise.resolve().then(fn);

  // ---- Registration and calls from Unison ----
  g.registerExtension = (ext) => {
    g.__ext = ext;
    return true;
  };
  const functionNames = (o) => {
    const names = [];
    for (let p = o; p && p !== Object.prototype; p = Object.getPrototypeOf(p))
      for (const k of Object.getOwnPropertyNames(p))
        if (k !== "constructor" && typeof o[k] === "function" && !names.includes(k)) names.push(k);
    return names;
  };
  let out = { state: "pending" };
  g.__unisonFunctions = () => JSON.stringify(g.__ext ? functionNames(g.__ext) : null);
  g.__unisonCall = (names, args) => {
    out = { state: "pending" };
    const ext = g.__ext;
    if (!ext) {
      out = { state: "error", error: "extension did not call registerExtension()" };
      return;
    }
    const name = names.find((n) => typeof ext[n] === "function");
    if (!name) {
      out = { state: "missing" };
      return;
    }
    const fail = (e) => {
      out = { state: "error", error: String((e && (e.message || e)) || "extension error") };
    };
    const settle = (r) => {
      if (r && typeof r.then === "function")
        r.then((v) => {
          out = { state: "ok", value: v === undefined ? null : v };
        }, fail);
      else out = { state: "ok", value: r === undefined ? null : r };
    };
    const run = () => {
      try {
        settle(ext[name].apply(ext, args));
      } catch (e) {
        fail(e);
      }
    };
    try {
      const ready = typeof ext.initialize === "function" ? ext.initialize(clone(settings)) : undefined;
      if (ready && typeof ready.then === "function") ready.then(run, fail);
      else run();
    } catch (e) {
      fail(e);
    }
  };
  g.__unisonTake = () => {
    try {
      return JSON.stringify(out);
    } catch (e) {
      return JSON.stringify({ state: "error", error: "result is not JSON: " + e.message });
    }
  };
  g.__unisonState = () => JSON.stringify(state);
})(globalThis);
