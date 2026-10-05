use base64::Engine;

use crate::auth::hub_client::{HubAuthError, HubClient};

pub async fn exchange_hub_ticket(token: &str, server_id: &str) -> Result<String, HubAuthError> {
    tracing::info!(server_id, "exchange_hub_ticket: calling join");
    let nonce_b64 = HubClient::join(token, server_id).await?;
    tracing::info!(
        server_id,
        "exchange_hub_ticket: got nonce, calling join_complete"
    );

    let nonce_bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(&nonce_b64)
        .map_err(|e| HubAuthError::Server(format!("invalid nonce encoding: {e}")))?;

    let (version, components, signature) = collect_hwid(&nonce_bytes);
    let config = crate::config::get_config();

    let ticket = HubClient::join_complete(
        token,
        &nonce_b64,
        version,
        &components,
        signature.as_deref(),
        config.variant,
    )
    .await?;
    tracing::info!(server_id, ticket = %ticket, "exchange_hub_ticket: got auth ticket from hub");
    Ok(ticket)
}

#[cfg(feature = "hwid")]
fn collect_hwid(nonce: &[u8]) -> (u32, Vec<serde_json::Value>, Option<String>) {
    let config = crate::config::get_config();
    match launcher_hwid::collect_and_sign(nonce, config.variant) {
        Some(payload) => {
            let components: Vec<serde_json::Value> = payload
                .components
                .iter()
                .map(|c| {
                    serde_json::json!({
                        "label": c.label,
                        "hash": base64::engine::general_purpose::STANDARD.encode(c.hash),
                        "tier": c.tier,
                    })
                })
                .collect();
            let sig = base64::engine::general_purpose::STANDARD.encode(&payload.signature);
            (launcher_hwid::PROTOCOL_VERSION, components, Some(sig))
        }
        None => (launcher_hwid::PROTOCOL_VERSION, Vec::new(), None),
    }
}

#[cfg(not(feature = "hwid"))]
fn collect_hwid(_nonce: &[u8]) -> (u32, Vec<serde_json::Value>, Option<String>) {
    tracing::warn!("HWID collection unavailable (built without `hwid` feature)");
    (0, Vec::new(), None)
}
