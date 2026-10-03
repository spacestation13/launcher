pub const PROTOCOL_VERSION: u32 = 0;

pub struct HwidComponent {
    pub label: String,
    pub hash: [u8; 32],
    pub tier: u8,
}

pub struct HwidPayload {
    pub components: Vec<HwidComponent>,
    pub signature: Vec<u8>,
}

/// Stub implementation. Release builds replace this crate with the real
/// ss13-hwid via CI (see .github/workflows/).
pub fn collect_and_sign(_nonce: &[u8], _variant: &str) -> Option<HwidPayload> {
    None
}
