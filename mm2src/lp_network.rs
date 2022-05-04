/******************************************************************************
 * Copyright © 2014-2019 The SuperNET Developers.                             *
 *                                                                            *
 * See the AUTHORS, DEVELOPER-AGREEMENT and LICENSE files at                  *
 * the top-level directory of this distribution for the individual copyright  *
 * holder information and the developer policies on copyright and licensing.  *
 *                                                                            *
 * Unless otherwise agreed in a custom licensing agreement, no part of the    *
 * SuperNET software, including this file may be copied, modified, propagated *
 * or distributed except according to the terms contained in the LICENSE file *
 *                                                                            *
 * Removal or modification of this copyright notice is prohibited.            *
 *                                                                            *
 ******************************************************************************/
//
//  lp_network.rs
//  marketmaker
//
use common::executor::spawn;
use common::log;
use common::mm_ctx::{MmArc, MmWeak};
use common::mm_error::prelude::*;
use common::mm_metrics::{ClockOps, MetricsOps};
use derive_more::Display;
use futures::{channel::oneshot, lock::Mutex as AsyncMutex, StreamExt};
use mm2_libp2p::atomicdex_behaviour::{AdexBehaviourCmd, AdexBehaviourEvent, AdexCmdTx, AdexEventRx, AdexResponse,
                                      AdexResponseChannel};
use mm2_libp2p::peers_exchange::PeerAddresses;
use mm2_libp2p::{decode_message, encode_message, GossipsubMessage, MessageId, PeerId, TOPIC_SEPARATOR};
#[cfg(test)] use mocktopus::macros::*;
use serde::de;
use std::net::ToSocketAddrs;
use std::sync::Arc;

use crate::mm2::{lp_ordermatch, lp_stats, lp_swap};

pub type P2PRequestResult<T> = Result<T, MmError<P2PRequestError>>;

#[derive(Debug, Display)]
pub enum P2PRequestError {
    EncodeError(String),
    DecodeError(String),
    SendError(String),
    ResponseError(String),
    #[display(fmt = "Expected 1 response, found {}", _0)]
    ExpectedSingleResponseError(usize),
}

impl From<rmp_serde::encode::Error> for P2PRequestError {
    fn from(e: rmp_serde::encode::Error) -> Self { P2PRequestError::EncodeError(e.to_string()) }
}

impl From<rmp_serde::decode::Error> for P2PRequestError {
    fn from(e: rmp_serde::decode::Error) -> Self { P2PRequestError::DecodeError(e.to_string()) }
}

#[derive(Eq, Debug, Deserialize, PartialEq, Serialize)]
pub enum P2PRequest {
    Ordermatch(lp_ordermatch::OrdermatchRequest),
    NetworkInfo(lp_stats::NetworkInfoRequest),
}

pub struct P2PContext {
    /// Using Mutex helps to prevent cloning which can actually result to channel being unbounded in case of using 1 tx clone per 1 message.
    pub cmd_tx: AsyncMutex<AdexCmdTx>,
}

#[cfg_attr(test, mockable)]
impl P2PContext {
    pub fn new(cmd_tx: AdexCmdTx) -> Self {
        P2PContext {
            cmd_tx: AsyncMutex::new(cmd_tx),
        }
    }

    pub fn store_to_mm_arc(self, ctx: &MmArc) { *ctx.p2p_ctx.lock().unwrap() = Some(Arc::new(self)) }

    pub fn fetch_from_mm_arc(ctx: &MmArc) -> Arc<Self> {
        ctx.p2p_ctx
            .lock()
            .unwrap()
            .as_ref()
            .unwrap()
            .clone()
            .downcast()
            .unwrap()
    }
}

pub async fn p2p_event_process_loop(ctx: MmWeak, mut rx: AdexEventRx, i_am_relay: bool) {
    loop {
        let adex_event = rx.next().await;
        let ctx = match MmArc::from_weak(&ctx) {
            Some(ctx) => ctx,
            None => return,
        };
        match adex_event {
            Some(AdexBehaviourEvent::Message(peer_id, message_id, message)) => {
                spawn(process_p2p_message(ctx, peer_id, message_id, message, i_am_relay));
            },
            Some(AdexBehaviourEvent::PeerRequest {
                peer_id,
                request,
                response_channel,
            }) => {
                if let Err(e) = process_p2p_request(ctx, peer_id, request, response_channel).await {
                    log::error!("Error on process P2P request: {:?}", e);
                }
            },
            None => break,
            _ => (),
        }
    }
}

async fn process_p2p_message(
    ctx: MmArc,
    peer_id: PeerId,
    message_id: MessageId,
    message: GossipsubMessage,
    i_am_relay: bool,
) {
    let mut to_propagate = false;
    let mut orderbook_pairs = vec![];

    for topic in message.topics {
        let mut split = topic.as_str().split(TOPIC_SEPARATOR);
        match split.next() {
            Some(lp_ordermatch::ORDERBOOK_PREFIX) => {
                if let Some(pair) = split.next() {
                    orderbook_pairs.push(pair.to_string());
                }
            },
            Some(lp_swap::SWAP_PREFIX) => {
                lp_swap::process_msg(ctx.clone(), split.next().unwrap_or_default(), &message.data);
                to_propagate = true;
            },
            None | Some(_) => (),
        }
    }

    if !orderbook_pairs.is_empty() {
        let process_fut = lp_ordermatch::process_msg(
            ctx.clone(),
            orderbook_pairs,
            peer_id.to_string(),
            &message.data,
            i_am_relay,
        );
        if process_fut.await {
            to_propagate = true;
        }
    }

    if to_propagate && i_am_relay {
        propagate_message(&ctx, message_id, peer_id);
    }
}

async fn process_p2p_request(
    ctx: MmArc,
    _peer_id: PeerId,
    request: Vec<u8>,
    response_channel: AdexResponseChannel,
) -> P2PRequestResult<()> {
    let request = decode_message::<P2PRequest>(&request)?;
    let result = match request {
        P2PRequest::Ordermatch(req) => lp_ordermatch::process_peer_request(ctx.clone(), req).await,
        P2PRequest::NetworkInfo(req) => lp_stats::process_info_request(ctx.clone(), req).await,
    };

    let res = match result {
        Ok(Some(response)) => AdexResponse::Ok { response },
        Ok(None) => AdexResponse::None,
        Err(e) => AdexResponse::Err { error: e },
    };

    let p2p_ctx = P2PContext::fetch_from_mm_arc(&ctx);
    let cmd = AdexBehaviourCmd::SendResponse { res, response_channel };
    p2p_ctx
        .cmd_tx
        .lock()
        .await
        .try_send(cmd)
        .map_to_mm(|e| P2PRequestError::SendError(e.to_string()))?;
    Ok(())
}

pub fn broadcast_p2p_msg(ctx: &MmArc, topics: Vec<String>, msg: Vec<u8>) {
    let ctx = ctx.clone();
    spawn(async move {
        let cmd = AdexBehaviourCmd::PublishMsg { topics, msg };
        let p2p_ctx = P2PContext::fetch_from_mm_arc(&ctx);
        if let Err(e) = p2p_ctx.cmd_tx.lock().await.try_send(cmd) {
            log::error!("broadcast_p2p_msg cmd_tx.send error {:?}", e);
        };
    });
}

/// Subscribe to the given `topic`.
///
/// # Safety
///
/// The function locks the [`MmCtx::p2p_ctx`] mutext.
pub async fn subscribe_to_topic(ctx: &MmArc, topic: String) {
    let p2p_ctx = P2PContext::fetch_from_mm_arc(ctx);
    let cmd = AdexBehaviourCmd::Subscribe { topic };
    if let Err(e) = p2p_ctx.cmd_tx.lock().await.try_send(cmd) {
        log::error!("subscribe_to_topic cmd_tx.send error {:?}", e);
    };
}

pub async fn request_any_relay<T: de::DeserializeOwned>(
    ctx: MmArc,
    req: P2PRequest,
) -> P2PRequestResult<Option<(T, PeerId)>> {
    let encoded = encode_message(&req)?;

    let (response_tx, response_rx) = oneshot::channel();
    let p2p_ctx = P2PContext::fetch_from_mm_arc(&ctx);
    let cmd = AdexBehaviourCmd::RequestAnyRelay {
        req: encoded,
        response_tx,
    };
    p2p_ctx
        .cmd_tx
        .lock()
        .await
        .try_send(cmd)
        .map_to_mm(|e| P2PRequestError::SendError(e.to_string()))?;
    match response_rx
        .await
        .map_to_mm(|e| P2PRequestError::ResponseError(e.to_string()))?
    {
        Some((from_peer, response)) => {
            let response = decode_message::<T>(&response)?;
            Ok(Some((response, from_peer)))
        },
        None => Ok(None),
    }
}

pub enum PeerDecodedResponse<T> {
    Ok(T),
    None,
    Err(String),
}

#[allow(dead_code)]
pub async fn request_relays<T: de::DeserializeOwned>(
    ctx: MmArc,
    req: P2PRequest,
) -> P2PRequestResult<Vec<(PeerId, PeerDecodedResponse<T>)>> {
    let encoded = encode_message(&req)?;

    let (response_tx, response_rx) = oneshot::channel();
    let p2p_ctx = P2PContext::fetch_from_mm_arc(&ctx);
    let cmd = AdexBehaviourCmd::RequestRelays {
        req: encoded,
        response_tx,
    };
    p2p_ctx
        .cmd_tx
        .lock()
        .await
        .try_send(cmd)
        .map_to_mm(|e| P2PRequestError::SendError(e.to_string()))?;
    let responses = response_rx
        .await
        .map_to_mm(|e| P2PRequestError::ResponseError(e.to_string()))?;
    Ok(parse_peers_responses(responses))
}

pub async fn request_peers<T: de::DeserializeOwned>(
    ctx: MmArc,
    req: P2PRequest,
    peers: Vec<String>,
) -> P2PRequestResult<Vec<(PeerId, PeerDecodedResponse<T>)>> {
    let encoded = encode_message(&req)?;

    let (response_tx, response_rx) = oneshot::channel();
    let p2p_ctx = P2PContext::fetch_from_mm_arc(&ctx);
    let cmd = AdexBehaviourCmd::RequestPeers {
        req: encoded,
        peers,
        response_tx,
    };
    p2p_ctx
        .cmd_tx
        .lock()
        .await
        .try_send(cmd)
        .map_to_mm(|e| P2PRequestError::SendError(e.to_string()))?;
    let responses = response_rx
        .await
        .map_to_mm(|e| P2PRequestError::ResponseError(e.to_string()))?;
    Ok(parse_peers_responses(responses))
}

pub async fn request_one_peer<T: de::DeserializeOwned>(
    ctx: MmArc,
    req: P2PRequest,
    peer: String,
) -> P2PRequestResult<Option<T>> {
    let clock = ctx.metrics.clock().expect("Metrics clock is not available");
    let start = clock.now();
    let mut responses = request_peers::<T>(ctx.clone(), req, vec![peer.clone()]).await?;
    let end = clock.now();
    mm_timing!(ctx.metrics, "peer.outgoing_request.timing", start, end, "peer" => peer);
    if responses.len() != 1 {
        return MmError::err(P2PRequestError::ExpectedSingleResponseError(responses.len()));
    }
    let (_, response) = responses.remove(0);
    match response {
        PeerDecodedResponse::Ok(response) => Ok(Some(response)),
        PeerDecodedResponse::None => Ok(None),
        PeerDecodedResponse::Err(e) => MmError::err(P2PRequestError::ResponseError(e)),
    }
}

fn parse_peers_responses<T: de::DeserializeOwned>(
    responses: Vec<(PeerId, AdexResponse)>,
) -> Vec<(PeerId, PeerDecodedResponse<T>)> {
    responses
        .into_iter()
        .map(|(peer_id, res)| {
            let res = match res {
                AdexResponse::Ok { response } => match decode_message::<T>(&response) {
                    Ok(res) => PeerDecodedResponse::Ok(res),
                    Err(e) => PeerDecodedResponse::Err(ERRL!("{}", e)),
                },
                AdexResponse::None => PeerDecodedResponse::None,
                AdexResponse::Err { error } => PeerDecodedResponse::Err(error),
            };
            (peer_id, res)
        })
        .collect()
}

pub fn propagate_message(ctx: &MmArc, message_id: MessageId, propagation_source: PeerId) {
    let ctx = ctx.clone();
    spawn(async move {
        let p2p_ctx = P2PContext::fetch_from_mm_arc(&ctx);
        let cmd = AdexBehaviourCmd::PropagateMessage {
            message_id,
            propagation_source,
        };
        if let Err(e) = p2p_ctx.cmd_tx.lock().await.try_send(cmd) {
            log::error!("propagate_message cmd_tx.send error {:?}", e);
        };
    });
}

pub fn add_reserved_peer_addresses(ctx: &MmArc, peer: PeerId, addresses: PeerAddresses) {
    let ctx = ctx.clone();
    spawn(async move {
        let p2p_ctx = P2PContext::fetch_from_mm_arc(&ctx);
        let cmd = AdexBehaviourCmd::AddReservedPeer { peer, addresses };
        if let Err(e) = p2p_ctx.cmd_tx.lock().await.try_send(cmd) {
            log::error!("add_reserved_peer_addresses cmd_tx.send error {:?}", e);
        };
    });
}

#[derive(Debug, Display)]
pub enum ParseAddressError {
    #[display(fmt = "Address/Seed {} resolved to IPv6 which is not supported", _0)]
    UnsupportedIPv6Address(String),
    #[display(fmt = "Address/Seed {} to_socket_addrs empty iter", _0)]
    EmptyIterator(String),
    #[display(fmt = "Couldn't resolve '{}' Address/Seed: {}", _0, _1)]
    UnresolvedAddress(String, String),
}

#[cfg(not(target_arch = "wasm32"))]
pub fn addr_to_ipv4_string(address: &str) -> Result<String, MmError<ParseAddressError>> {
    // Remove "https:// or http://" etc.. from address str
    let formated_address = address.split("://").last().unwrap_or(address);
    let address_with_port = if formated_address.contains(':') {
        formated_address.to_string()
    } else {
        format!("{}:0", formated_address)
    };
    match address_with_port.as_str().to_socket_addrs() {
        Ok(mut iter) => match iter.next() {
            Some(addr) => {
                if addr.is_ipv4() {
                    Ok(addr.ip().to_string())
                } else {
                    log::warn!(
                        "Address/Seed {} resolved to IPv6 {} which is not supported",
                        address,
                        addr
                    );
                    MmError::err(ParseAddressError::UnsupportedIPv6Address(address.into()))
                }
            },
            None => {
                log::warn!("Address/Seed {} to_socket_addrs empty iter", address);
                MmError::err(ParseAddressError::EmptyIterator(address.into()))
            },
        },
        Err(e) => {
            log::error!("Couldn't resolve '{}' seed: {}", address, e);
            MmError::err(ParseAddressError::UnresolvedAddress(address.into(), e.to_string()))
        },
    }
}

#[derive(Debug, Display)]
pub enum NetIdError {
    #[display(fmt = "Netid {} is larger than max {}", netid, max_netid)]
    LargerThanMax { netid: u16, max_netid: u16 },
}

pub fn lp_ports(netid: u16) -> Result<(u16, u16, u16), MmError<NetIdError>> {
    const LP_RPCPORT: u16 = 7783;
    let max_netid = (65535 - 40 - LP_RPCPORT) / 4;
    if netid > max_netid {
        return MmError::err(NetIdError::LargerThanMax { netid, max_netid });
    }

    let other_ports = if netid != 0 {
        let net_mod = netid % 10;
        let net_div = netid / 10;
        (net_div * 40) + LP_RPCPORT + net_mod
    } else {
        LP_RPCPORT
    };
    Ok((other_ports + 10, other_ports + 20, other_ports + 30))
}
