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
- This method has a far higher success rate than any other standard method. It quite literally has held 100% success (not a singular fail) as of July 10, 2026 (after thousands of solves). 
- Because this method uses standard Mozilla Firefox, the entire solving process comes off as legitimate to Cloudflare.

## Caveats

- The solver is **not headless** — a GUI is required.
- The method relies on a browser with overrides enabled.
- Tunneling multiple proxies through each iframe is not supported. Do note this may potentially be added in the future if a feasible solution (some form of advanced tunneling) is found. Note that per-window proxying, however, is supported.
- Designed for smaller-scale token harvesting, though the token server architecture does support larger-scale operations.
- Ineffective for general, random web-scraping. Knowing the websites it will be used on is most effective.
- Multi user-agent rotation currently not supported (detected).
- **Requires Firefox for multi-proxy solving. If you want to use a singular IP, then any browser that supports overrides works.**

---

## Components

The bypass is comprised of four main components:

1. **Token Harvester / Turnstile Widget Loader**
2. **Turnstile Widget Identifier & Clicker**
3. **Token Server**
4. **Extensions**

---

### 1. Token Harvester / Turnstile Widget Loader

The Token Harvester loads the Turnstile widget by spawning multiple iframe-based solvers, each pointing at a different Cloudflare site widget. Every solver iframe connects to the token server and forwards any solved tokens to it, and after forwarding a token it also resets the widget and begins solving for another token. Each window will also conect to its respective proxy from the proxy list upon recieving the idx for the proxy from the token server, plus also spoof the user-agent to its respective user-agent based on the recieved idx.

**Setup:**

1. **Configure the files.** The config is in `index.html`:
   - Set `PRELOAD_IFRAMES` (the number of iframe solvers to load on page start) **NOTE: PLEASE KEEP PRELOAD_IFRAMES AT 1. Currently, upon any solve the location reloads. Additionally, multi-iframe solving on a single page has been found to be quite slow. This is all but deprecated as of now but if a feasible solution to tunneling is implemented (as discussed in future plans) it may become useful again.**, `TOKEN_SERVER_HOST` (your token server host, obviously), `PROXY_CONNECT_TIMEOUT` (time for proxy connection to timeout and page to begin reloading), and `USE_PROXY_SOLVING` (boolean to determine if you want to use the multi-proxy solving system). Originally I did just use const SITEKEY which is why that's still declared in the index.html, but after having to change it around consistently it got annoying. So it's set in localStorage now. So set `localStorage.sitekey` (the website's Cloudflare sitekey) in localStorage.
  
   - If you do not know how to access a sitekey, here is a short and easy method you can use to access it: in devtools, find the turnstile.js file in the sources tab. In it, ctrl f "sitekey". You'll see many instances. You can breakpoint a few of these and then run the page to get into the scope, which will have the sitekey. 

2. **Set your proxies.**  Set your linesplit list of proxies to `localStorage.proxies`. The proxy extension will connect to a proxy from this list according to the recieved solver idx. Note the proxies list should include the protocol extension protocol://

3. **Set your user-agents.**  Set your linesplit list of user-agents to `localStorage.user_agents`. The proxy extension will ensure requests per solve are spoofed to a user-agent based on the recieved solver idx. You do not need these, if you don't have enough the system will just keep the user-agent you already have, but for maximum anonymity purposes this is good. **NOTE: currently UAs are detected. Do not use this. User-agents are flagged even after modifying navigator properties and other basic fingerprinting metrics. Even the best user-agent switching extensions fail now. It appears as of 2026 CLoudflare has started matching TLS fingerprinting to UAs, making it difficult to work with. So do not set your user-agent list. This is just here in case a solution to this is presented, and also because this is a PoC and ideally a fully functional spoof would already exist. ALSO NOTE, since this method is already built to be used with legitimate, standard web browsers, this shouldn't pose much of an issue as you'll already be emitting an authentic user-agent.**

4. **Apply as browser overrides.** Replace the target webpage's main HTML file with `index.html`. 

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
| `0` | Incoming token + solver id from a sender. The server routes it to the registered receiver socket with the fewest acquired tokens (based on total acquired, not taking into account tokens that were already consumed). Structure: <0, ...solver_idx_bytes (u32), ...token_bytes>. |
| `1` | Register the sending socket as a receiver and initialize its receiver status. Send this packet when designing a system to actually allow your infrastructure to acquire the tokens. |
| `2` | Request the total token count. The server responds with the current count. |
| `3` | Request the solver_idx. The server responds with this window's solver_idx. Necessary for knowing which proxy solved a challenge in case there are IP checks in place. |

*Clientbound (server → client):*

| Description |
|-------------|
| Incoming token + solver id delivered to a receiver. Structure: <...solver_idx_bytes (u32), ...token_bytes>.|
| Token count response. Sent directly to the requesting client as u64 LE bytes without a header, since that client only needs this single value and no additional packet types are currently required. |
| Solver_idx response. Sent directly as u32 LE bytes to the requesting client. |

*Note these packets do not have headers, as there is only one packet type sent to each endpoint.*

---

### 4. Extensions

Extensions allow us to utilize our browser's full API capability to connect proxies and spoof user-agents (currently detected, so do not use uas), plus block WebRTC.

You'll need two key extensions.

1. As previously mentioned first of all, you'll need FireFox. The architecture for connecting to proxies was designed with FireFox's API, especially since it allows per-window proxy connections. You'll need to install the `firefox-proxy-extension` attached in this repository, as this provides the API necessary for asynchronous proxy connections, allowing you to await and connect to a proxy before continuing execution. Additionally, this extension also spoofs the user-agent field of each solver request, which is also done according to the solver_idx just like the proxy is, so that your proxy can match your custom user-agent.
2. A WebRTC API spoofer or blocker. WebRTC can leak your real IP if not careful, so getting a good extension to block this is critical. You can just look one up online, there are plenty.

---

## Starting It Up

1. Start the **token server**.
2. Start your backend, token managing system. 
3. Start the **auto-clicker**.
4. Open your **modified webpage**.
5. Press **F8** to enable the auto-clicker.
6. Watch it go.

---

## Putting it all Together: the Entire Process

After the user starts up the process, here's what happens:

For each solver tab:

1. The tab first connects to the token server.
2. The tab sends the solver_idx request packet (u8<3>) to the server. 
3. The token server sends back a unique solver_idx, with each request that is handled incrementing the solver_idx value (with modulo across the entire length of the list of course).
4. Once the solver_idx is recieved, the solver tab attempts to use the firefox-proxy-extension to await and asynchronously connect to a proxy corresponding with the recieved solver_idx, and it also ensures all requests going through this proxy will go through a selected user-agent also based on your solver_idx.
5. Once connected, an iframe containing the turnstile widget is loaded. 
6. The tab does the work provided by the widget and completes it.
7. If a checkbox challenge is found, the checkbox clicker clicks the checkbox to complete the challenge.
8. Once the challenge is complete, and the token is recieved, the solver tab sends the result to the server (u8<0, ...solver_idx_bytes, ...token_bytes>). It forwards the solver_idx used and the token. 
9. Upon completion the page reloads--restarting the process. 
10. The token server, which has just recieved the token packet from the solver, forwards it to the reciever with the least tokens. Note, as already mentioned, *you'll* need to set up your reciever architecture. You can connect to the token server and send u8<1> to set up a reciever. From there, you'll of course build the system to do what you actually want with the recieved tokens. 

---

## Future Plans (may not be done, but if major updates do occur to this project it will likely be these).
As previously mentioned, 2026 CF has really amped up their user-agent spoof detection. They now match user-agent reported browser data to even the TLS handshakes you exhibit. A bypass for this is top priority.

An automatic page-loader and harvester setup script may be created in order to aid with multi-proxy solving, as per page loads are currently needed for such.

If a feasible solution is found, a way to tunnel individual iframes (hence enhancing multi-proxy solving outside of just different tabs) may be implemented.

---

## Contributing

---

All contributions are very welcome. If you have a way to improve this project, please share with issues, pull requests, etc.

---

## Some Images and Media of Applications

<img width="1919" height="942" alt="image" src="https://github.com/user-attachments/assets/bd5b88a4-b824-4591-832e-812e254adb68" />
https://github.com/user-attachments/assets/dfca651a-e13c-47f7-8d54-80b029a4983b

<img width="521" height="227" alt="image" src="https://github.com/user-attachments/assets/eeddcd7e-afe5-4084-9f15-40c834cdd2ad" />
Note: no recievers are available here because I didn't connect anything. This is just to show the token harvesting utilization. You set up your architecture with the recievers.
