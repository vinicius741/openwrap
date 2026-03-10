use crate::profiles::{AssetKind, InlineAsset};

pub fn supported_inline_tag(tag: &str) -> Option<AssetKind> {
    match tag {
        "ca" => Some(AssetKind::Ca),
        "cert" => Some(AssetKind::Cert),
        "key" => Some(AssetKind::Key),
        "tls-auth" => Some(AssetKind::TlsAuth),
        "tls-crypt" => Some(AssetKind::TlsCrypt),
        _ => None,
    }
}

pub fn build_inline_asset(tag: &str, line: usize, content: String) -> Option<InlineAsset> {
    Some(InlineAsset {
        directive: tag.to_string(),
        kind: supported_inline_tag(tag)?,
        content,
        line,
    })
}
