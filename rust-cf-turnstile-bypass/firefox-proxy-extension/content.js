window.addEventListener("message", (event) => {
    // Data must come from our own webpage.
    if (event.source != window || !event.data) return;

    if (event.data.type == "SET_TAB_PROXY") {
        // Forward the requested proxy to the background.
        browser.runtime.sendMessage({
            action: "setup_proxy",
            proxy_details: event.data.proxy_details
        }).then((response) => {
            if (response && response.success) {
                // Return message to the page telling the client the proxy is reading--async unlocks once recieved,
                // giving us a mock lock.
                window.postMessage({ type: "PROXY_READY" }, "*");
            }
        }).catch((err) => {
            console.error("Extension bridge error:", err);
        });
    }
});