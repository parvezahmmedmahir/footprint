pub mod comparison;
pub mod heatmap;
pub mod indicator;
pub mod kline;

use exchange::Timeframe;
use serde::{Deserialize, Serialize};

use super::aggr::{
    self,
    ticks::TickAggr,
    time::{DataPoint, TimeSeries},
};
pub use kline::KlineChartKind;

// NEW: Enhanced data structures for professional trading features
#[derive(Debug, Clone)]
pub struct MarketAnalysis {
    pub rejection_zones: Vec<kline::RejectionZone>,
    pub large_orders: Vec<kline::LargeOrder>,
    pub volume_clusters: Vec<VolumeCluster>,
    pub support_resistance: Vec<SupportResistanceLevel>,
}

#[derive(Debug, Clone)]
pub struct VolumeCluster {
    pub price_level: exchange::util::Price,
    pub total_volume: f32,
    pub buy_volume: f32,
    pub sell_volume: f32,
    pub cluster_strength: f32,
    pub timestamp: u64,
}

#[derive(Debug, Clone)]
pub struct SupportResistanceLevel {
    pub price_level: exchange::util::Price,
    pub strength: f32,
    pub touches: u32,
    pub last_touch: u64,
    pub level_type: LevelType,
}

#[derive(Debug, Clone, PartialEq)]
pub enum LevelType {
    Support,
    Resistance,
    Breakout,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradingConfig {
    pub show_volume_profile: bool,
    pub show_delta_profile: bool,
    pub show_bid_ask: bool,
    pub highlight_rejections: bool,
    pub show_large_orders: bool,
    pub auto_detect_support_resistance: bool,
    pub volume_threshold: f32,
    pub rejection_threshold: f32,
    pub sr_touch_threshold: u32,
}

impl Default for TradingConfig {
    fn default() -> Self {
        Self {
            show_volume_profile: true,
            show_delta_profile: true,
            show_bid_ask: false,
            highlight_rejections: true,
            show_large_orders: true,
            auto_detect_support_resistance: true,
            volume_threshold: 1000.0,
            rejection_threshold: 0.7,
            sr_touch_threshold: 3,
        }
    }
}

pub enum PlotData<D: DataPoint> {
    TimeBased(TimeSeries<D>),
    TickBased(TickAggr),
}

impl<D: DataPoint> PlotData<D> {
    pub fn latest_y_midpoint(&self, calculate_target_y: impl Fn(exchange::Kline) -> f32) -> f32 {
        match self {
            PlotData::TimeBased(timeseries) => timeseries
                .latest_kline()
                .map_or(0.0, |kline| calculate_target_y(*kline)),
            PlotData::TickBased(tick_aggr) => tick_aggr
                .latest_dp()
                .map_or(0.0, |(dp, _)| calculate_target_y(dp.kline)),
        }
    }

    pub fn visible_price_range(
        &self,
        start_interval: u64,
        end_interval: u64,
    ) -> Option<(f32, f32)> {
        match self {
            PlotData::TimeBased(timeseries) => {
                timeseries.min_max_price_in_range(start_interval, end_interval)
            }
            PlotData::TickBased(tick_aggr) => {
                tick_aggr.min_max_price_in_range(start_interval as usize, end_interval as usize)
            }
        }
    }

    // NEW: Enhanced analysis methods for professional features
    pub fn analyze_market_structure(
        &self,
        start_interval: u64,
        end_interval: u64,
        config: &TradingConfig,
    ) -> MarketAnalysis {
        let mut analysis = MarketAnalysis {
            rejection_zones: Vec::new(),
            large_orders: Vec::new(),
            volume_clusters: Vec::new(),
            support_resistance: Vec::new(),
        };

        match self {
            PlotData::TimeBased(timeseries) => {
                self.analyze_time_based_data(
                    timeseries,
                    start_interval,
                    end_interval,
                    config,
                    &mut analysis,
                );
            }
            PlotData::TickBased(tick_aggr) => {
                self.analyze_tick_based_data(
                    tick_aggr,
                    start_interval,
                    end_interval,
                    config,
                    &mut analysis,
                );
            }
        }

        analysis
    }

    fn analyze_time_based_data(
        &self,
        timeseries: &TimeSeries<D>,
        start_interval: u64,
        end_interval: u64,
        config: &TradingConfig,
        analysis: &mut MarketAnalysis,
    ) {
        for (timestamp, dp) in timeseries.datapoints.range(start_interval..=end_interval) {
            if let (Some(kline), Some(footprint)) = (dp.kline(), dp.footprint()) {
                self.analyze_datapoint(*timestamp, kline, footprint, config, analysis);
            }
        }

        if config.auto_detect_support_resistance {
            self.detect_support_resistance(
                timeseries,
                start_interval,
                end_interval,
                config,
                analysis,
            );
        }
    }

    fn analyze_tick_based_data(
        &self,
        tick_aggr: &TickAggr,
        start_interval: u64,
        end_interval: u64,
        config: &TradingConfig,
        analysis: &mut MarketAnalysis,
    ) {
        let start_idx = start_interval as usize;
        let end_idx = end_interval as usize;

        for (index, dp) in tick_aggr
            .datapoints
            .iter()
            .enumerate()
            .filter(|(index, _)| *index >= start_idx && *index <= end_idx)
        {
            self.analyze_datapoint(index as u64, &dp.kline, &dp.footprint, config, analysis);
        }

        if config.auto_detect_support_resistance {
            // Support/resistance detection for tick-based data would need different logic
        }
    }

    fn analyze_datapoint(
        &self,
        timestamp: u64,
        kline: &exchange::Kline,
        footprint: &kline::KlineTrades,
        config: &TradingConfig,
        analysis: &mut MarketAnalysis,
    ) {
        // Analyze volume clusters
        if config.show_volume_profile || config.show_delta_profile {
            self.analyze_volume_clusters(timestamp, kline, footprint, config, analysis);
        }

        // Detect large orders
        if config.show_large_orders {
            self.detect_large_orders(timestamp, footprint, config, analysis);
        }
    }

    fn analyze_volume_clusters(
        &self,
        timestamp: u64,
        _kline: &exchange::Kline,
        footprint: &kline::KlineTrades,
        config: &TradingConfig,
        analysis: &mut MarketAnalysis,
    ) {
        let total_volume: f32 = footprint.trades.values().map(|g| g.total_qty()).sum();

        for (price, group) in &footprint.trades {
            if group.total_qty() >= config.volume_threshold {
                let cluster_strength = group.total_qty() / total_volume.max(1.0);

                analysis.volume_clusters.push(VolumeCluster {
                    price_level: *price,
                    total_volume: group.total_qty(),
                    buy_volume: group.buy_qty,
                    sell_volume: group.sell_qty,
                    cluster_strength,
                    timestamp,
                });
            }
        }
    }

    fn detect_large_orders(
        &self,
        timestamp: u64,
        footprint: &kline::KlineTrades,
        config: &TradingConfig,
        analysis: &mut MarketAnalysis,
    ) {
        for (price, group) in &footprint.trades {
            if group.buy_qty >= config.volume_threshold {
                analysis.large_orders.push(kline::LargeOrder {
                    price: *price,
                    volume: group.buy_qty,
                    is_buy: true,
                    timestamp,
                });
            }
            if group.sell_qty >= config.volume_threshold {
                analysis.large_orders.push(kline::LargeOrder {
                    price: *price,
                    volume: group.sell_qty,
                    is_buy: false,
                    timestamp,
                });
            }
        }
    }

    fn detect_support_resistance(
        &self,
        timeseries: &TimeSeries<D>,
        start_interval: u64,
        end_interval: u64,
        config: &TradingConfig,
        analysis: &mut MarketAnalysis,
    ) {
        // Simplified support/resistance detection based on price touches
        // In a real implementation, this would be more sophisticated

        let price_levels: Vec<f32> = timeseries
            .datapoints
            .range(start_interval..=end_interval)
            .flat_map(|(_, dp)| {
                dp.kline()
                    .map(|k| vec![k.low.to_f32(), k.high.to_f32()])
                    .unwrap_or_default()
            })
            .collect();

        // Group nearby price levels and count touches
        let mut level_map: std::collections::HashMap<u64, u32> = std::collections::HashMap::new();

        for price in price_levels {
            let rounded_price = ((price / 0.01).round() * 100.0) as u64; // Use u64 as key for predictability
            *level_map.entry(rounded_price).or_insert(0) += 1;
        }

        for (price_u64, touches) in level_map {
            let price = price_u64 as f32 / 100.0;
            if touches >= config.sr_touch_threshold {
                let level_type = if price <= timeseries.average_price().unwrap_or(price) {
                    LevelType::Support
                } else {
                    LevelType::Resistance
                };

                analysis.support_resistance.push(SupportResistanceLevel {
                    price_level: exchange::util::Price::from_f32(price),
                    strength: (touches as f32 / config.sr_touch_threshold as f32).min(1.0),
                    touches,
                    last_touch: end_interval,
                    level_type,
                });
            }
        }
    }

    // NEW: Get volume distribution for volume profile
    pub fn get_volume_distribution(
        &self,
        start_interval: u64,
        end_interval: u64,
    ) -> Vec<VolumeCluster> {
        let mut distribution = Vec::new();

        match self {
            PlotData::TimeBased(timeseries) => {
                for (timestamp, dp) in timeseries.datapoints.range(start_interval..=end_interval) {
                    if let Some(footprint) = dp.footprint() {
                        for (price, group) in &footprint.trades {
                            distribution.push(VolumeCluster {
                                price_level: *price,
                                total_volume: group.total_qty(),
                                buy_volume: group.buy_qty,
                                sell_volume: group.sell_qty,
                                cluster_strength: 0.0, // Will be calculated later
                                timestamp: *timestamp,
                            });
                        }
                    }
                }
            }
            PlotData::TickBased(tick_aggr) => {
                let start_idx = start_interval as usize;
                let end_idx = end_interval as usize;

                for (index, dp) in tick_aggr
                    .datapoints
                    .iter()
                    .enumerate()
                    .filter(|(index, _)| *index >= start_idx && *index <= end_idx)
                {
                    for (price, group) in &dp.footprint.trades {
                        distribution.push(VolumeCluster {
                            price_level: *price,
                            total_volume: group.total_qty(),
                            buy_volume: group.buy_qty,
                            sell_volume: group.sell_qty,
                            cluster_strength: 0.0,
                            timestamp: index as u64,
                        });
                    }
                }
            }
        }

        // Calculate cluster strengths based on total volume
        let total_volume: f32 = distribution.iter().map(|c| c.total_volume).sum();
        for cluster in &mut distribution {
            cluster.cluster_strength = cluster.total_volume / total_volume.max(1.0);
        }

        distribution
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct ViewConfig {
    pub splits: Vec<f32>,
    pub autoscale: Option<Autoscale>,
    // NEW: Enhanced view configuration for professional features
    pub trading_config: Option<TradingConfig>,
    pub show_volume_histogram: bool,
    pub show_price_levels: bool,
    pub show_market_depth: bool,
}

impl ViewConfig {
    // NEW: Constructor with trading-specific defaults
    pub fn trading_default() -> Self {
        Self {
            splits: vec![0.7, 0.3], // Main chart 70%, indicators 30%
            autoscale: Some(Autoscale::FitToVisible),
            trading_config: Some(TradingConfig::default()),
            show_volume_histogram: true,
            show_price_levels: true,
            show_market_depth: false,
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, Default, PartialEq)]
pub enum Autoscale {
    #[default]
    CenterLatest,
    FitToVisible,
    // NEW: Additional autoscale options for trading
    FitToVolume,  // Scale based on volume distribution
    LockToPrice,  // Lock to specific price level
    DynamicRange, // Adaptive scaling based on volatility
}

/// Defines how chart data is aggregated and displayed along the x-axis.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum Basis {
    /// Time-based aggregation where each datapoint represents a fixed time interval.
    Time(exchange::Timeframe),

    /// Trade-based aggregation where each datapoint represents a fixed number of trades.
    ///
    /// The u16 value represents the number of trades per aggregation unit.
    Tick(aggr::TickCount),

    // NEW: Volume-based aggregation for professional analysis
    /// Volume-based aggregation where each datapoint represents a fixed volume amount.
    Volume(u32),

    // NEW: Range-based aggregation for specific price movements
    /// Range-based aggregation where each datapoint represents a fixed price range.
    Range(f32),
}

impl Basis {
    pub fn is_time(&self) -> bool {
        matches!(self, Basis::Time(_))
    }

    pub fn is_tick(&self) -> bool {
        matches!(self, Basis::Tick(_))
    }

    // NEW: Check if volume-based
    pub fn is_volume(&self) -> bool {
        matches!(self, Basis::Volume(_))
    }

    // NEW: Check if range-based
    pub fn is_range(&self) -> bool {
        matches!(self, Basis::Range(_))
    }

    pub fn default_heatmap_time(ticker_info: Option<exchange::TickerInfo>) -> Self {
        let fallback = Timeframe::MS500;

        let interval = ticker_info.map_or(fallback, |info| {
            let ex = info.exchange();
            Timeframe::HEATMAP
                .iter()
                .copied()
                .find(|tf| ex.supports_heatmap_timeframe(*tf))
                .unwrap_or(fallback)
        });

        interval.into()
    }

    // NEW: Get appropriate basis for different analysis types
    pub fn for_volume_profile() -> Self {
        Basis::Volume(1000) // 1000 units per bar
    }

    pub fn for_market_analysis() -> Self {
        Basis::Time(Timeframe::M1) // 1-minute bars for detailed analysis
    }

    pub fn for_overview() -> Self {
        Basis::Time(Timeframe::H1) // 1-hour bars for overview
    }
}

impl std::fmt::Display for Basis {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Basis::Time(timeframe) => write!(f, "{timeframe}"),
            Basis::Tick(count) => write!(f, "{count}"),
            Basis::Volume(volume) => write!(f, "Vol{}", volume),
            Basis::Range(range) => write!(f, "Rng{:.2}", range),
        }
    }
}

impl From<exchange::Timeframe> for Basis {
    fn from(timeframe: exchange::Timeframe) -> Self {
        Self::Time(timeframe)
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum Study {
    Heatmap(Vec<heatmap::HeatmapStudy>),
    Footprint(Vec<kline::FootprintStudy>),
    // NEW: Additional study types for professional analysis
    VolumeProfile(VolumeProfileStudy),
    MarketDepth(MarketDepthStudy),
    OrderFlow(OrderFlowStudy),
}

// NEW: Volume profile study configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct VolumeProfileStudy {
    pub show_poc: bool,      // Point of Control
    pub show_vah: bool,      // Value Area High
    pub show_val: bool,      // Value Area Low
    pub va_percentage: f32,  // Value Area percentage (typically 70%)
    pub session_based: bool, // Session-based or total profile
}

impl Default for VolumeProfileStudy {
    fn default() -> Self {
        Self {
            show_poc: true,
            show_vah: true,
            show_val: true,
            va_percentage: 0.7,
            session_based: true,
        }
    }
}

// NEW: Market depth study
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MarketDepthStudy {
    pub levels: u32,           // Number of levels to show
    pub show_cumulative: bool, // Show cumulative volume
    pub animate_changes: bool, // Animate depth changes
}

impl Default for MarketDepthStudy {
    fn default() -> Self {
        Self {
            levels: 10,
            show_cumulative: true,
            animate_changes: true,
        }
    }
}

// NEW: Order flow study
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OrderFlowStudy {
    pub show_imbalances: bool,
    pub show_absorption: bool,
    pub show_liquidity: bool,
    pub threshold: f32,
}

impl Default for OrderFlowStudy {
    fn default() -> Self {
        Self {
            show_imbalances: true,
            show_absorption: true,
            show_liquidity: false,
            threshold: 0.7,
        }
    }
}

// NEW: Chart performance optimization settings
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PerformanceConfig {
    pub max_data_points: usize, // Maximum points to keep in memory
    pub render_quality: RenderQuality,
    pub update_frequency: u32, // Updates per second
    pub cache_enabled: bool,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq)]
pub enum RenderQuality {
    Low,    // Faster rendering
    Medium, // Balanced
    High,   // Best quality
}

impl Default for PerformanceConfig {
    fn default() -> Self {
        Self {
            max_data_points: 10000,
            render_quality: RenderQuality::Medium,
            update_frequency: 60,
            cache_enabled: true,
        }
    }
}
