// Object map for proxy ID info.
let tab_proxies = {};

let origin_proxies = {};

// Object map for per-tab / per-origin user agent overrides.
let tab_user_agents = {};

let origin_user_agents = {};

// Listen to messages posted by the client if we want to set up a proxy.
browser.runtime.onMessage.addListener((message, sender, send_response) => {
    if (message.action == "setup_proxy" && sender.tab) {
        let tab_id = sender.tab.id;
        let proxy_string = message.proxy_details;
        let tab_origin = new URL(sender.tab.url).origin;
        
        try {
            let url = new URL(proxy_string);
            let proxy_type = url.protocol.replace(":", "").toLowerCase();
            
            // Firefox requires the extension be written as "socks".
            if (proxy_type == "socks5") proxy_type = "socks";

            let host_name = url.hostname;
            let port_num = parseInt(url.port, 10);

            if (["http", "https", "socks", "socks4"].includes(proxy_type) && host_name && port_num) {
                
                let proxy_config = { 
                    type: proxy_type, 
                    host: host_name, 
                    port: port_num,
                    proxyDNS: true 
                };

                tab_proxies[tab_id] = proxy_config;
                origin_proxies[tab_origin] = proxy_config;

                // Optional user agent override. If none was passed, clear any
                // previously stored override so the tab reverts to its real UA.
                if (message.user_agent) {
                    tab_user_agents[tab_id] = message.user_agent;
                    origin_user_agents[tab_origin] = message.user_agent;
                    console.log(`[Proxy Bridge] Tab ${tab_id} user agent overridden to: ${message.user_agent}`);
                } else {
                    delete tab_user_agents[tab_id];
                    delete origin_user_agents[tab_origin];
                }
                
                console.log(`[Proxy Bridge] Tab ${tab_id} bound to ${proxy_type}://${host_name}:${port_num}`);
                send_response({ success: true });
            } else {
                console.error(`[Proxy Bridge] Invalid proxy format or unsupported protocol: ${proxy_type}`);
                send_response({ success: false });
            }
        } catch (err) {
            console.error("[Proxy Bridge] Failed to parse proxy URL.", err);
            send_response({ success: false });
        }
    }
    
    return false; 
});

// Listen and tunnel requests if a proxy is set.
browser.proxy.onRequest.addListener(
    (details) => {
        let proxy_info = tab_proxies[details.tabId];
        
        if (!proxy_info && details.tabId == -1) {
            let request_origin = details.originUrl ? new URL(details.originUrl).origin : null;
            if (request_origin && origin_proxies[request_origin]) {
                proxy_info = origin_proxies[request_origin];
            }
        }
        
        if (proxy_info) {
            return [proxy_info]; 
        }
        
        return [{ type: "direct" }]; 
    },
    { urls: ["<all_urls>"] }
);

browser.proxy.onError.addListener(error => {
    console.error(`[Proxy Bridge] Network error:`, error.message);
});

// Rewrite the outgoing User-Agent header for any tab/origin that has an
// override configured. Tabs without one are left alone and send their real UA.
browser.webRequest.onBeforeSendHeaders.addListener(
    (details) => {
        let ua_override = tab_user_agents[details.tabId];

        if (!ua_override && details.tabId == -1) {
            let request_origin = details.originUrl ? new URL(details.originUrl).origin : null;
            if (request_origin && origin_user_agents[request_origin]) {
                ua_override = origin_user_agents[request_origin];
            }
        }

        if (ua_override) {
            let headers = details.requestHeaders.filter(
                (header) => header.name.toLowerCase() !== "user-agent"
            );
            headers.push({ name: "User-Agent", value: ua_override });
            return { requestHeaders: headers };
        }

        return {};
    },
    { urls: ["<all_urls>"] },
    ["blocking", "requestHeaders"]
);

browser.tabs.onRemoved.addListener((tab_id) => {
    if (tab_proxies[tab_id]) {
        console.log(`[Proxy Bridge] Tab ${tab_id} closed. Clearing proxy mapping.`);
        delete tab_proxies[tab_id];
    }
    if (tab_user_agents[tab_id]) {
        delete tab_user_agents[tab_id];
    }
});