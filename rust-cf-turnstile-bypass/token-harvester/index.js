(function () {
    'use strict';

    let start_time = Date.now();
    globalThis.run_time = function () {
        return ((Date.now() - start_time) / 1000).toFixed(3) + 's';
    };

    try { localStorage.removeItem('cached_captcha_token'); } catch (e) {}

    let resolvers = [];
    let token = null;
    let retry_timeout = null;
    let retry_count = 0;
    let max_retries = 10;
    let retry_delay_ms = 2000;

    let token_sender = new WebSocket(window.top.token_server_host);

    Object.defineProperty(window, 'captcha_token', {
        configurable: true,
        get: function () { return token; },
        set: function (v) {
            token = v;
            console.log("Token: " + v);
            retry_count = 0;
            console.log('[turnstile] token:', v, '| time:', run_time());
            resolvers.forEach(function (r) { r(v); });
            resolvers = [];
            let bytes = new TextEncoder().encode(v);
            let pkt = new Uint8Array(bytes.length + 5);
            // Send the token packet to the token server.
            pkt[0] = 0;
            pkt[1] = window.top.solver_idx & 255;
            pkt[2] = (window.top.solver_idx >> 8) & 255;
            pkt[3] = (window.top.solver_idx >> 16) & 255;
            pkt[4] = (window.top.solver_idx >> 24) & 255;
            pkt.set(bytes, 5);
            if (token_sender.readyState == 1) {
                token_sender.send(pkt);
            } else {
                token_sender.onopen = function() {
                    token_sender.send(pkt);
                }
            }
            // Reset widget if in multiple solvers, reload page if on one.
            if (window.top.preload_iframes != 1) {
                window.reset_turnstile();
                window.cf__reactTurnstileOnLoad();
            } else {
                location.reload();
            }
        }
    });

    window.get_turnstile_token = function () {
        if (token) return Promise.resolve(token);
        return new Promise(function (r) { resolvers.push(r); });
    };

    window.reset_turnstile = function () {
        token = null;
        if (retry_timeout) { clearTimeout(retry_timeout); retry_timeout = null; }
        try { localStorage.removeItem('cached_captcha_token'); } catch (e) {}
        if (window.turnstile) { try { window.turnstile.reset(); } catch (e) {} }
    };

    function schedule_retry(reason) {
        if (retry_count >= max_retries) {
            console.warn('[turnstile] max retries reached, giving up');
            return;
        }
        retry_count++;
        let delay = retry_delay_ms * retry_count;
        console.log('[turnstile] ' + reason + ' — retry ' + retry_count + '/' + max_retries + ' in ' + delay + 'ms');
        if (retry_timeout) clearTimeout(retry_timeout);
        retry_timeout = setTimeout(function () {
            retry_timeout = null;
            console.log('[turnstile] retrying render...');
            window.reset_turnstile();
            window.cf__reactTurnstileOnLoad();
        }, delay);
    }

    window.cf__reactTurnstileOnLoad = function () {
        let container = document.getElementById('__ts_container__');
        if (!container) {
            container = document.createElement('div');
            container.id = '__ts_container__';
            container.style.cssText = 'position:fixed;top:50%;left:50%;transform:translate(-50%,-50%);z-index:9999;';
            document.body.appendChild(container);
        }

        container.innerHTML = '';

        window.turnstile.render(container, {
            sitekey: window.top.sitekey,
            appearance: 'always',
            callback: function (t) {
                window.captcha_token = t;
                try { localStorage.setItem('cached_captcha_token', t); } catch (e) {}
            },
            'error-callback': function (code) {
                console.log('[turnstile] error:', code);
                if (typeof window.on_turnstile_error == 'function') window.on_turnstile_error(code);
                schedule_retry('error ' + code);
            },
            'expired-callback': function () {
                token = null;
                console.log('[turnstile] token expired, retrying');
                schedule_retry('expired');
            }
        });
    };
})();
