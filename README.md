# rust-cf-turnstile-bypass

A proof-of-concept Cloudflare Turnstile bypass system built in Rust. Includes a token harvesting mechanism comprising a widget generator, a Turnstile checkbox clicker, and a token server for receiving and managing solved tokens. No API service required. 

---

# Pros

- No API service is required.
- Solver can effectively generate many tokens per minute, even without the use of an API service. 
- Data is handled and already managed by a server that makes managing your haverested tokens easy.
- Method is generally effective when you know the website you want to apply it to beforehand.
- The method is relatively firm and not as easy to patch as other bypasses, as it relies on overriding pages to avoid any policies like CORs or any fingerprinting, and the checkbox identifier will work as long as Cloudflare does not drastically change the UI of the widget itself.
- Easy to use and setup, especially compared to certain other bypasses.

## Caveats

- The solver is **not headless** — a GUI is required.
- The method relies on a browser with overrides enabled.
- Uses iframes to create multiple simultaneous solvers.
- Tunneling multiple proxies through each iframe is not supported. Do note this may potentially be added in the future if a feasible solution is found, though. For now, the only solution for multi-proxy support is to spawn multiple windows, and use a browser extension that enables per-window proxies (e.g. FoxyProxy).
- Designed for smaller-scale token harvesting, though the token server architecture does support larger-scale operations.
- Ineffective for general, random web-scraping. Knowing the websites it will be used on is most effective.

---

## How It Works

The bypass is comprised of three main components:

1. **Token Harvester / Turnstile Widget Loader**
2. **Turnstile Widget Identifier & Clicker**
3. **Token Server**

---

### 1. Token Harvester / Turnstile Widget Loader

The Token Harvester loads the Turnstile widget by spawning multiple iframe-based solvers, each pointing at a different Cloudflare site widget. Every solver iframe connects to the token server and forwards any solved tokens to it, and after forwarding a token it also resets the widget and begins solving for another token.

**Setup:**

1. **Configure the files.** Configuration is place in `index.html`:
   - Set `PRELOAD_IFRAMES` (the number of iframe solvers to load on page start), `TOKEN_SERVER_HOST` (your token server host, obviously), and `SITEKEY` (the website's Cloudflare sitekey).

2. **Apply as browser overrides.** Replace the target webpage's main HTML file with `index.html`, and its main JS script with `index.js`. If the site inlines its scripts, you can still override with `index.js` — since `index.html` is also overridden, it will be loaded as a script regardless. If this is not applicable due to say, tricky origin stuff or something of that sort, you can also of course inline the script into the index.html.

**Why overrides?**

Using overrides does require loading the actual page, but it sidesteps issues with CORS policies, TLS fingerprinting, and other browser/address analysis the target site may employ. Because the page loads normally and passes all standard security checks, our modified scripts can generate tokens cleanly without triggering those protections.

---

### 2. Turnstile Clicker

The Turnstile Clicker automatically solves checkbox click challenges. Run the relevant `main.rs` file to start it. The clicker is **disabled by default** — press **F8** to toggle it on or off.

**Setup:**

Set the config values described in `main.rs`. That's all, aside from installing dependencies.

**How it works:**

The clicker identifies Cloudflare Turnstile checkboxes by analyzing pixel RGB values. It searches for pixels matching the characteristic grey ring border of the Turnstile checkbox. Once a candidate pixel is found, it performs a depth-first search (DFS) to verify the pixel forms a closed ring/loop. It then searches inward from all four sides to isolate the whitespace within the border — the actual clickable area. Finally, it dispatches OS-level input events to move the mouse to a point within that region and click.

> **Note:** The F8 toggle exists for good reason. The token harvester page is entirely black and contains nothing that should be falsely detected as a checkbox. However, other pages may produce false positives, so it's recommended to only enable the clicker when the solver page is active.

---

### 3. Token Server

The Token Server doesn't participate in solving — it stores and manages the tokens produced by the harvester. Solver iframes forward their tokens here as they're solved.

**Setup:**

Set the `PORT`, and `PROXIES_LIST_LENGTH` values in the config. That's all, aside from dependencies.

**Packet & Protocol Structure:**

*Serverbound (client → server):*

| Header | Description |
|--------|-------------|
| `0` | Incoming token + solver id from a sender. The server routes it to the registered receiver socket with the fewest acquired tokens (based on total acquired, not taking into account tokens that were already consumed). Structure: <0, ...sender_id_bytes (u32), ...token_bytes>. |
| `1` | Register the sending socket as a receiver and initialize its receiver status. Send this packet when designing a system to actually allow your infrastructure to acquire the tokens. |
| `2` | Request the total token count. The server responds with the current count. |
| `3` | Request the solver_idx. The server responds with this window's solver_idx. Necessary for knowing which proxy solved a challenge in case there are IP checks in place. |

*Clientbound (server → client):*

| Description |
|-------------|
| Incoming token + solver id delivered to a receiver. Structure: <...sender_id_bytes (u32), ...token_bytes>.|
| Token count response. Sent directly to the requesting client as u64 LE bytes without a header, since that client only needs this single value and no additional packet types are currently required. |
| Solver_idx response. Sent directly as u32 LE bytes to the requesting client. |

*Note these packets dote not have headers, as there is only one packet type sent to each endpoint.*

---

### 4. Extensions

I would recommend a few browser extensions to maximize solving potential:
1. A per-window, advanced browser proxy extension that allows fine-grained control over browser level proxies. A good example of this is FoxyProxy. A system that can rotate a proxy list upon window reload is most important to be compatible for multi-proxy solving with this architecture.
2. A WebRTC api spoofer or blocker. WebRTC can leak your real IP if not careful, so getting a good extension to block this is critical.
3. An advanced user-agent spoofer. This one isn't all that necessary, but if you're looking to maximize anonymity then you'll likely want one of these. 

---

## Starting It Up

1. Start the **token server**.
2. Start your backend, token managing system. 
3. Start the **auto-clicker**.
4. Open your **modified webpage**.
5. Press **F8** to enable the auto-clicker.
6. Watch it go.

---

## Future Plans (may not be done, but if major updates do occur to this project it will likely be these).
An automatic page-loader and harvester setup script may be created in order to aid with multi-proxy solving, as per page loads are currently needed for such.

If a feasible solution is found, a way to tunnel individual iframes (hence enhancing multi-proxy solving outside of just different tabs) may be implemented.

<img width="1919" height="907" alt="image" src="https://github.com/user-attachments/assets/7e6c9bfb-e720-4c21-a8d6-88b699e5af88" />
