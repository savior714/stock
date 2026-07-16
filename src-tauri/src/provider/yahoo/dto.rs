use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct YahooChartResponse {
    pub chart: Option<YahooChart>,
}

#[derive(Debug, Deserialize)]
pub struct YahooChart {
    pub result: Option<Vec<YahooChartData>>,
    #[serde(rename = "error")]
    pub error: Option<YahooChartError>,
}

#[derive(Debug, Deserialize)]
pub struct YahooChartData {
    pub meta: Option<YahooMeta>,
    pub timestamp: Option<Vec<i64>>,
    pub indicators: Option<YahooIndicators>,
}

#[derive(Debug, Deserialize)]
pub struct YahooMeta {
    pub currency: Option<String>,
    pub symbol: Option<String>,
    #[serde(rename = "regularMarketPrice")]
    pub regular_market_price: Option<f64>,
    pub chart_previous_close: Option<f64>,
    #[serde(rename = "dataGranularity")]
    pub data_granularity: Option<String>,
    #[serde(rename = "rangeSeconds")]
    pub range_seconds: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct YahooIndicators {
    pub quote: Option<Vec<YahooQuote>>,
    pub adjclose: Option<Vec<YahooAdjClose>>,
}

#[derive(Debug, Deserialize)]
pub struct YahooQuote {
    pub open: Option<Vec<Option<f64>>>,
    pub high: Option<Vec<Option<f64>>>,
    pub low: Option<Vec<Option<f64>>>,
    pub close: Option<Vec<Option<f64>>>,
    pub volume: Option<Vec<Option<u64>>>,
}

#[derive(Debug, Deserialize)]
pub struct YahooAdjClose {
    pub adjclose: Option<Vec<Option<f64>>>,
}

#[derive(Debug, Deserialize)]
pub struct YahooChartError {
    pub code: String,
    pub description: String,
}
