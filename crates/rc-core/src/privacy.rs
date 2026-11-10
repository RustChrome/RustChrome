//! Privacy payloads shared by both shells.
//!
//! Rules this module follows:
//! * never contradict the engine (no fake Firefox UA on Chromium, no pinned
//!   screen that disagrees with the window, no timezone that disagrees with
//!   `Date#toString`),
//! * never break ordinary sites for no reason (WebGL stays usable and is only
//!   coerced; WebRTC stays usable unless the user picks Strict),
//! * never claim anonymity. These are mitigations, not Tor.

use std::time::{SystemTime, UNIX_EPOCH};

/// A random value drawn once per browser session. It keys the canvas noise, so
/// a page gets the same pixels twice in a row but a different canvas next run —
/// deterministic noise would be a stable identifier, which is worse than none.
#[derive(Clone, Copy, Debug)]
pub struct SessionSeed(pub u32);

impl SessionSeed {
    pub fn new() -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0x9e37_79b9_7f4a_7c15);
        let mut x = nanos ^ ((std::process::id() as u64) << 32);
        x ^= x >> 33;
        x = x.wrapping_mul(0xff51_afd7_ed55_8ccd);
        x ^= x >> 29;
        SessionSeed((x as u32) | 1)
    }

    pub fn rotate(&mut self) {
        *self = SessionSeed::new();
    }
}

impl Default for SessionSeed {
    fn default() -> Self {
        Self::new()
    }
}

/// Injected stylesheet used by "force dark pages".
pub const FORCE_DARK_CSS: &str = r#"
html { background-color: #ffffff !important; }
html { filter: invert(0.92) hue-rotate(180deg) !important; }
img, picture, video, canvas, svg, iframe { filter: invert(1) hue-rotate(180deg) !important; }
"#;

const ANTI_FP_TEMPLATE: &str = r##"
(() => {
  const SEED = __SEED__ >>> 0;
  const STRICT = __STRICT__;
  const def = (o, p, v) => { try { Object.defineProperty(o, p, { get: () => v, configurable: true }); } catch (e) {} };

  // The user agent is NOT overridden here. The host sets it through engine
  // settings so the HTTP header, the client hints and the script agree; a JS
  // override would leave the header untouched and create a contradiction.

  // Common desktop values rather than 2 cores / 4 GB: those sit in a narrow,
  // easily recognised bucket.
  def(navigator, 'hardwareConcurrency', 8);
  def(navigator, 'deviceMemory', 8);
  def(navigator, 'languages', Object.freeze(['en-US', 'en']));
  def(navigator, 'language', 'en-US');
  def(navigator, 'doNotTrack', '1');
  def(navigator, 'webdriver', false);

  // Clock: offset, Intl view and Date#toString all agree on UTC.
  try {
    Date.prototype.getTimezoneOffset = () => 0;
    const dts = Date.prototype.toString;
    Date.prototype.toString = function () {
      return dts.call(this).replace(/GMT[+-]\d{4}[^)]*\)/, 'GMT+0000 (Coordinated Universal Time)');
    };
    const ro = Intl.DateTimeFormat.prototype.resolvedOptions;
    Intl.DateTimeFormat.prototype.resolvedOptions = function () {
      const o = ro.apply(this, arguments);
      try { o.timeZone = 'UTC'; } catch (e) {}
      return o;
    };
  } catch (e) {}

  // Device enumeration: Chromium already hides labels without permission; this
  // drops the (still enumerable) device ids too.
  try {
    if (navigator.mediaDevices) {
      navigator.mediaDevices.enumerateDevices = () => Promise.resolve([]);
    }
  } catch (e) {}

  // Canvas noise: a pure function of (byte index, session seed). Same session =>
  // identical pixels every read (sites keep working); new session => different
  // hash (no cross-session identifier).
  const bit = (i) => {
    let x = (i ^ SEED) >>> 0;
    x = Math.imul(x ^ (x >>> 15), 0x2c1b3c6d) >>> 0;
    x = Math.imul(x ^ (x >>> 12), 0x297a2d39) >>> 0;
    return (x >>> 16) & 1;
  };
  const salt = (d) => {
    const n = d.data.length;
    for (let i = 0; i + 4 <= n; i += 16) d.data[i] = (d.data[i] ^ bit(i)) & 255;
  };
  try {
    const proto = CanvasRenderingContext2D.prototype;
    const gid = proto.getImageData;
    const tdu = HTMLCanvasElement.prototype.toDataURL;
    let painting = false;
    proto.getImageData = function () {
      const d = gid.apply(this, arguments);
      if (!painting) {
        try { salt(d); } catch (e) {}
      }
      return d;
    };
    HTMLCanvasElement.prototype.toDataURL = function () {
      try {
        const c = document.createElement('canvas');
        c.width = this.width; c.height = this.height;
        const x = c.getContext('2d');
        if (!x) return tdu.apply(this, arguments);
        x.drawImage(this, 0, 0);
        painting = true;
        const d = gid.call(x, 0, 0, c.width, c.height);
        painting = false;
        salt(d);
        x.putImageData(d, 0, 0);
        return tdu.apply(c, arguments);
      } catch (e) {
        return tdu.apply(this, arguments);
      }
    };
  } catch (e) {}

  // WebGL: keep the API (maps, games, 3D sites keep working) and coerce the two
  // strings that actually identify the GPU. Strict mode removes the context.
  try {
    for (const name of ['WebGLRenderingContext', 'WebGL2RenderingContext']) {
      const C = window[name];
      if (!C) continue;
      const gp = C.prototype.getParameter;
      C.prototype.getParameter = function (p) {
        if (p === 37445) return 'Generic';
        if (p === 37446) return 'Generic Renderer';
        return gp.apply(this, arguments);
      };
    }
    if (STRICT) {
      const orig = HTMLCanvasElement.prototype.getContext;
      HTMLCanvasElement.prototype.getContext = function (type) {
        const t = String(type).toLowerCase();
        if (t === 'webgl' || t === 'webgl2' || t === 'experimental-webgl') return null;
        return orig.apply(this, arguments);
      };
    }
  } catch (e) {}

  // WebRTC: request only relay candidates (no LAN address, no direct peer
  // connection) rather than deleting the API. Strict mode deletes it.
  try {
    if (STRICT) {
      delete window.RTCPeerConnection;
      delete window.webkitRTCPeerConnection;
      delete window.RTCDataChannel;
    } else if (window.RTCPeerConnection) {
      const RTC = window.RTCPeerConnection;
      const Patched = function (cfg, ...rest) {
        const c = Object.assign({}, cfg || {});
        c.iceTransportPolicy = 'relay';
        return new RTC(c, ...rest);
      };
      Patched.prototype = RTC.prototype;
      try { Object.setPrototypeOf(Patched, RTC); } catch (e) {}
      def(window, 'RTCPeerConnection', Patched);
    }
  } catch (e) {}
})();
"##;

/// Anti-fingerprinting payload for the current session.
pub fn antifp_js(seed: SessionSeed, strict: bool) -> String {
    ANTI_FP_TEMPLATE
        .replace("__SEED__", &seed.0.to_string())
        .replace("__STRICT__", if strict { "true" } else { "false" })
}

/// JavaScript that produces the probe report. Kept in the core so both shells
/// measure exactly the same things in exactly the same way.
pub const PROBE_JS: &str = r##"
(() => {
  const fnv = (str) => {
    let h = 0x811c9dc5;
    for (let i = 0; i < str.length; i++) {
      h ^= str.charCodeAt(i);
      h = Math.imul(h, 0x01000193) >>> 0;
    }
    return ('00000000' + h.toString(16)).slice(-8);
  };
  const canvasHash = () => {
    try {
      const c = document.createElement('canvas');
      c.width = 220; c.height = 60;
      const x = c.getContext('2d');
      if (!x) return 'no-2d';
      x.textBaseline = 'top';
      x.font = '16px Arial';
      x.fillStyle = '#f60';
      x.fillRect(0, 0, 220, 60);
      x.fillStyle = '#069';
      x.fillText('RustChrome probe \u{1F510}', 4, 8);
      x.fillStyle = 'rgba(102,204,0,0.7)';
      x.fillText('gnirts tset', 90, 30);
      const d = x.getImageData(0, 0, 220, 60);
      let s = '';
      for (let i = 0; i < d.data.length; i += 97) s += String.fromCharCode(d.data[i]);
      return fnv(s);
    } catch (e) { return 'err:' + e.name; }
  };
  const gl = () => {
    try {
      const c = document.createElement('canvas');
      const g = c.getContext('webgl') || c.getContext('experimental-webgl');
      if (!g) return { available: false };
      const dbg = g.getExtension('WEBGL_debug_renderer_info');
      return {
        available: true,
        vendor: dbg ? String(g.getParameter(dbg.UNMASKED_VENDOR_WEBGL)) : 'masked',
        renderer: dbg ? String(g.getParameter(dbg.UNMASKED_RENDERER_WEBGL)) : 'masked'
      };
    } catch (e) { return { available: false, error: e.name }; }
  };
  // The probe page is a data: document with an opaque origin, so storage
  // access throws there. Probe, never assume.
  const safe = (f, fallback) => { try { return f(); } catch (e) { return fallback; } };
  const out = {
    ua: navigator.userAgent,
    platform: navigator.platform || '',
    brands: navigator.userAgentData ? JSON.stringify(navigator.userAgentData.brands) : 'none',
    hc: navigator.hardwareConcurrency,
    mem: navigator.deviceMemory || null,
    langs: (navigator.languages || []).join(','),
    lang: navigator.language,
    tz: Intl.DateTimeFormat().resolvedOptions().timeZone,
    tzoff: new Date().getTimezoneOffset(),
    datestr: new Date(2020, 5, 15, 12, 0, 0).toString(),
    screen: [screen.width, screen.height, screen.availWidth, screen.availHeight, screen.colorDepth],
    dpr: window.devicePixelRatio,
    outer: [window.outerWidth, window.outerHeight],
    inner: [window.innerWidth, window.innerHeight],
    plugins: navigator.plugins.length,
    dnt: navigator.doNotTrack,
    cookieEnabled: navigator.cookieEnabled,
    storage: {
      ls: safe(() => !!window.localStorage, false),
      idb: safe(() => !!window.indexedDB, false),
      sw: safe(() => 'serviceWorker' in navigator, false)
    },
    webrtc: typeof window.RTCPeerConnection,
    canvas1: canvasHash(),
    canvas2: canvasHash(),
    webgl: gl(),
    permissions: typeof navigator.permissions !== 'undefined'
  };
  return JSON.stringify(out);
})()
"##;

/// Small page that runs the probe and exposes it as `window.__probe()`.
pub fn probe_html() -> String {
    r#"<!doctype html><html><head><meta charset="utf-8"><title>RustChrome privacy probe</title>
<style>body{font-family:system-ui,sans-serif;margin:40px;background:#fff;color:#202124}
code{background:#f1f3f4;padding:2px 5px;border-radius:4px}</style></head><body>
<h1>Privacy probe</h1>
<p>Open the console and run <code>__probe()</code>, or let the harness capture it.</p>
<pre id="out"></pre>
<script>
window.__probe = function () {
  const s = __PROBE__;
  document.getElementById('out').textContent = s;
  return s;
};
// The report is published as the title so a host can read it with the plain
// "document title" API instead of evaluating JavaScript.
try { document.title = window.__probe(); } catch (e) { document.title = 'FAIL ' + e; }
</script></body></html>"#
        .replace("__PROBE__", PROBE_JS)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn payload_is_non_trivial_and_seeded() {
        let js = antifp_js(SessionSeed(12345), false);
        assert!(js.contains("12345"));
        assert!(js.contains("hardwareConcurrency"));
        assert!(js.contains("iceTransportPolicy"));
        assert!(!js.contains("__SEED__"));
    }

    #[test]
    fn strict_mode_differs() {
        let a = antifp_js(SessionSeed(1), false);
        let b = antifp_js(SessionSeed(1), true);
        assert!(b.contains("const STRICT = true"));
        assert!(a.contains("const STRICT = false"));
    }

    #[test]
    fn seeds_are_odd_and_vary() {
        let a = SessionSeed::new();
        let b = SessionSeed::new();
        assert_eq!(a.0 % 2, 1);
        assert_ne!(a.0, b.0, "two sessions must not share a canvas seed");
    }

    #[test]
    fn does_not_lie_about_the_engine() {
        let js = antifp_js(SessionSeed(1), false);
        assert!(!js.contains("Firefox"));
        assert!(!js.contains("rv:128"));
        // Canvas noise must be a function of the seed, not a constant pattern.
        assert!(js.contains("bit(i)"));
    }

    #[test]
    fn probe_reports_the_things_we_tested() {
        for key in ["canvas1", "webgl", "tzoff", "hc", "webrtc", "plugins"] {
            assert!(PROBE_JS.contains(key), "probe missing {key}");
        }
        assert!(probe_html().contains("__probe"));
    }
}
