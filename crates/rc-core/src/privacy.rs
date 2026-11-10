//! Privacy payloads shared by both shells.
//!
//! The Linux shell pins a generic machine through WebKitGTK settings plus this
//! kind of script. The Windows shell has fewer engine switches, so it leans on
//! the script more heavily — but it must not lie about the engine: on Windows
//! the UA is set through WebView2 settings (Chromium), never overridden to a
//! Firefox string, because an engine mismatch is itself a fingerprint.

/// Injected stylesheet used by "force dark pages". Filters are the only way to
/// darken arbitrary pages without a full renderer feature.
pub const FORCE_DARK_CSS: &str = r#"
html { background-color: #ffffff !important; }
html { filter: invert(0.92) hue-rotate(180deg) !important; }
img, picture, video, canvas, svg, iframe { filter: invert(1) hue-rotate(180deg) !important; }
"#;

/// Fixed-profile anti-fingerprinting for Blink.
///
/// Deliberately does NOT touch `navigator.userAgent` or the client hints: the
/// UA string is set through WebView2 settings so the HTTP header and the JS
/// view agree, and client hints are handled by that same override.
pub const ANTI_FP_JS: &str = r##"
(() => {
  const def = (o, p, v) => { try { Object.defineProperty(o, p, { get: () => v, configurable: true }); } catch (e) {} };
  const W = 1280, H = 800;

  if (screen) {
    def(screen, 'width', W); def(screen, 'height', H);
    def(screen, 'availWidth', W); def(screen, 'availHeight', H - 40);
    def(screen, 'colorDepth', 24); def(screen, 'pixelDepth', 24);
    try {
      const o = screen.orientation;
      if (o) { def(o, 'angle', 0); def(o, 'type', 'landscape-primary'); }
    } catch (e) {}
  }
  def(window, 'devicePixelRatio', 1);
  def(window, 'outerWidth', W); def(window, 'outerHeight', H);

  if (navigator) {
    def(navigator, 'hardwareConcurrency', 2);
    def(navigator, 'deviceMemory', 4);
    def(navigator, 'languages', Object.freeze(['en-US', 'en']));
    def(navigator, 'language', 'en-US');
    def(navigator, 'doNotTrack', '1');
    def(navigator, 'webdriver', false);
    try {
      if (navigator.userAgentData) {
        const brands = [{ brand: 'Chromium', version: '124' }];
        const high = () => Promise.resolve({
          architecture: 'x86', bitness: '64', brands: brands,
          fullVersionList: [{ brand: 'Chromium', version: '124.0.0.0' }],
          mobile: false, model: '', platform: 'Windows',
          platformVersion: '10.0.0', uaFullVersion: '124.0.0.0', wow64: false
        });
        def(navigator, 'userAgentData', {
          brands: brands, mobile: false, platform: 'Windows',
          getHighEntropyValues: high, toJSON: () => ({ brands: brands, mobile: false, platform: 'Windows' })
        });
      }
    } catch (e) {}
    try {
      if (navigator.geolocation) {
        navigator.geolocation.getCurrentPosition = (ok, err) => {
          if (err) err({ code: 1, message: 'denied', PERMISSION_DENIED: 1 });
        };
        navigator.geolocation.watchPosition = () => 0;
      }
    } catch (e) {}
    try {
      if (navigator.mediaDevices) {
        navigator.mediaDevices.enumerateDevices = () => Promise.resolve([]);
        navigator.mediaDevices.getUserMedia = () => Promise.reject(new Error('denied'));
      }
    } catch (e) {}
  }

  // WebGL is not switchable off at engine level on WebView2, so neutralise the
  // context factory and mask the renderer strings.
  try {
    const noCtx = function (type) {
      const t = String(type).toLowerCase();
      return (t.indexOf('webgl') === 0) ? null : null;
    };
    const orig = HTMLCanvasElement.prototype.getContext;
    HTMLCanvasElement.prototype.getContext = function (type, ...rest) {
      const t = String(type).toLowerCase();
      if (t === 'webgl' || t === 'webgl2' || t === 'experimental-webgl') return null;
      return orig.call(this, type, ...rest);
    };
    void noCtx;
  } catch (e) {}

  // Canvas readback noise (deterministic per pixel index, invisible on screen).
  try {
    const salt = (d) => { for (let i = 0; i < d.data.length; i += 997) d.data[i] = (d.data[i] + 1) & 255; };
    const gid = CanvasRenderingContext2D.prototype.getImageData;
    CanvasRenderingContext2D.prototype.getImageData = function (...a) {
      const d = gid.apply(this, a); salt(d); return d;
    };
  } catch (e) {}

  // WebRTC: no engine switch on WebView2, so remove the constructor entirely.
  try {
    delete window.RTCPeerConnection;
    delete window.webkitRTCPeerConnection;
    delete window.RTCDataChannel;
    if (window.navigator) delete navigator.mediaDevices;
  } catch (e) {}

  // Fixed clock: UTC offset 0 everywhere the page can look.
  try {
    Date.prototype.getTimezoneOffset = () => 0;
    const ro = Intl.DateTimeFormat.prototype.resolvedOptions;
    Intl.DateTimeFormat.prototype.resolvedOptions = function () {
      const o = ro.apply(this, arguments);
      try { o.timeZone = 'UTC'; } catch (e) {}
      return o;
    };
  } catch (e) {}
})();
"##;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scripts_are_non_trivial() {
        assert!(ANTI_FP_JS.len() > 500);
        assert!(ANTI_FP_JS.contains("hardwareConcurrency"));
        assert!(FORCE_DARK_CSS.contains("invert"));
    }

    #[test]
    fn antifp_does_not_lie_about_the_engine() {
        // A Firefox UA on a Chromium engine is a stronger fingerprint than none.
        assert!(!ANTI_FP_JS.contains("rv:128.0"));
        assert!(!ANTI_FP_JS.contains("Firefox"));
    }
}
