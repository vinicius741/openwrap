use std::collections::HashMap;

use crate::profiles::{AssetKind, ParsedProfile};

pub fn rewrite_profile(
    parsed: &ParsedProfile,
    rewritten_assets: &HashMap<AssetKind, String>,
) -> String {
    let mut output = Vec::new();
    let mut auth_nocache_present = false;

    for directive in &parsed.directives {
        if directive.name == "auth-nocache" {
            auth_nocache_present = true;
        }

        if let Some(kind) = AssetKind::from_directive(&directive.name) {
            if let Some(path) = rewritten_assets.get(&kind) {
                output.push(format!("{} {}", directive.name, path));
                continue;
            }
        }

        if directive.name == "auth-user-pass" {
            output.push("auth-user-pass".to_string());
            continue;
        }

        let mut parts = vec![directive.name.clone()];
        parts.extend(directive.args.clone());
        output.push(parts.join(" "));
    }

    for inline in &parsed.inline_assets {
        if let Some(path) = rewritten_assets.get(&inline.kind) {
            output.push(format!("{} {}", inline.directive, path));
        }
    }

    if !auth_nocache_present {
        output.push("auth-nocache".to_string());
    }

    output.join("\n") + "\n"
}
