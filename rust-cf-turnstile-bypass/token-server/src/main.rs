// Token server to recieve, route tokens, and output total acquired tokens.

use futures_util::{SinkExt, StreamExt};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, AtomicU32, Ordering};
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, Mutex};
use tokio_tungstenite::{accept_async, tungstenite::Message};

const PORT: u16 = 8080;
const PROXIES_LIST_LENGTH: u32 = 187;

type Tx = mpsc::UnboundedSender<Message>;

#[derive(Debug)]
struct ReceiverEntry {
    tx: Tx,
    acquired_tokens: usize
}

#[derive(Debug, Default)]
struct State {
    receivers: HashMap<u64, ReceiverEntry>
}

static NEXT_ID: AtomicU64 = AtomicU64::new(0);
static TOTAL_TOKENS_COUNT: AtomicU64 = AtomicU64::new(0);
static SOLVER_IDX: AtomicU32 = AtomicU32::new(0);

#[tokio::main]
async fn main() {
    let addr = format!("0.0.0.0:{}", PORT);
    let listener = TcpListener::bind(&addr).await.expect("Failed to bind");
    println!("WebSocket server listening on ws://localhost:{}", PORT);

    let state: Arc<Mutex<State>> = Arc::new(Mutex::new(State::default()));

    while let Ok((stream, _)) = listener.accept().await {
        let state = Arc::clone(&state);
        tokio::spawn(handle_connection(stream, state));
    }
}

async fn handle_connection(stream: TcpStream, state: Arc<Mutex<State>>) {
    let ws_stream = match accept_async(stream).await {
        Ok(ws) => ws,
        Err(e) => {
            eprintln!("[-] WebSocket handshake failed: {}", e);
            return;
        }
    };

    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let (mut ws_tx, mut ws_rx) = ws_stream.split();

    let (tx, mut rx) = mpsc::unbounded_channel::<Message>();

    tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if ws_tx.send(msg).await.is_err() {
                break;
            }
        }
    });

    while let Some(result) = ws_rx.next().await {
        let raw = match result {
            Ok(Message::Binary(data)) => data,
            Ok(Message::Close(_)) | Err(_) => break,
            _ => continue,
        };

        if raw.is_empty() {
            continue;
        }

        let header = raw[0];

        match header {
            // Incoming token from a sender, which we route to the receiver with least amount of acquired tokens.
            // [0, ...solver_idx_bytes, ...token_bytes]
            0 => {
                TOTAL_TOKENS_COUNT.fetch_add(1, Ordering::Relaxed);

                let solver_idx_and_token_bytes = &raw[1..];

                // [...solver_idx_bytes, ...token_bytes].
                let mut token_packet = Vec::with_capacity(solver_idx_and_token_bytes.len());
                token_packet.extend_from_slice(solver_idx_and_token_bytes);

                let mut s = state.lock().await;

                // Find the receiver with the least acquired tokens.
                let best = s.receivers.iter().min_by_key(|(_, entry)| entry.acquired_tokens).map(|(k, _)| *k);

                if let Some(recv_id) = best {
                    println!("sending new token to socket with ID: {}", recv_id);
                    if let Some(entry) = s.receivers.get_mut(&recv_id) {
                        let _ = entry.tx.send(Message::Binary(token_packet));
                        entry.acquired_tokens += 1;
                    }
                } else {
                    eprintln!("[-] No receivers available to route token to");
                }
            }

            // Register this socket as a receiver.
            // [1]
            1 => {
                let mut s = state.lock().await;
                s.receivers.insert(id, ReceiverEntry { tx: tx.clone(), acquired_tokens: 0 });
                println!("[+] Set new receiver. Total recievers: {}", s.receivers.len());
            }

            // Token count request. Send total acquired tokens back to requester.
            // [2]
            2 => {
                let mut total_tokens_packet = Vec::new();
                total_tokens_packet.extend_from_slice(&TOTAL_TOKENS_COUNT.load(Ordering::Relaxed).to_le_bytes());
                let _ = tx.send(Message::binary(total_tokens_packet));
                println!("[+] Total tokens count request packet recieved. Sent back the total tokens count.");
            }

            // Solver idx request packet. Used by solvers to know which proxy index they're working with. Also increments and modulos the idx for next time it is accessed.
            // [3]
            3 => {
                let mut solver_idx_packet = Vec::new();
                let solver_idx = SOLVER_IDX.fetch_add(1, Ordering::Relaxed) % PROXIES_LIST_LENGTH;
                solver_idx_packet.extend_from_slice(&solver_idx.to_le_bytes());
                let _ = tx.send(Message::binary(solver_idx_packet));
                println!("[+] Solver idx request packet recieved. Sending back solver idx: {}.", solver_idx);
            }

            _ => {
                eprintln!("Unknown header byte: {}", header);
            }
        }
    }

    let mut s = state.lock().await;
    s.receivers.remove(&id);
}
