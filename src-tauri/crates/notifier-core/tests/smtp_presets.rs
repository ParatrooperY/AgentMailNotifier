use agent_mail_notifier_core::{SmtpEncryption, SmtpPreset, smtp_preset_for};

#[test]
fn resolves_supported_email_domains_without_exposing_server_fields() {
    assert_eq!(
        smtp_preset_for(&["test-user", "qq.com"].join("@")),
        Some(SmtpPreset {
            provider_label: "QQ 邮箱 · SSL 465",
            host: "smtp.qq.com",
            port: 465,
            encryption: SmtpEncryption::Ssl,
        }),
    );
    assert_eq!(
        smtp_preset_for(&["test-user", "outlook.com"].join("@")),
        Some(SmtpPreset {
            provider_label: "Outlook · STARTTLS 587",
            host: "smtp.office365.com",
            port: 587,
            encryption: SmtpEncryption::Starttls,
        }),
    );
    assert_eq!(smtp_preset_for(&["test-user", "unknown.invalid"].join("@")), None);
}
