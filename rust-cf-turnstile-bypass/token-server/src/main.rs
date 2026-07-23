// Token server to recieve, route tokens, and output total acquired tokens.

use futures_util::{SinkExt, StreamExt};
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, Mutex};
use tokio_tungstenite::{accept_async, tungstenite::Message};

const PORT: u16 = 8080;

type Tx = mpsc::UnboundedSender<Message>;

#[derive(Debug, Default)]
struct State {
    // Active connections.
    connections: HashMap<u32, Tx>,
    // Available solvers (not currently solving). 
    // Note this isn't actually a generic queue structure, 
    // there is no specific ordered pick from the HashSet.
    // This doesn't matter for our case as we just want any available solver.
    available_solvers_queue: HashSet<u32>,
}

static NEXT_ID: AtomicU32 = AtomicU32::new(0);

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

    // Assign socket id and push socket data to the sockets HashMap.
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    
    let (mut ws_tx, mut ws_rx) = ws_stream.split();
    let (tx, mut rx) = mpsc::unbounded_channel::<Message>();
    {
        let mut s = state.lock().await;
        s.connections.insert(id, tx.clone());
    }

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

        let header = raw[0];

        match header {
            // Token result from solver. This token is recieved, 
            // and forwarded to the requester (reciever) with the associated requester_id.
            // [0, ...requester_id_bytes, ...solver_idx_bytes ...token_bytes]
            // If the solve failed, there will be no token bytes in this packet.
            0 => {
                let mut requester_id_bytes = [0u8; 4];
                requester_id_bytes.copy_from_slice(&raw[1..5]);
                let requester_id = u32::from_le_bytes(requester_id_bytes);

                let mut s = state.lock().await;

                // Route the token back to the specific requester who asked for it by looking up its requester id.
                if let Some(requester_tx) = s.connections.get(&requester_id) {
                    // Forward the token back to the reciever.
                    // [...solver_idx_bytes, ...token_bytes]
                    let mut token_packet = Vec::new();
                    token_packet.extend_from_slice(&raw[5..9]);
                    
                    // If the solve failed, there will be no token bytes in the packet,
                    // and thus we send no token bytes to the reciever.
                    if raw.len() > 9 {
                        token_packet.extend_from_slice(&raw[9..]);
                    }

                    let _ = requester_tx.send(Message::Binary(token_packet));
                    println!("[+] Routed token back to requester ID: {}.", requester_id);
                } else {
                    println!("[-] Requester ID {} is no longer connected.", requester_id);
                }

                // The solver is now finished. Re-add it to the queue since it is available now.
                s.available_solvers_queue.insert(id);
                println!("[+] Solver {} re-added to queue. Total available: {}.", id, s.available_solvers_queue.len());
            }

            // On demand solve request from a requester.
            // This will forward our request for a solve to the next available solver in queue.
            // [1, ...solver_idx_bytes]
            1 => {
                let mut s = state.lock().await;
                
                let solver_id_opt = s.available_solvers_queue.iter().next().copied();

                if let Some(solver_id) = solver_id_opt {
                    // Remove this solver now as it is occupied.
                    s.available_solvers_queue.remove(&solver_id);

                    if let Some(solver_tx) = s.connections.get(&solver_id) {
                        // [...solver_idx_bytes, ...requester_id_bytes, ...(field_name_len, ...field_name_bytes, field_value_len, ...field_value_bytes)]
                        let mut forward_packet = Vec::new();
                        forward_packet.extend_from_slice(&raw[1..5]);
                        forward_packet.extend_from_slice(&id.to_le_bytes());
                        
                        if raw.len() > 5 {
                            forward_packet.extend_from_slice(&raw[5..]);
                        }

                        let _ = solver_tx.send(Message::Binary(forward_packet));
                        println!("[+] Forwarded on-demand request from {} to solver {}.", id, solver_id);
                    }
                } else {
                    // Indicate that this solver request couldn't go through.
                    // [0]
                     let _ = tx.send(Message::binary(vec![0]));
                    println!("[-] No solvers available in the queue to handle request from {}.", id);
                }
            }

            // Register this socket as a solver, and append its id to the available_solvers_queue.
            // [2]
            2 => {
                let mut s = state.lock().await;
                s.available_solvers_queue.insert(id);
                println!("[+] Solver {} added to queue. Total available: {}.", id, s.available_solvers_queue.len());
            }

            _ => {
                eprintln!("Unknown header byte: {} from socket {}.", header, id);
            }
        }
    }

    // Disconnect socket and clear data.
    let mut s = state.lock().await;
    s.connections.remove(&id);
    s.available_solvers_queue.remove(&id);
    println!("[-] Socket {} disconnected.", id);
}
