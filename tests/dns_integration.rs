// Integration tests require root privileges
// Run with: sudo cargo test --test dns_integration

#[test]
#[ignore] // Run manually with sudo
fn test_dns_setup_and_cleanup() {
    use roxy::infrastructure::dns::{DnsService, get_dns_service};

    let dns = get_dns_service().unwrap();

    // Clean state
    let _ = dns.cleanup();
    assert!(!dns.is_configured());

    // Setup
    dns.setup().unwrap();
    assert!(dns.is_configured());

    // Validate
    dns.validate().unwrap();

    // Cleanup
    dns.cleanup().unwrap();
    assert!(!dns.is_configured());
}
