use super::{
    super::{
        Exchange, Kline, MarketKind, OpenInterest, Price, PushFrequency, SizeUnit, StreamKind,
        Ticker, TickerInfo, TickerStats, Timeframe, Trade,
        adapter::StreamTicksize,
        connect::{State, connect_ws},
        de_string_to_f32,
        depth::{DeOrder, DepthPayload, DepthUpdate, LocalDepthCache},
        is_symbol_supported,
        limiter::{self, RateLimiter},
        volume_size_unit,
    },
    AdapterError, Event,
};

use fastwebsockets::OpCode;
use iced_futures::{
    futures::{SinkExt, Stream, channel::mpsc},
    stream,
};
use serde::Deserialize;
use sonic_rs::{FastStr, to_object_iter_unchecked};
use tokio::sync::Mutex;

use std::{collections::HashMap, sync::LazyLock, time::Duration};

// Switch to Binance Futures Domain for better liquidity and data
const LINEAR_PERP_Domain: &str = "https://fapi.binance.com";
const WS_DOMAIN: &str = "fstream.binance.com";

static FOREX_LIMITER: LazyLock<Mutex<ForexLimiter>> =
    LazyLock::new(|| Mutex::new(ForexLimiter::new(LIMIT, REFILL_RATE)));

const LIMIT: usize = 2400; // Futures API limit is 2400/min
const REFILL_RATE: Duration = Duration::from_secs(60);
const LIMITER_BUFFER_PCT: f32 = 0.03;

pub struct ForexLimiter {
    bucket: limiter::DynamicBucket,
}

impl ForexLimiter {
    pub fn new(limit: usize, refill_rate: Duration) -> Self {
        let effective_limit = (limit as f32 * (1.0 - LIMITER_BUFFER_PCT)) as usize;
        ForexLimiter {
            bucket: limiter::DynamicBucket::new(effective_limit, refill_rate),
        }
    }
}

impl RateLimiter for ForexLimiter {
    fn prepare_request(&mut self, weight: usize) -> Option<Duration> {
        let (wait_time, _reason) = self.bucket.prepare_request(weight);
        wait_time
    }

    fn update_from_response(&mut self, response: &reqwest::Response, _weight: usize) {
        if let Some(header_value) = response
            .headers()
            .get("x-mbx-used-weight-1m")
            .and_then(|h| h.to_str().ok())
            .and_then(|s| s.parse::<usize>().ok())
        {
            self.bucket.update_weight(header_value);
        }
    }

    fn should_exit_on_response(&self, response: &reqwest::Response) -> bool {
        let status = response.status();
        status == 429 || status == 418
    }
}

fn exchange_from_market_type(_market: MarketKind) -> Exchange {
    Exchange::Forex
}

fn limiter_from_market_type(_market: MarketKind) -> &'static Mutex<ForexLimiter> {
    &FOREX_LIMITER
}

fn ws_domain_from_market_type(_market: MarketKind) -> &'static str {
    WS_DOMAIN
}

#[derive(Deserialize, Clone)]
pub struct FetchedPerpDepth {
    #[serde(rename = "lastUpdateId")]
    update_id: u64,
    #[serde(rename = "T")]
    time: u64,
    #[serde(rename = "bids")]
    bids: Vec<DeOrder>,
    #[serde(rename = "asks")]
    asks: Vec<DeOrder>,
}

#[derive(Deserialize, Debug, Clone)]
struct SonicKline {
    #[serde(rename = "t")]
    time: u64,
    #[serde(rename = "o", deserialize_with = "de_string_to_f32")]
    open: f32,
    #[serde(rename = "h", deserialize_with = "de_string_to_f32")]
    high: f32,
    #[serde(rename = "l", deserialize_with = "de_string_to_f32")]
    low: f32,
    #[serde(rename = "c", deserialize_with = "de_string_to_f32")]
    close: f32,
    #[serde(rename = "v", deserialize_with = "de_string_to_f32")]
    volume: f32,
    #[serde(rename = "V", deserialize_with = "de_string_to_f32")]
    taker_buy_base_asset_volume: f32,
    #[serde(rename = "i")]
    interval: String,
}

#[derive(Deserialize, Debug, Clone)]
struct SonicKlineWrap {
    #[serde(rename = "s")]
    symbol: String,
    #[serde(rename = "k")]
    kline: SonicKline,
}

#[derive(Deserialize, Debug)]
struct SonicTrade {
    #[serde(rename = "T")]
    time: u64,
    #[serde(rename = "p", deserialize_with = "de_string_to_f32")]
    price: f32,
    #[serde(rename = "q", deserialize_with = "de_string_to_f32")]
    qty: f32,
    #[serde(rename = "m")]
    is_sell: bool,
}

enum SonicDepth {
    Perp(PerpDepth),
}

#[derive(Deserialize)]
struct PerpDepth {
    #[serde(rename = "T")]
    time: u64,
    #[serde(rename = "U")]
    first_id: u64,
    #[serde(rename = "u")]
    final_id: u64,
    #[serde(rename = "pu")]
    prev_final_id: u64,
    #[serde(rename = "b")]
    bids: Vec<DeOrder>,
    #[serde(rename = "a")]
    asks: Vec<DeOrder>,
}

enum StreamData {
    Trade(SonicTrade),
    Depth(SonicDepth),
    Kline(Ticker, SonicKline),
}

enum StreamWrapper {
    Trade,
    Depth,
    Kline,
}

impl StreamWrapper {
    fn from_stream_type(stream_type: &FastStr) -> Option<Self> {
        stream_type
            .split('@')
            .nth(1)
            .and_then(|after_at| match after_at {
                s if s.starts_with("de") => Some(StreamWrapper::Depth),
                s if s.starts_with("ag") => Some(StreamWrapper::Trade),
                s if s.starts_with("kl") => Some(StreamWrapper::Kline),
                _ => None,
            })
    }
}

fn feed_de(slice: &[u8], market: MarketKind) -> Result<StreamData, AdapterError> {
    let exchange = exchange_from_market_type(market);

    let mut stream_type: Option<StreamWrapper> = None;
    let iter: sonic_rs::ObjectJsonIter = unsafe { to_object_iter_unchecked(slice) };

    for elem in iter {
        let (k, v) = elem.map_err(|e| AdapterError::ParseError(e.to_string()))?;

        if k == "stream" {
            if let Some(s) = StreamWrapper::from_stream_type(&v.as_raw_faststr()) {
                stream_type = Some(s);
            }
        } else if k == "data" {
            match stream_type {
                Some(StreamWrapper::Trade) => {
                    let trade: SonicTrade = sonic_rs::from_str(&v.as_raw_faststr())
                        .map_err(|e| AdapterError::ParseError(e.to_string()))?;

                    return Ok(StreamData::Trade(trade));
                }
                Some(StreamWrapper::Depth) => {
                    let depth: PerpDepth = sonic_rs::from_str(&v.as_raw_faststr())
                        .map_err(|e| AdapterError::ParseError(e.to_string()))?;

                    return Ok(StreamData::Depth(SonicDepth::Perp(depth)));
                }
                Some(StreamWrapper::Kline) => {
                    let kline_wrap: SonicKlineWrap = sonic_rs::from_str(&v.as_raw_faststr())
                        .map_err(|e| AdapterError::ParseError(e.to_string()))?;

                    return Ok(StreamData::Kline(
                        Ticker::new(&kline_wrap.symbol, exchange),
                        kline_wrap.kline,
                    ));
                }
                _ => {
                    log::error!("Unknown stream type");
                }
            }
        } else {
            log::error!("Unknown data: {:?}", k);
        }
    }

    Err(AdapterError::ParseError(
        "Failed to parse ws data".to_string(),
    ))
}

async fn try_resync(
    exchange: Exchange,
    ticker_info: TickerInfo,
    contract_size: Option<f32>,
    orderbook: &mut LocalDepthCache,
    state: &mut State,
    output: &mut mpsc::Sender<Event>,
    already_fetching: &mut bool,
) {
    let ticker = ticker_info.ticker;

    let (tx, rx) = tokio::sync::oneshot::channel();
    *already_fetching = true;

    tokio::spawn(async move {
        let result = fetch_depth(&ticker, contract_size).await;
        let _ = tx.send(result);
    });

    match rx.await {
        Ok(Ok(depth)) => {
            orderbook.update(DepthUpdate::Snapshot(depth), ticker_info.min_ticksize);
        }
        Ok(Err(e)) => {
            let _ = output
                .send(Event::Disconnected(
                    exchange,
                    format!("Depth fetch failed: {e}"),
                ))
                .await;
        }
        Err(e) => {
            *state = State::Disconnected;

            output
                .send(Event::Disconnected(
                    exchange,
                    format!("Failed to send fetched depth for {ticker}, error: {e}"),
                ))
                .await
                .expect("Trying to send disconnect event...");
        }
    }
    *already_fetching = false;
}

#[allow(unused_assignments)]
pub fn connect_market_stream(
    ticker_info: TickerInfo,
    push_freq: PushFrequency,
) -> impl Stream<Item = Event> {
    stream::channel(100, async move |mut output| {
        let mut state = State::Disconnected;

        let ticker = ticker_info.ticker;

        let (symbol_str, market) = ticker.to_full_symbol_and_type();
        let exchange = exchange_from_market_type(market);

        let mut orderbook: LocalDepthCache = LocalDepthCache::default();
        let mut trades_buffer: Vec<Trade> = Vec::new();
        let mut already_fetching: bool = false;
        let mut prev_id: u64 = 0;

        let contract_size = None; 
        let size_in_quote_ccy = volume_size_unit() == SizeUnit::Quote;

        loop {
            match &mut state {
                State::Disconnected => {
                    let stream_1 = format!("{}@aggTrade", symbol_str.to_lowercase());
                    let stream_2 = format!("{}@depth@100ms", symbol_str.to_lowercase());

                    let domain = ws_domain_from_market_type(market);
                    let streams = format!("{stream_1}/{stream_2}");
                    let url = format!("wss://{domain}/stream?streams={streams}");

                    if let Ok(websocket) = connect_ws(domain, &url).await {
                        let (tx, rx) = tokio::sync::oneshot::channel();

                        tokio::spawn(async move {
                            let result = fetch_depth(&ticker, contract_size).await;
                            let _ = tx.send(result);
                        });
                        match rx.await {
                            Ok(Ok(depth)) => {
                                orderbook
                                    .update(DepthUpdate::Snapshot(depth), ticker_info.min_ticksize);
                                prev_id = 0;

                                state = State::Connected(websocket);

                                let _ = output.send(Event::Connected(exchange)).await;
                            }
                            Ok(Err(e)) => {
                                let _ = output
                                    .send(Event::Disconnected(
                                        exchange,
                                        format!("Depth fetch failed: {e}"),
                                    ))
                                    .await;
                            }
                            Err(e) => {
                                let _ = output
                                    .send(Event::Disconnected(
                                        exchange,
                                        format!("Channel error: {e}"),
                                    ))
                                    .await;
                            }
                        }
                    } else {
                        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

                        let _ = output
                            .send(Event::Disconnected(
                                exchange,
                                "Failed to connect to websocket".to_string(),
                            ))
                            .await;
                    }
                }
                State::Connected(ws) => {
                    match ws.read_frame().await {
                        Ok(msg) => match msg.opcode {
                            OpCode::Text => {
                                if let Ok(data) = feed_de(&msg.payload[..], market) {
                                    match data {
                                        StreamData::Trade(de_trade) => {
                                            let price = Price::from_f32(de_trade.price)
                                                .round_to_min_tick(ticker_info.min_ticksize);
                                            // For Perp volume, Qty is usually in base?
                                            // Binance Linear Futures: qty is in base asset (e.g. BTC)
                                            // So logic is similar.
                                            let qty = if size_in_quote_ccy {
                                                    (de_trade.qty * de_trade.price).round()
                                                } else {
                                                    de_trade.qty
                                                };

                                            let trade = Trade {
                                                time: de_trade.time,
                                                is_sell: de_trade.is_sell,
                                                price,
                                                qty,
                                            };

                                            trades_buffer.push(trade);
                                        }
                                        StreamData::Depth(depth_type) => {
                                            if already_fetching {
                                                continue;
                                            }

                                            let last_update_id = orderbook.last_update_id;

                                            match depth_type {
                                                SonicDepth::Perp(ref de_depth) => {
                                                    if (de_depth.final_id <= last_update_id)
                                                        || last_update_id == 0
                                                    {
                                                        continue;
                                                    }

                                                    if prev_id == 0
                                                        && (de_depth.first_id > last_update_id + 1)
                                                        || (last_update_id + 1 > de_depth.final_id)
                                                    {
                                                        log::warn!(
                                                            "Out of sync at first event. Trying to resync...\n"
                                                        );

                                                        try_resync(
                                                            exchange,
                                                            ticker_info,
                                                            contract_size,
                                                            &mut orderbook,
                                                            &mut state,
                                                            &mut output,
                                                            &mut already_fetching,
                                                        )
                                                        .await;
                                                    }

                                                    if (prev_id == 0)
                                                        || (prev_id == de_depth.prev_final_id)
                                                    {
                                                        orderbook.update(
                                                            DepthUpdate::Diff(new_depth_cache(
                                                                &depth_type,
                                                                contract_size,
                                                            )),
                                                            ticker_info.min_ticksize,
                                                        );

                                                        let _ = output
                                                            .send(Event::DepthReceived(
                                                                StreamKind::DepthAndTrades {
                                                                    ticker_info,
                                                                    depth_aggr:
                                                                        StreamTicksize::Client,
                                                                    push_freq,
                                                                },
                                                                de_depth.time,
                                                                orderbook.depth.clone(),
                                                                std::mem::take(&mut trades_buffer)
                                                                    .into_boxed_slice(),
                                                            ))
                                                            .await;

                                                        prev_id = de_depth.final_id;
                                                    } else {
                                                        state = State::Disconnected;
                                                        let _ = output.send(
                                                                Event::Disconnected(
                                                                    exchange,
                                                                    format!("Out of sync. Expected update_id: {}, got: {}", de_depth.prev_final_id, prev_id)
                                                                )
                                                            ).await;
                                                    }
                                                }
                                            }
                                        }
                                        _ => {}
                                    }
                                }
                            }
                            OpCode::Close => {
                                state = State::Disconnected;
                                let _ = output
                                    .send(Event::Disconnected(
                                        exchange,
                                        "Connection closed".to_string(),
                                    ))
                                    .await;
                            }
                            _ => {}
                        },
                        Err(e) => {
                            state = State::Disconnected;
                            let _ = output
                                .send(Event::Disconnected(
                                    exchange,
                                    "Error reading frame: ".to_string() + &e.to_string(),
                                ))
                                .await;
                        }
                    };
                }
            }
        }
    })
}

fn new_depth_cache(depth: &SonicDepth, contract_size: Option<f32>) -> DepthPayload {
    let (time, final_id, bids, asks) = match depth {
        SonicDepth::Perp(de) => (de.time, de.final_id, &de.bids, &de.asks),
    };

    let size_in_quote_ccy = volume_size_unit() == SizeUnit::Quote;

    DepthPayload {
        last_update_id: final_id,
        time,
        bids: bids
            .iter()
            .map(|x| DeOrder {
                price: x.price,
                qty: calc_qty(x.qty, x.price, contract_size, size_in_quote_ccy),
            })
            .collect(),
        asks: asks
            .iter()
            .map(|x| DeOrder {
                price: x.price,
                qty: calc_qty(x.qty, x.price, contract_size, size_in_quote_ccy),
            })
            .collect(),
    }
}

async fn fetch_depth(
    ticker: &Ticker,
    contract_size: Option<f32>,
) -> Result<DepthPayload, AdapterError> {
    let (symbol_str, _market_type) = ticker.to_full_symbol_and_type();

    let base_url = LINEAR_PERP_Domain.to_string() + "/fapi/v1/depth";

    let depth_limit = 1000;

    let url = format!(
        "{}?symbol={}&limit={}",
        base_url,
        symbol_str.to_uppercase(),
        depth_limit
    );

    let _weight = 20; 

    let limiter = &FOREX_LIMITER;
    let text = crate::limiter::http_request_with_limiter(&url, limiter, _weight, None, None).await?;

    let size_in_quote_ccy = volume_size_unit() == SizeUnit::Quote;

    let fetched_depth: FetchedPerpDepth =
        serde_json::from_str(&text).map_err(|e| AdapterError::ParseError(e.to_string()))?;

    let depth = DepthPayload {
        last_update_id: fetched_depth.update_id,
        time: fetched_depth.time,
        bids: fetched_depth
            .bids
            .iter()
            .map(|x| DeOrder {
                price: x.price,
                qty: calc_qty(x.qty, x.price, contract_size, size_in_quote_ccy),
            })
            .collect(),
        asks: fetched_depth
            .asks
            .iter()
            .map(|x| DeOrder {
                price: x.price,
                qty: calc_qty(x.qty, x.price, contract_size, size_in_quote_ccy),
            })
            .collect(),
    };

    Ok(depth)
}

fn calc_qty(qty: f32, price: f32, contract_size: Option<f32>, size_in_quote_ccy: bool) -> f32 {
    match contract_size {
        Some(size) => qty * size,
        None => {
            if size_in_quote_ccy {
                (qty * price).round()
            } else {
                qty
            }
        }
    }
}

pub fn connect_kline_stream(
    streams: Vec<(TickerInfo, Timeframe)>,
    market: MarketKind,
) -> impl Stream<Item = Event> {
    stream::channel(100, async move |mut output| {
        let mut state = State::Disconnected;
        let exchange = exchange_from_market_type(market);

        let ticker_info_map = streams
            .iter()
            .map(|(ticker_info, _)| (ticker_info.ticker, *ticker_info))
            .collect::<HashMap<Ticker, TickerInfo>>();

        let size_in_quote_ccy = volume_size_unit() == SizeUnit::Quote;

        loop {
            match &mut state {
                State::Disconnected => {
                    let stream_str = streams
                        .iter()
                        .map(|(ticker_info, timeframe)| {
                            let ticker = ticker_info.ticker;
                            format!(
                                "{}@kline_{}",
                                ticker.to_full_symbol_and_type().0.to_lowercase(),
                                timeframe
                            )
                        })
                        .collect::<Vec<String>>()
                        .join("/");

                    let domain = ws_domain_from_market_type(market);
                    let url = format!("wss://{domain}/stream?streams={stream_str}");

                    if let Ok(websocket) = connect_ws(domain, &url).await {
                        state = State::Connected(websocket);
                        let _ = output.send(Event::Connected(exchange)).await;
                    } else {
                        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

                        let _ = output
                            .send(Event::Disconnected(
                                exchange,
                                "Failed to connect to websocket".to_string(),
                            ))
                            .await;
                    }
                }
                State::Connected(ws) => match ws.read_frame().await {
                    Ok(msg) => match msg.opcode {
                        OpCode::Text => {
                            if let Ok(StreamData::Kline(ticker, de_kline)) =
                                feed_de(&msg.payload[..], market)
                            {
                                let (buy_volume, sell_volume) = {
                                    let buy_volume = de_kline.taker_buy_base_asset_volume;
                                    let sell_volume = de_kline.volume - buy_volume;

                                    if size_in_quote_ccy {
                                        (
                                            (buy_volume * de_kline.close).round(),
                                            (sell_volume * de_kline.close).round(),
                                        )
                                    } else {
                                        (buy_volume, sell_volume)
                                    }
                                };

                                if let Some((_, tf)) = streams
                                    .iter()
                                    .find(|(_, tf)| tf.to_string() == de_kline.interval)
                                {
                                    if let Some(info) = ticker_info_map.get(&ticker) {
                                        let ticker_info = *info;
                                        let timeframe = *tf;

                                        let kline = Kline::new(
                                            de_kline.time,
                                            de_kline.open,
                                            de_kline.high,
                                            de_kline.low,
                                            de_kline.close,
                                            (buy_volume, sell_volume),
                                            info.min_ticksize,
                                        );

                                        let _ = output
                                            .send(Event::KlineReceived(
                                                StreamKind::Kline {
                                                    ticker_info,
                                                    timeframe,
                                                },
                                                kline,
                                            ))
                                            .await;
                                    } else {
                                        log::error!("Ticker info not found for ticker: {}", ticker);
                                    }
                                }
                            }
                        }
                        OpCode::Close => {
                            state = State::Disconnected;
                            let _ = output
                                .send(Event::Disconnected(
                                    exchange,
                                    "Connection closed".to_string(),
                                ))
                                .await;
                        }
                        _ => {}
                    },
                    Err(e) => {
                        state = State::Disconnected;
                        let _ = output
                            .send(Event::Disconnected(
                                exchange,
                                "Error reading frame: ".to_string() + &e.to_string(),
                            ))
                            .await;
                    }
                },
            }
        }
    })
}

#[derive(Deserialize, Debug, Clone)]
struct FetchedKlines(
    u64,
    #[serde(deserialize_with = "de_string_to_f32")] f32,
    #[serde(deserialize_with = "de_string_to_f32")] f32,
    #[serde(deserialize_with = "de_string_to_f32")] f32,
    #[serde(deserialize_with = "de_string_to_f32")] f32,
    #[serde(deserialize_with = "de_string_to_f32")] f32,
    u64,
    String,
    u32,
    #[serde(deserialize_with = "de_string_to_f32")] f32,
    String,
    String,
);

impl From<FetchedKlines> for Kline {
    fn from(fetched: FetchedKlines) -> Self {
        let sell_volume = fetched.5 - fetched.9;

        Self {
            time: fetched.0,
            open: Price::from_f32(fetched.1),
            high: Price::from_f32(fetched.2),
            low: Price::from_f32(fetched.3),
            close: Price::from_f32(fetched.4),
            volume: (fetched.9, sell_volume),
        }
    }
}

pub async fn fetch_klines(
    ticker_info: TickerInfo,
    timeframe: Timeframe,
    range: Option<(u64, u64)>,
) -> Result<Vec<Kline>, AdapterError> {
    let ticker = ticker_info.ticker;

    let (symbol_str, _market_type) = ticker.to_full_symbol_and_type();
    let timeframe_str = timeframe.to_string();

    let base_url = LINEAR_PERP_Domain.to_string() + "/fapi/v1/klines";

    let mut url = format!("{base_url}?symbol={symbol_str}&interval={timeframe_str}");

    let _limit_param = if let Some((start, end)) = range {
        let interval_ms = timeframe.to_milliseconds();
        let num_intervals = ((end - start) / interval_ms).min(1000);

        if num_intervals < 3 {
            let new_start = start - (interval_ms * 5);
            let new_end = end + (interval_ms * 5);
            let num_intervals = ((new_end - new_start) / interval_ms).min(1000);

            url.push_str(&format!(
                "&startTime={new_start}&endTime={new_end}&limit={num_intervals}"
            ));
        } else {
            url.push_str(&format!(
                "&startTime={start}&endTime={end}&limit={num_intervals}"
            ));
        }
        num_intervals
    } else {
        let num_intervals = 400;
        url.push_str(&format!("&limit={num_intervals}",));
        num_intervals
    };

    let weight = 2;
    let limiter = &FOREX_LIMITER;

    let fetched_klines: Vec<FetchedKlines> =
        limiter::http_parse_with_limiter(&url, limiter, weight, None, None).await?;

    let size_in_quote_ccy = volume_size_unit() == SizeUnit::Quote;

    let klines: Vec<_> = fetched_klines
        .into_iter()
        .map(|k| Kline {
            time: k.0,
            open: Price::from_f32(k.1).round_to_min_tick(ticker_info.min_ticksize),
            high: Price::from_f32(k.2).round_to_min_tick(ticker_info.min_ticksize),
            low: Price::from_f32(k.3).round_to_min_tick(ticker_info.min_ticksize),
            close: Price::from_f32(k.4).round_to_min_tick(ticker_info.min_ticksize),
            volume: {
                let sell_volume = if size_in_quote_ccy {
                    ((k.5 - k.9) * k.4).round()
                } else {
                    k.5 - k.9
                };
                let buy_volume = if size_in_quote_ccy {
                    (k.9 * k.4).round()
                } else {
                    k.9
                };
                (buy_volume, sell_volume)
            },
        })
        .collect();

    Ok(klines)
}

pub async fn fetch_ticksize() -> Result<HashMap<Ticker, Option<TickerInfo>>, AdapterError> {
    let url = LINEAR_PERP_Domain.to_string() + "/fapi/v1/exchangeInfo";
    let weight = 20;

    let response_text = crate::limiter::HTTP_CLIENT
        .get(&url)
        .send()
        .await
        .map_err(AdapterError::FetchError)?
        .text()
        .await
        .map_err(AdapterError::FetchError)?;

    let exchange_info: serde_json::Value = serde_json::from_str(&response_text)
        .map_err(|e| AdapterError::ParseError(format!("Failed to parse exchange info: {e}")))?;

    let symbols = exchange_info["symbols"]
        .as_array()
        .ok_or_else(|| AdapterError::ParseError("Missing symbols array".to_string()))?;

    let exchange = Exchange::Forex;
    let mut ticker_info_map = HashMap::new();

    // Comprehensive list of Major/Minor/Exotic Forex currencies supported (Base)
    let accepted_bases = [
        "EUR", "GBP", "JPY", "1000JPY", "AUD", "CAD", "CHF", "NZD", "SGD", 
        "HKD", "SEK", "NOK", "DKK", "ZAR", "TRY", "BRL", "MXN",
        "INR", "RUB", "KRW", "CNY", "IDR", "TWD", "THB", "VND"
    ];

    for item in symbols {
        let symbol_str = item["symbol"]
            .as_str()
            .ok_or_else(|| AdapterError::ParseError("Missing symbol".to_string()))?;

        if !is_symbol_supported(symbol_str, exchange, true) {
            continue;
        }

        if let Some(status) = item["status"].as_str()
            && status != "TRADING"
            && status != "HALT"
        {
            continue;
        }

        let base_asset = item["baseAsset"].as_str().unwrap_or("");
        let quote_asset = item["quoteAsset"].as_str().unwrap_or("");
        
        // Forex check for Futures:
        if quote_asset != "USDT" && quote_asset != "USDC" {
            continue;
        }
        
        if !accepted_bases.contains(&base_asset) {
            continue;
        }

        let filters = item["filters"]
            .as_array()
            .ok_or_else(|| AdapterError::ParseError("Missing filters array".to_string()))?;

        let price_filter = filters
            .iter()
            .find(|x| x["filterType"].as_str().unwrap_or_default() == "PRICE_FILTER");

        let min_qty = filters
            .iter()
            .find(|x| x["filterType"].as_str().unwrap_or_default() == "LOT_SIZE")
            .and_then(|x| x["minQty"].as_str())
            .ok_or_else(|| {
                AdapterError::ParseError("Missing minQty in LOT_SIZE filter".to_string())
            })?
            .parse::<f32>()
            .map_err(|e| AdapterError::ParseError(format!("Failed to parse minQty: {e}")))?;

        // Contract size is usually 1 for standard pairs but good to check
        let contract_size = item["contractSize"].as_f64().map(|v| v as f32); 

        let display_name = if symbol_str.ends_with("USDT") {
            format!("{}USD", base_asset.trim_start_matches("1000"))
        } else if symbol_str.ends_with("USDC") {
            format!("{}USD", base_asset.trim_start_matches("1000"))
        } else {
            symbol_str.to_string()
        };

        let ticker = Ticker::new_with_display(symbol_str, exchange, Some(&display_name));

        if let Some(price_filter) = price_filter {
            let min_ticksize = price_filter["tickSize"]
                .as_str()
                .ok_or_else(|| AdapterError::ParseError("tickSize not found".to_string()))?
                .parse::<f32>()
                .map_err(|e| AdapterError::ParseError(format!("Failed to parse tickSize: {e}")))?;

            let info = TickerInfo::new(ticker, min_ticksize, min_qty, contract_size);

            ticker_info_map.insert(ticker, Some(info));
        } else {
            ticker_info_map.insert(ticker, None);
        }
    }

    Ok(ticker_info_map)
}

pub async fn fetch_ticker_prices() -> Result<HashMap<Ticker, TickerStats>, AdapterError> {
    let url = LINEAR_PERP_Domain.to_string() + "/fapi/v1/ticker/24hr";
    let weight = 40;

    let limiter = &FOREX_LIMITER;

    let parsed_response: Vec<serde_json::Value> =
        limiter::http_parse_with_limiter(&url, limiter, weight, None, None).await?;

    let exchange = Exchange::Forex;
    let mut ticker_price_map = HashMap::new();

    let accepted_bases = [
        "EUR", "GBP", "JPY", "1000JPY", "AUD", "CAD", "CHF", "NZD", "SGD", 
        "HKD", "SEK", "NOK", "DKK", "ZAR", "TRY", "BRL", "MXN",
        "INR", "RUB", "KRW", "CNY", "IDR", "TWD", "THB", "VND"
    ];

    for item in parsed_response {
        let symbol = item["symbol"]
            .as_str()
            .ok_or_else(|| AdapterError::ParseError("Symbol not found".to_string()))?;

        // Filtering logic duplicated (ideal to share, but for now strict strict filter)
        let is_valid_forex = accepted_bases.iter().any(|base| {
            symbol.starts_with(base) && (symbol.ends_with("USDT") || symbol.ends_with("USDC"))
        });

        if !is_valid_forex {
            continue;
        }

        if !is_symbol_supported(symbol, exchange, false) {
            continue;
        }

        let last_price = item["lastPrice"]
            .as_str()
            .ok_or_else(|| AdapterError::ParseError("Last price not found".to_string()))?
            .parse::<f32>()
            .map_err(|e| AdapterError::ParseError(format!("Failed to parse last price: {e}")))?;

        let price_change_pt = item["priceChangePercent"]
            .as_str()
            .ok_or_else(|| AdapterError::ParseError("Price change percent not found".to_string()))?
            .parse::<f32>()
            .map_err(|e| {
                AdapterError::ParseError(format!("Failed to parse price change percent: {e}"))
            })?;

        let volume = item["quoteVolume"]
                    .as_str()
                    .ok_or_else(|| AdapterError::ParseError("Quote volume not found".to_string()))?
                    .parse::<f32>()
                    .map_err(|e| {
                        AdapterError::ParseError(format!("Failed to parse quote volume: {e}"))
                    })?;

        let ticker_stats = TickerStats {
            mark_price: last_price,
            daily_price_chg: price_change_pt,
            daily_volume: volume,
        };

        let base_asset = symbol.trim_end_matches("USDT").trim_end_matches("USDC");
        let display_name = format!("{}USD", base_asset.trim_start_matches("1000"));
        let ticker = Ticker::new_with_display(symbol, exchange, Some(&display_name));
        ticker_price_map.insert(ticker, ticker_stats);
    }

    Ok(ticker_price_map)
}

#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DeOpenInterest {
    #[serde(rename = "timestamp")]
    pub time: u64,
    #[serde(rename = "sumOpenInterest", deserialize_with = "de_string_to_f32")]
    pub sum: f32,
}

pub async fn fetch_historical_oi(
    ticker: Ticker,
    range: Option<(u64, u64)>,
    period: Timeframe,
) -> Result<Vec<OpenInterest>, AdapterError> {
    let (ticker_str, _) = ticker.to_full_symbol_and_type();
    let period_str = period.to_string();

    let base_url = LINEAR_PERP_Domain.to_string() + "/futures/data/openInterestHist";
    let mut url = format!("{}?symbol={}&period={}", base_url, ticker_str, period_str);

    if let Some((start, end)) = range {
         // API limit is 30 days
        let thirty_days_ms = 30 * 24 * 60 * 60 * 1000;
        let thirty_days_ago = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64
            - thirty_days_ms;

        let adjusted_start = if start < thirty_days_ago {
             thirty_days_ago
        } else {
             start
        };
        let interval_ms = period.to_milliseconds();
        let num_intervals = ((end - adjusted_start) / interval_ms).min(500);

        url.push_str(&format!(
            "&startTime={adjusted_start}&endTime={end}&limit={num_intervals}"
        ));
    } else {
        url.push_str("&limit=400");
    }

    let limiter = &FOREX_LIMITER;
    let weight = 12;

    let text = crate::limiter::http_request_with_limiter(&url, limiter, weight, None, None).await?;

    let binance_oi: Vec<DeOpenInterest> = serde_json::from_str(&text).map_err(|e| {
        AdapterError::ParseError(format!("Failed to parse open interest: {e}"))
    })?;

    // Open Interest is usually returned in base asset units or sometimes contracts.
    // For Linear Perps, it's usually Base Asset amount.
    // Since we don't have contract_size here readily, we assume 1 or use value directly.
    // Actually, fetch_ticksize saves contract_size in TickerInfo, but we don't have TickerInfo here.
    // However, for standard Linear (USDT), 1 contract = 1 unit usually, unless specified.
    // We will just return the raw sum.
    
    let open_interest = binance_oi
        .iter()
        .map(|x| OpenInterest {
            time: x.time,
            value: x.sum,
        })
        .collect::<Vec<OpenInterest>>();

    Ok(open_interest)
}
