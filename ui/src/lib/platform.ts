// The WebView user agent is enough to tell Windows (WebView2) from macOS (WKWebView).
export const isWindows =
  typeof navigator !== 'undefined' && /Windows/i.test(navigator.userAgent)
