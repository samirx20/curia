import QtQuick
import QtWebView

// The page itself, in Windows' own web engine (WebView2) through Qt WebView. Kept apart so
// the rest of the app loads even where that engine is missing.
WebView {
    id: page
    signal opened(string url)
    onUrlChanged: page.opened(url.toString())
    onLoadingChanged: info => console.log("web: " + (info.status === WebView.LoadSucceededStatus ? "loaded " : info.status === WebView.LoadFailedStatus ? "failed " + info.errorString + " " : "loading ") + info.url)
}
