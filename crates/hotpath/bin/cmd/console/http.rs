use eyre::Result;
use hotpath::MetricsJson;

/// Fetches metrics from the hotpath HTTP server
pub fn fetch_metrics(port: u16) -> Result<MetricsJson> {
    let url = format!("http://localhost:{}/metrics", port);
    let metrics: MetricsJson = ureq::get(&url)
        .call()
        .map_err(|e| eyre::eyre!("HTTP request failed: {}", e))?
        .body_mut()
        .read_json()
        .map_err(|e| eyre::eyre!("JSON deserialization failed: {}", e))?;
    Ok(metrics)
}
