use crate::dns::DnsPolicy;

pub fn dns_policy_to_string(policy: &DnsPolicy) -> &'static str {
    match policy {
        DnsPolicy::SplitDnsPreferred => "SplitDnsPreferred",
        DnsPolicy::FullOverride => "FullOverride",
        DnsPolicy::ObserveOnly => "ObserveOnly",
    }
}

pub fn dns_policy_from_string(value: &str) -> DnsPolicy {
    match value {
        "FullOverride" => DnsPolicy::FullOverride,
        "ObserveOnly" => DnsPolicy::ObserveOnly,
        _ => DnsPolicy::SplitDnsPreferred,
    }
}
