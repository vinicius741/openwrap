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

#[cfg(test)]
mod tests {
    use super::{build_inline_asset, supported_inline_tag};
    use crate::profiles::AssetKind;

    #[test]
    fn supported_inline_tag_ca() {
        assert_eq!(supported_inline_tag("ca"), Some(AssetKind::Ca));
    }

    #[test]
    fn supported_inline_tag_cert() {
        assert_eq!(supported_inline_tag("cert"), Some(AssetKind::Cert));
    }

    #[test]
    fn supported_inline_tag_key() {
        assert_eq!(supported_inline_tag("key"), Some(AssetKind::Key));
    }

    #[test]
    fn supported_inline_tag_tls_auth() {
        assert_eq!(supported_inline_tag("tls-auth"), Some(AssetKind::TlsAuth));
    }

    #[test]
    fn supported_inline_tag_tls_crypt() {
        assert_eq!(supported_inline_tag("tls-crypt"), Some(AssetKind::TlsCrypt));
    }

    #[test]
    fn supported_inline_tag_unsupported() {
        assert_eq!(supported_inline_tag("pkcs12"), None);
        assert_eq!(supported_inline_tag("unknown"), None);
        assert_eq!(supported_inline_tag(""), None);
    }

    #[test]
    fn build_inline_asset_with_valid_tag() {
        let asset = build_inline_asset("ca", 10, "CERTIFICATE DATA".to_string());
        assert!(asset.is_some());
        let asset = asset.unwrap();
        assert_eq!(asset.directive, "ca");
        assert_eq!(asset.kind, AssetKind::Ca);
        assert_eq!(asset.content, "CERTIFICATE DATA");
        assert_eq!(asset.line, 10);
    }

    #[test]
    fn build_inline_asset_with_invalid_tag() {
        assert!(build_inline_asset("unsupported", 1, "content".to_string()).is_none());
    }

    #[test]
    fn build_inline_asset_preserves_line_number() {
        let asset = build_inline_asset("key", 42, "key content".to_string()).unwrap();
        assert_eq!(asset.line, 42);
    }

    #[test]
    fn build_inline_asset_preserves_content() {
        let content = "-----BEGIN RSA PRIVATE KEY-----\ndata\n-----END RSA PRIVATE KEY-----";
        let asset = build_inline_asset("key", 1, content.to_string()).unwrap();
        assert_eq!(asset.content, content);
    }
}
