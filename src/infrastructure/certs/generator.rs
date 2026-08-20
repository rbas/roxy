use rcgen::{CertificateParams, DistinguishedName, DnType, IsCa, KeyUsagePurpose};
use time::{Duration, OffsetDateTime};

/// Build the standard Roxy CA certificate parameters.
pub(crate) fn build_ca_cert_params() -> CertificateParams {
    let mut params = CertificateParams::default();
    let mut distinguished_name = DistinguishedName::new();
    distinguished_name.push(DnType::CommonName, "Roxy Local Development CA");
    // Keep this identity byte-for-byte compatible with CAs created by older
    // Roxy versions; the daemon reconstructs the issuer from these params.
    distinguished_name.push(DnType::OrganizationName, "Roxy");
    params.distinguished_name = distinguished_name;
    params.is_ca = IsCa::Ca(rcgen::BasicConstraints::Unconstrained);
    let now = OffsetDateTime::now_utc();
    params.not_before = now;
    params.not_after = now + Duration::days(3650);
    params.key_usages = vec![KeyUsagePurpose::KeyCertSign, KeyUsagePurpose::CrlSign];
    params
}
